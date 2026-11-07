//! `RedditWebBackend`: the [`Backend`] on a hidden WebKit view (SPEC §4).

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agora_core::*;
use futures::FutureExt;
use futures::channel::oneshot;
use gtk::{gio, glib};
use serde::{Deserialize, Serialize};
use serde_json::json;
use webkit6::prelude::*;
use webkit6::{LoadEvent, NetworkSession, UserContentFilterStore, UserContentManager, WebView, WebsiteDataTypes};

use crate::bridge::Bridge;
use crate::parse::comments::More;
use crate::parse::reply::{Outcome, Reply, classify, is_login_url};
use crate::parse::{self, broken};
use crate::quiet;
use crate::scheduler::{Lane, Next, Scheduler, View};

/// How long the home page may take to load.
const HOME_TIMEOUT: Duration = Duration::from_secs(60);

/// Where the backend keeps its data, and which site it talks to.
#[derive(Debug, Clone)]
pub struct Config {
    /// `https://www.reddit.com`; a local server in the bridge tests.
    pub origin: String,
    /// Path of the hidden view's home page on `origin`.
    pub home_path: String,
    /// WebKit's persistent data, including the login.
    pub data_dir: PathBuf,
    /// WebKit's HTTP cache and compiled filters.
    pub cache_dir: PathBuf,
    /// Survives restarts so quitting never shortens a back-off.
    pub backoff_file: PathBuf,
}

impl Config {
    /// The real site and the directories of SPEC §14.
    pub fn reddit() -> Self {
        let data = glib::user_data_dir().join("agora");
        let cache = glib::user_cache_dir().join("agora");
        Self {
            origin: "https://www.reddit.com".into(),
            home_path: "/r/popular/".into(),
            data_dir: data.join("web"),
            cache_dir: cache.join("web"),
            backoff_file: data.join("backoff.json"),
        }
    }
}

/// A back-off, as saved in [`Config::backoff_file`].
#[derive(Serialize, Deserialize)]
struct SavedBackoff {
    until_unix: u64,
    length_secs: u64,
}

type Waiter = oneshot::Sender<BackendResult<()>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Home {
    NotLoaded,
    Loading,
    Loaded,
}

struct Inner {
    config: Config,
    session: NetworkSession,
    manager: UserContentManager,
    view: WebView,
    bridge: Bridge,
    scheduler: RefCell<Scheduler<Waiter>>,
    start: Instant,
    pumping: Cell<bool>,
    home: Cell<Home>,
    home_waiters: RefCell<Vec<oneshot::Sender<BackendResult<()>>>>,
    filter_added: Cell<bool>,
    /// An operation found the session logged out; requests stop until a
    /// `whoami` finds it signed in again (SPEC §4.4).
    expired: Cell<bool>,
    /// Names from the last `subscriptions`, lower case: the right rail does not
    /// say whether the user joined.
    joined: RefCell<HashSet<String>>,
    events: (
        async_channel::Sender<BackendEvent>,
        async_channel::Receiver<BackendEvent>,
    ),
}

/// Holds the single in-flight slot; the next request may go when it drops.
struct Permit(Rc<Inner>);

impl Drop for Permit {
    fn drop(&mut self) {
        let now = self.0.now();
        self.0.scheduler.borrow_mut().finished(now);
        Inner::pump(&self.0);
    }
}

/// Ends a home page load: a load that did not succeed can be tried again, and
/// anyone still waiting is told it was cancelled.
struct HomeLoad(Rc<Inner>);

impl Drop for HomeLoad {
    fn drop(&mut self) {
        if self.0.home.get() == Home::Loading {
            self.0.home.set(Home::NotLoaded);
        }
        self.0.home_waiters.borrow_mut().clear();
    }
}

/// Which lane and view a request belongs to.
#[derive(Clone)]
struct Slot {
    lane: Lane,
    view: View,
    /// A later step of an operation already running.
    step: bool,
}

impl Inner {
    fn now(&self) -> Duration {
        self.start.elapsed()
    }

    /// Sends what may be sent now, and waits when the pacing says so.
    fn pump(this: &Rc<Self>) {
        if this.pumping.replace(true) {
            return;
        }
        let this = this.clone();
        glib::spawn_future_local(async move {
            loop {
                let next = this.scheduler.borrow_mut().next(this.now());
                match next {
                    Next::Wait(wait) => glib::timeout_future(wait).await,
                    Next::Idle => break,
                    Next::Send(waiter) => {
                        if waiter.send(Ok(())).is_err() {
                            // Nobody is waiting any more; nothing was sent.
                            this.scheduler.borrow_mut().finished(this.now());
                            continue;
                        }
                        break;
                    }
                }
            }
            this.pumping.set(false);
        });
    }

    /// Waits for this request's turn.
    async fn acquire(this: &Rc<Self>, slot: &Slot) -> BackendResult<Permit> {
        let (sender, receiver) = oneshot::channel();
        {
            let mut scheduler = this.scheduler.borrow_mut();
            if slot.step {
                if let Err(waiter) = scheduler.enqueue_step(slot.lane, slot.view.clone(), sender) {
                    let _ = waiter.send(Err(AgoraError::Cancelled));
                }
            } else {
                for waiter in scheduler.enqueue(slot.lane, slot.view.clone(), sender) {
                    let _ = waiter.send(Err(AgoraError::Cancelled));
                }
            }
        }
        Self::pump(this);
        match receiver.await {
            Ok(Ok(())) => Ok(Permit(this.clone())),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(AgoraError::Cancelled),
        }
    }

    fn check_block(&self) -> BackendResult<()> {
        match self.scheduler.borrow().blocked_for(self.now()) {
            Some(retry_after) => Err(AgoraError::Blocked { retry_after }),
            None => Ok(()),
        }
    }

    /// Why nothing may be sent at the moment.
    fn refuse(&self, operation: &str) -> BackendResult<()> {
        self.check_block()?;
        if self.expired.get() && operation != "whoami" {
            return Err(AgoraError::SessionExpired);
        }
        Ok(())
    }

    /// Runs one request through the lanes.
    async fn call(this: &Rc<Self>, slot: Slot, op: &'static str, args: serde_json::Value) -> BackendResult<Reply> {
        this.refuse(op)?;
        Self::ensure_home(this, slot.lane).await?;
        let permit = Self::acquire(this, &slot).await?;
        // A block may have started while this request waited.
        this.refuse(op)?;
        tracing::debug!(op, "request");
        let reply = this.bridge.call(op, &args).await;
        drop(permit);
        let reply = reply?;
        match classify(&reply) {
            Outcome::Ok => Ok(reply),
            Outcome::Pushback => Err(this.pushback()),
            Outcome::LoggedOut if op == "whoami" => Ok(reply),
            Outcome::LoggedOut => Err(this.logged_out()),
            Outcome::Failed(e) => Err(e),
        }
    }

    /// Stops all traffic after pushback (SPEC §4.6) and reports it.
    fn pushback(&self) -> AgoraError {
        let (retry_after, dropped) = self.scheduler.borrow_mut().pushback(self.now());
        for waiter in dropped {
            let _ = waiter.send(Err(AgoraError::Blocked { retry_after }));
        }
        tracing::warn!("Reddit pushed back; stopping for {retry_after:?}");
        self.save_backoff(retry_after);
        let _ = self.events.0.try_send(BackendEvent::Blocked { retry_after });
        AgoraError::Blocked { retry_after }
    }

    /// Requests stop, and what was queued is never sent (SPEC §4.4).
    fn logged_out(&self) -> AgoraError {
        if !self.expired.replace(true) {
            let _ = self.events.0.try_send(BackendEvent::SessionExpired);
        }
        for waiter in self.scheduler.borrow_mut().drain() {
            let _ = waiter.send(Err(AgoraError::SessionExpired));
        }
        AgoraError::SessionExpired
    }

    fn save_backoff(&self, length: Duration) {
        let until = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + length.as_secs();
        let saved = SavedBackoff {
            until_unix: until,
            length_secs: length.as_secs(),
        };
        let path = &self.config.backoff_file;
        let result = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(path, serde_json::to_vec(&saved).unwrap_or_default()));
        if let Err(e) = result {
            tracing::warn!("could not save the back-off: {e}");
        }
    }

    fn restore_backoff(&self) {
        let Ok(bytes) = std::fs::read(&self.config.backoff_file) else {
            return;
        };
        let Ok(saved) = serde_json::from_slice::<SavedBackoff>(&bytes) else {
            return;
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if saved.until_unix > now {
            self.scheduler.borrow_mut().restore_backoff(
                self.now(),
                Duration::from_secs(saved.until_unix - now),
                Duration::from_secs(saved.length_secs),
            );
        }
    }

    /// A failure only costs the quiet page, so it is logged and not fatal.
    async fn add_quiet_filter(&self, manager: &UserContentManager) {
        let store = UserContentFilterStore::new(&self.config.cache_dir.join("filters").to_string_lossy());
        match store
            .save_future("agora-quiet", &glib::Bytes::from_owned(quiet::rules().into_bytes()))
            .await
        {
            Ok(filter) => manager.add_filter(&filter),
            Err(e) => tracing::warn!("could not compile the quiet-page filter: {e}"),
        }
    }

    fn on_home_page(&self) -> bool {
        self.home.get() == Home::Loaded
            && self
                .view
                .uri()
                .is_some_and(|uri| uri.starts_with(&format!("{}/", self.config.origin)))
    }

    /// The hidden view must sit on a real page before any call (SPEC §4.2).
    /// Loading it counts as a request.
    async fn ensure_home(this: &Rc<Self>, lane: Lane) -> BackendResult<()> {
        if this.on_home_page() {
            return Ok(());
        }
        if this.home.get() == Home::Loading {
            let (sender, receiver) = oneshot::channel();
            this.home_waiters.borrow_mut().push(sender);
            return receiver.await.unwrap_or(Err(AgoraError::Cancelled));
        }
        this.home.set(Home::Loading);
        // However this ends, nobody stays stuck behind `Loading`.
        let guard = HomeLoad(this.clone());
        let slot = Slot {
            lane,
            view: View::None,
            step: false,
        };
        let result = Self::load_home(this, &slot).await;
        if result.is_ok() {
            this.home.set(Home::Loaded);
        }
        for waiter in this.home_waiters.take() {
            let _ = waiter.send(result.clone());
        }
        drop(guard);
        result
    }

    async fn load_home(this: &Rc<Self>, slot: &Slot) -> BackendResult<()> {
        if !this.filter_added.replace(true) {
            this.add_quiet_filter(&this.manager).await;
        }
        let permit = Self::acquire(this, slot).await?;
        // The session may be expired: signing in again needs this page too.
        this.check_block()?;
        let url = format!("{}{}", this.config.origin, this.config.home_path);
        tracing::debug!(%url, "loading the home page");
        let status = load_page(&this.view, &url).await;
        drop(permit);
        let status = status?;
        match status {
            429 | 403 => Err(this.pushback()),
            0 => Err(AgoraError::Network("the home page gave no response".into())),
            s if s >= 400 => Err(AgoraError::Network(format!("HTTP {s}"))),
            _ => Ok(()),
        }
    }

    async fn whoami(this: &Rc<Self>, lane: Lane) -> BackendResult<Option<Username>> {
        let slot = Slot {
            lane,
            view: View::None,
            step: false,
        };
        let reply = Self::call(this, slot, "whoami", json!({})).await?;
        if is_login_url(&reply.final_url) {
            return Ok(None);
        }
        let name = parse::user::whoami(&reply.final_url);
        if name.is_some() {
            this.expired.set(false);
            // Signed out, the home page may have ended up on the login page.
            if this.view.uri().is_some_and(|uri| is_login_url(&uri)) {
                this.home.set(Home::NotLoaded);
            }
        }
        Ok(name)
    }

    async fn listing(this: &Rc<Self>, lane: Lane, request: ListingRequest) -> BackendResult<Listing<Post>> {
        let view = View::Feed(feed_key(&request.feed));
        let (feed, name) = match &request.feed {
            Feed::Home => ("home", String::new()),
            Feed::Popular => ("popular", String::new()),
            Feed::Subreddit(name) => ("subreddit", name.to_string()),
            Feed::User(name) => ("user", name.to_string()),
            Feed::Saved => ("saved", String::new()),
        };
        let range = match request.sort {
            Sort::Top(range) => range.key(),
            _ => "day",
        };
        let args = json!({
            "feed": feed,
            "name": name,
            "sort": request.sort.key(),
            "range": range,
            "after": request.after.as_ref().map(Cursor::as_str),
        });
        if request
            .after
            .as_ref()
            .is_some_and(|c| !c.as_str().starts_with("/svc/shreddit/"))
        {
            return Err(broken("listing"));
        }
        let slot = Slot {
            lane,
            view,
            step: false,
        };
        let reply = Self::call(this, slot, "listing", args).await?;
        parse::listing::parse(&reply.body)
    }

    async fn thread(this: &Rc<Self>, lane: Lane, request: ThreadRequest) -> BackendResult<(Post, Vec<CommentNode>)> {
        let view = View::Thread(request.post.as_str().to_string());
        let bare = request.post.as_str().trim_start_matches("t3_").to_string();
        let slot = Slot {
            lane,
            view,
            step: false,
        };
        let page = Self::call(
            this,
            slot.clone(),
            "post_page",
            json!({"subreddit": request.subreddit.as_str(), "post": bare}),
        )
        .await?;
        let post = parse::post_page(&page.body, &request.post)?;
        let focus = request.focus.as_ref().map(|c| c.as_str().trim_start_matches("t1_"));
        let tree = Self::call(
            this,
            Slot { step: true, ..slot },
            "comments",
            json!({
                "subreddit": request.subreddit.as_str(),
                "post": bare,
                "sort": comment_sort_param(request.sort),
                "focus": focus,
            }),
        )
        .await?;
        if !tree.body.trim().is_empty() && !tree.body.contains("<shreddit-") {
            return Err(broken("post_with_comments"));
        }
        Ok((post, parse::comments::parse(&tree.body, &request.post)))
    }

    async fn more_comments(
        this: &Rc<Self>,
        lane: Lane,
        post: PostId,
        token: MoreToken,
    ) -> BackendResult<Vec<CommentNode>> {
        let more = More::from_token(&token)?;
        if !more.src.starts_with("/svc/shreddit/more-comments/") {
            return Err(broken("more_comments"));
        }
        let view = View::Thread(post.as_str().to_string());
        let slot = Slot {
            lane,
            view,
            step: false,
        };
        let args = json!({"src": more.src, "cursor": more.cursor});
        let reply = Self::call(this, slot, "more_comments", args).await?;
        if !reply.body.trim().is_empty() && !reply.body.contains("<shreddit-") {
            return Err(broken("more_comments"));
        }
        Ok(parse::comments::parse(&reply.body, &post))
    }

    async fn subreddit_info(this: &Rc<Self>, lane: Lane, name: SubredditName) -> BackendResult<Subreddit> {
        let view = View::Feed(feed_key(&Feed::Subreddit(name.clone())));
        let slot = Slot {
            lane,
            view,
            step: false,
        };
        let reply = Self::call(this, slot, "subreddit_info", json!({"name": name.as_str()})).await?;
        let mut sub = parse::subreddit::parse(&reply.body, &name)?;
        sub.subscribed |= this.joined.borrow().contains(&sub.name.as_str().to_lowercase());
        Ok(sub)
    }

    async fn subscriptions(this: &Rc<Self>, lane: Lane) -> BackendResult<Vec<Subreddit>> {
        let slot = Slot {
            lane,
            view: View::None,
            step: false,
        };
        let reply = Self::call(this, slot, "subscriptions", json!({})).await?;
        let subs = parse::subscriptions::parse(&reply.body)?;
        this.joined
            .replace(subs.iter().map(|s| s.name.as_str().to_lowercase()).collect());
        Ok(subs)
    }

    async fn user_profile(this: &Rc<Self>, lane: Lane, name: Username) -> BackendResult<User> {
        let slot = Slot {
            lane,
            view: View::None,
            step: false,
        };
        let reply = Self::call(this, slot, "user_profile", json!({"name": name.as_str()})).await?;
        parse::user::profile(&reply.body, &name)
    }

    async fn notifications(this: &Rc<Self>, lane: Lane) -> BackendResult<Listing<Notification>> {
        let slot = Slot {
            lane,
            view: View::None,
            step: false,
        };
        let reply = Self::call(this, slot, "notifications", json!({})).await?;
        parse::notifications::parse(&reply.body)
    }

    async fn search(this: &Rc<Self>, lane: Lane, request: SearchRequest) -> BackendResult<SearchResults> {
        let slot = Slot {
            lane,
            view: View::None,
            step: false,
        };
        let sort = match request.sort {
            SearchSort::Relevance => "relevance",
            SearchSort::Hot => "hot",
            SearchSort::Top => "top",
            SearchSort::New => "new",
            SearchSort::Comments => "comments",
        };
        let args = json!({
            "query": request.query,
            "subreddit": request.subreddit.as_ref().map(SubredditName::as_str),
            "sort": sort,
            "range": request.range.key(),
            "after": request.after.as_ref().map(Cursor::as_str),
        });
        let reply = Self::call(this, slot, "search", args).await?;
        parse::search::parse(&reply.body)
    }
}

fn feed_key(feed: &Feed) -> String {
    match feed {
        Feed::Home => "home".into(),
        Feed::Popular => "popular".into(),
        Feed::Saved => "saved".into(),
        Feed::Subreddit(name) => format!("r/{}", name.as_str().to_lowercase()),
        Feed::User(name) => format!("u/{}", name.as_str().to_lowercase()),
    }
}

/// Reddit's name for a comment order.
fn comment_sort_param(sort: CommentSort) -> &'static str {
    match sort {
        CommentSort::Best => "CONFIDENCE",
        CommentSort::Top => "TOP",
        CommentSort::New => "NEW",
        CommentSort::Controversial => "CONTROVERSIAL",
        CommentSort::Old => "OLD",
        CommentSort::Qa => "QA",
    }
}

/// Loads `url` and returns its HTTP status.
async fn load_page(view: &WebView, url: &str) -> BackendResult<u32> {
    let (sender, receiver) = async_channel::bounded::<BackendResult<u32>>(1);
    let done = sender.clone();
    let changed = view.connect_load_changed(move |view, event| {
        if event == LoadEvent::Finished {
            let status = view
                .main_resource()
                .and_then(|r| r.response())
                .map_or(0, |r| r.status_code());
            let _ = done.try_send(Ok(status));
        }
    });
    let failed = view.connect_load_failed(move |_, _, _, error| {
        if !error.matches(webkit6::NetworkError::Cancelled) {
            let _ = sender.try_send(Err(AgoraError::Network(error.message().to_string())));
        }
        false
    });
    view.load_uri(url);
    let recv = std::pin::pin!(receiver.recv());
    let timeout = std::pin::pin!(glib::timeout_future(HOME_TIMEOUT));
    let result = futures::future::select(recv, timeout).await;
    view.disconnect(changed);
    view.disconnect(failed);
    match result {
        futures::future::Either::Left((Ok(status), _)) => status,
        _ => {
            view.stop_loading();
            Err(AgoraError::Network("the home page did not load".into()))
        }
    }
}

/// One line of the self-test's report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfTestLine {
    /// The operation.
    pub operation: &'static str,
    /// `Ok` with a short note, or what went wrong.
    pub result: Result<String, String>,
}

/// The [`Backend`] on a hidden WebKit view. Create one per application; it does
/// nothing and sends nothing until the first operation.
#[derive(Clone)]
pub struct RedditWebBackend {
    inner: Rc<Inner>,
}

impl RedditWebBackend {
    /// Creates the session and the hidden view (not shown, no traffic yet).
    pub fn new(config: Config) -> Self {
        let session = NetworkSession::new(
            Some(&config.data_dir.to_string_lossy()),
            Some(&config.cache_dir.to_string_lossy()),
        );
        let manager = UserContentManager::new();
        let view = WebView::builder()
            .network_session(&session)
            .user_content_manager(&manager)
            .build();
        view.set_is_muted(true);
        if let Some(settings) = WebViewExt::settings(&view) {
            settings.set_media_playback_requires_user_gesture(true);
        }
        let bridge = Bridge::new(view.clone(), &manager, &config.origin);
        let inner = Rc::new(Inner {
            config,
            session,
            manager,
            view,
            bridge,
            scheduler: RefCell::new(Scheduler::new(random_jitter)),
            start: Instant::now(),
            pumping: Cell::new(false),
            home: Cell::new(Home::NotLoaded),
            home_waiters: RefCell::default(),
            filter_added: Cell::new(false),
            expired: Cell::new(false),
            joined: RefCell::default(),
            events: async_channel::unbounded(),
        });
        inner.restore_backoff();
        Self { inner }
    }

    /// A visible view on Reddit's own login page, on the same session (SPEC
    /// §4.4), with the same quiet-page filter as the hidden view. `on_navigated`
    /// runs when the page leaves the login path, and again on later real page
    /// loads in case that was not the end of signing in. Reddit's scripts changing
    /// the address do not run it, so they cannot cause Agora traffic.
    pub fn login_view(&self, on_navigated: impl Fn() + 'static) -> gtk::Widget {
        let manager = UserContentManager::new();
        let view = WebView::builder()
            .network_session(&self.inner.session)
            .user_content_manager(&manager)
            .build();
        let inner = self.inner.clone();
        let on_navigated = Rc::new(on_navigated);
        let left_login = Rc::new(Cell::new(false));
        let last = Rc::new(RefCell::new(String::new()));
        let leave = {
            let (left_login, last, on_navigated) = (left_login.clone(), last.clone(), on_navigated.clone());
            move |uri: &str| {
                if !left_login.replace(true) {
                    last.replace(uri.to_string());
                    // The hidden page was loaded signed out.
                    if inner.home.get() == Home::Loaded {
                        inner.home.set(Home::NotLoaded);
                    }
                    on_navigated();
                }
            }
        };
        view.connect_uri_notify(move |view| {
            let Some(uri) = view.uri() else { return };
            if is_login_url(&uri) {
                left_login.set(false);
            } else {
                leave(&uri);
            }
        });
        view.connect_load_changed(move |view, event| {
            if event != LoadEvent::Finished {
                return;
            }
            let Some(uri) = view.uri() else { return };
            if !is_login_url(&uri) && *last.borrow() != uri.as_str() {
                last.replace(uri.to_string());
                on_navigated();
            }
        });
        view.connect_create(|view, _| {
            // Single sign-on opens a pop-up that talks back to its opener.
            let popup = WebView::builder().related_view(view).build();
            let window = gtk::Window::builder()
                .default_width(520)
                .default_height(640)
                .child(&popup)
                .build();
            if let Some(root) = view.root().and_downcast::<gtk::Window>() {
                window.set_transient_for(Some(&root));
                window.set_modal(true);
            }
            popup.connect_ready_to_show(glib::clone!(
                #[weak]
                window,
                move |_| window.present()
            ));
            popup.connect_close(glib::clone!(
                #[weak]
                window,
                move |_| window.close()
            ));
            Some(popup.upcast())
        });
        let url = format!("{}/login", self.inner.config.origin);
        let widget = view.clone().upcast();
        let inner = self.inner.clone();
        glib::spawn_future_local(async move {
            inner.add_quiet_filter(&manager).await;
            view.load_uri(&url);
        });
        widget
    }

    /// Runs every read operation once in the background lane (SPEC §4.5), and
    /// stops at the first pushback.
    pub async fn self_test(&self) -> Vec<SelfTestLine> {
        let this = &self.inner;
        let lane = Lane::Background;
        let mut lines = Vec::new();
        let mut line = |operation, result: Result<String, AgoraError>| {
            let stop = matches!(result, Err(AgoraError::Blocked { .. } | AgoraError::SessionExpired));
            lines.push(SelfTestLine {
                operation,
                result: result.map_err(|e| e.to_string()),
            });
            !stop
        };

        let who = Inner::whoami(this, lane).await;
        let name = who.as_ref().ok().cloned().flatten();
        let go = line(
            "whoami",
            match who {
                Ok(Some(name)) => Ok(format!("signed in as {name}")),
                Ok(None) => Err(AgoraError::SessionExpired),
                Err(e) => Err(e),
            },
        );
        if !go {
            return lines;
        }

        let request = |feed| ListingRequest {
            feed,
            sort: Sort::Hot,
            after: None,
        };
        let mut first_post = None;
        for (operation, feed) in [("listing (Popular)", Feed::Popular), ("listing (Home)", Feed::Home)] {
            let result = Inner::listing(this, lane, request(feed)).await;
            if let Ok(listing) = &result
                && first_post.is_none()
            {
                first_post = listing.items.iter().find(|p| p.comment_count > 0).cloned();
            }
            if !line(operation, result.map(|l| format!("{} posts", l.items.len()))) {
                return lines;
            }
        }

        if let Some(post) = first_post {
            let result = Inner::subreddit_info(this, lane, post.subreddit.clone()).await;
            if !line(
                "subreddit_info",
                result.map(|s| format!("r/{}, {} rules", s.name, s.rules.len())),
            ) {
                return lines;
            }
            let thread = Inner::thread(
                this,
                lane,
                ThreadRequest {
                    subreddit: post.subreddit.clone(),
                    post: post.id.clone(),
                    sort: CommentSort::Best,
                    focus: None,
                },
            )
            .await;
            let more = thread.as_ref().ok().and_then(|(_, nodes)| first_more(nodes));
            if !line(
                "post_with_comments",
                thread.map(|(p, nodes)| format!("{} ({} top-level)", p.id, nodes.len())),
            ) {
                return lines;
            }
            match more {
                Some(token) => {
                    let result = Inner::more_comments(this, lane, post.id.clone(), token).await;
                    if !line("more_comments", result.map(|n| format!("{} nodes", n.len()))) {
                        return lines;
                    }
                }
                None => {
                    line("more_comments", Ok("skipped: this thread has no more row".into()));
                }
            }
        }

        let result = Inner::subscriptions(this, lane).await;
        if !line("subscriptions", result.map(|s| format!("{} communities", s.len()))) {
            return lines;
        }
        if let Some(name) = name {
            let result = Inner::user_profile(this, lane, name).await;
            if !line(
                "user_profile",
                result.map(|u| format!("{} karma", u.post_karma.unwrap_or(0))),
            ) {
                return lines;
            }
        }
        let result = Inner::notifications(this, lane).await;
        if !line("notifications", result.map(|n| format!("{} shown", n.items.len()))) {
            return lines;
        }
        let result = Inner::search(
            this,
            lane,
            SearchRequest {
                query: "gnome".into(),
                subreddit: None,
                kind: SearchKind::Posts,
                sort: SearchSort::Relevance,
                range: TimeRange::All,
                after: None,
            },
        )
        .await;
        line("search", result.map(|r| format!("{} posts", r.posts.len())));
        lines
    }
}

/// The first "more" row in a tree, depth first.
fn first_more(nodes: &[CommentNode]) -> Option<MoreToken> {
    nodes.iter().find_map(|node| match node {
        CommentNode::More { token, .. } => Some(token.clone()),
        CommentNode::Comment { replies, .. } => first_more(replies),
        CommentNode::Continue { .. } => None,
    })
}

fn random_jitter() -> Duration {
    Duration::from_millis(u64::from(glib::random_int_range(0, 501) as u32))
}

fn operation<T: 'static>(
    inner: &Rc<Inner>,
    run: impl AsyncFnOnce(&Rc<Inner>) -> BackendResult<T> + 'static,
) -> BackendFuture<T> {
    let inner = inner.clone();
    async move { run(&inner).await }.boxed_local()
}

fn unavailable<T: 'static>() -> BackendFuture<T> {
    async { Err(AgoraError::NotAvailable) }.boxed_local()
}

impl Backend for RedditWebBackend {
    fn events(&self) -> async_channel::Receiver<BackendEvent> {
        self.inner.events.1.clone()
    }

    fn whoami(&self) -> BackendFuture<Option<Username>> {
        operation(&self.inner, async |i| Inner::whoami(i, Lane::Interactive).await)
    }

    fn listing(&self, request: ListingRequest) -> BackendFuture<Listing<Post>> {
        operation(&self.inner, async move |i| {
            Inner::listing(i, Lane::Interactive, request).await
        })
    }

    fn post_with_comments(&self, request: ThreadRequest) -> BackendFuture<(Post, Vec<CommentNode>)> {
        operation(&self.inner, async move |i| {
            Inner::thread(i, Lane::Interactive, request).await
        })
    }

    fn more_comments(&self, post: PostId, _sort: CommentSort, token: MoreToken) -> BackendFuture<Vec<CommentNode>> {
        // The token's URL already carries the sort the thread loaded with.
        operation(&self.inner, async move |i| {
            Inner::more_comments(i, Lane::Interactive, post, token).await
        })
    }

    fn subreddit_info(&self, name: SubredditName) -> BackendFuture<Subreddit> {
        operation(&self.inner, async move |i| {
            Inner::subreddit_info(i, Lane::Interactive, name).await
        })
    }

    fn subscriptions(&self) -> BackendFuture<Vec<Subreddit>> {
        operation(&self.inner, async |i| Inner::subscriptions(i, Lane::Background).await)
    }

    fn user_profile(&self, name: Username) -> BackendFuture<User> {
        operation(&self.inner, async move |i| {
            Inner::user_profile(i, Lane::Interactive, name).await
        })
    }

    fn notifications(&self, _after: Option<Cursor>) -> BackendFuture<Listing<Notification>> {
        operation(&self.inner, async |i| Inner::notifications(i, Lane::Background).await)
    }

    fn search(&self, request: SearchRequest) -> BackendFuture<SearchResults> {
        operation(&self.inner, async move |i| {
            Inner::search(i, Lane::Interactive, request).await
        })
    }

    fn sign_out(&self) -> BackendFuture<()> {
        operation(&self.inner, async |i| {
            for waiter in i.scheduler.borrow_mut().drain() {
                let _ = waiter.send(Err(AgoraError::Cancelled));
            }
            i.home.set(Home::NotLoaded);
            i.expired.set(false);
            i.joined.borrow_mut().clear();
            i.view.load_uri("about:blank");
            let manager = i
                .session
                .website_data_manager()
                .ok_or_else(|| AgoraError::Io("no website data manager".into()))?;
            let (sender, receiver) = oneshot::channel();
            manager.clear(
                WebsiteDataTypes::ALL,
                glib::TimeSpan::from_seconds(0),
                gio::Cancellable::NONE,
                move |result| {
                    let _ = sender.send(result);
                },
            );
            receiver
                .await
                .map_err(|_| AgoraError::Io("sign-out did not finish".into()))?
                .map_err(|e| AgoraError::Io(e.message().to_string()))?;
            let _ = i.events.0.try_send(BackendEvent::SignedOut);
            Ok(())
        })
    }

    // Writes reach this backend in slice 4 (SPEC §20).

    fn vote(&self, _target: Thing, _vote: VoteState) -> BackendFuture<()> {
        unavailable()
    }

    fn save(&self, _target: Thing, _saved: bool) -> BackendFuture<()> {
        unavailable()
    }

    fn hide(&self, _post: PostId, _hidden: bool) -> BackendFuture<()> {
        unavailable()
    }

    fn subscribe(&self, _subreddit_id: String, _subscribed: bool) -> BackendFuture<()> {
        unavailable()
    }

    fn mark_read(&self, _notification_ids: Vec<String>) -> BackendFuture<()> {
        unavailable()
    }

    fn submit(&self, _post: NewPost) -> BackendFuture<Post> {
        unavailable()
    }

    fn comment(&self, _parent: Thing, _markdown: String) -> BackendFuture<Comment> {
        unavailable()
    }

    fn edit(&self, _target: Thing, _markdown: String) -> BackendFuture<Body> {
        unavailable()
    }

    fn delete(&self, _target: Thing) -> BackendFuture<()> {
        unavailable()
    }

    fn report(&self, _target: Thing, _reason: String) -> BackendFuture<()> {
        unavailable()
    }
}
