//! Bridge integration (SPEC §19): a local server plays reddit.com from the
//! recorded fixtures to a real hidden WebView, so `js/*` and the parsers are
//! tested end to end without touching Reddit. Run under `xvfb-run`.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agora_core::*;
use agora_reddit_web::{Config, RedditWebBackend};
use gtk::glib;

const PAGE: &str = "<!doctype html><title>home</title>\
    <faceplate-partial src=\"/svc/shreddit/partial/x/common-left-nav?sig=abc\"></faceplate-partial>\
    <script>fetch('/probe?agora=' + typeof window.agora + '&handler=' +\
    typeof (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.agora));</script>";

#[derive(Debug, Clone)]
struct Seen {
    at: Instant,
    method: String,
    path: String,
    accept: String,
    body: String,
}

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Seen>>>);

impl Log {
    fn all(&self) -> Vec<Seen> {
        self.0.lock().unwrap().clone()
    }

    /// Requests the app made, without the page's own probe and favicon.
    fn calls(&self) -> Vec<Seen> {
        self.all()
            .into_iter()
            .filter(|s| !s.path.starts_with("/probe") && s.path != "/favicon.ico")
            .collect()
    }
}

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

fn respond(path: &str) -> (u16, Vec<(&'static str, String)>, String) {
    let html = |name: &str| (200, Vec::new(), fixture(name));
    let bare = path.split('?').next().unwrap();
    match bare {
        "/r/popular/" => (
            200,
            vec![("Set-Cookie", "csrf_token=tok123; Path=/".to_string())],
            PAGE.to_string(),
        ),
        "/user/me/" => (
            302,
            vec![("Location", "/user/agora_tester/".to_string())],
            String::new(),
        ),
        "/user/agora_tester/" => html("user_profile_own.html"),
        "/svc/shreddit/community-more-posts/hot/" if path.contains("name=gone") => {
            (302, vec![("Location", "/login/?dest=x".to_string())], String::new())
        }
        "/login" | "/login/" => (200, Vec::new(), "<html>log in</html>".into()),
        "/svc/shreddit/community-more-posts/hot/" if path.contains("name=blockme") => {
            (403, Vec::new(), fixture("blocked_network_security.html"))
        }
        "/svc/shreddit/community-more-posts/hot/" if path.contains("name=slow") => (429, Vec::new(), String::new()),
        "/svc/shreddit/community-more-posts/hot/" => html("listing_subreddit.html"),
        "/r/gnome/comments/1xkeknc/" => html("listing_subreddit.html"),
        "/r/gnome/comments/other/" => (
            200,
            Vec::new(),
            r#"<shreddit-post id="t3_other" post-title="Other" subreddit-name="gnome"></shreddit-post>"#.to_string(),
        ),
        "/svc/shreddit/comments/r/gnome/1xkeknc" => html("post_comments.html"),
        "/svc/shreddit/comments/r/gnome/other" => html("post_comments.html"),
        "/svc/shreddit/more-comments/r/gnome/t3_1xkeknc" => html("more_comments.html"),
        "/svc/shreddit/feeds/popular-feed" | "/svc/shreddit/feeds/home-feed" => html("listing_subreddit.html"),
        "/svc/shreddit/feeds/subreddit-right-rail" => html("subreddit_info_right_rail.html"),
        "/svc/shreddit/partial/x/common-left-nav" => html("subscriptions_left_nav_empty.html"),
        "/notifications" => html("listing_subreddit.html"),
        "/search/" => html("search_posts.html"),
        _ => (404, Vec::new(), "nope".into()),
    }
}

fn serve(mut stream: TcpStream, log: Log) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.is_empty() {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let mut headers = HashMap::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = header.split_once(':') {
            headers.insert(k.to_ascii_lowercase(), v.trim().to_string());
        }
    }
    let length: usize = headers.get("content-length").and_then(|l| l.parse().ok()).unwrap_or(0);
    let mut body = vec![0; length];
    let _ = reader.read_exact(&mut body);
    log.0.lock().unwrap().push(Seen {
        at: Instant::now(),
        method,
        path: path.clone(),
        accept: headers.get("accept").cloned().unwrap_or_default(),
        body: String::from_utf8_lossy(&body).into_owned(),
    });
    let (status, extra, body) = respond(&path);
    let mut head = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (k, v) in extra {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
}

fn start_server() -> (String, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let log = Log::default();
    let for_thread = log.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let log = for_thread.clone();
            std::thread::spawn(move || serve(stream, log));
        }
    });
    (origin, log)
}

fn backend(origin: &str, dir: &std::path::Path, name: &str) -> RedditWebBackend {
    let root = dir.join(name);
    RedditWebBackend::new(Config {
        origin: origin.to_string(),
        home_path: "/r/popular/".into(),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
        backoff_file: root.join("backoff.json"),
    })
}

fn listing(sub: &str) -> ListingRequest {
    ListingRequest {
        feed: Feed::Subreddit(SubredditName::new(sub)),
        sort: Sort::Hot,
        after: None,
    }
}

fn first_more(nodes: &[CommentNode]) -> Option<MoreToken> {
    nodes.iter().find_map(|n| match n {
        CommentNode::More { token, .. } => Some(token.clone()),
        CommentNode::Comment { replies, .. } => first_more(replies),
        CommentNode::Continue { .. } => None,
    })
}

/// Every gap between requests: one burst gap at most after a view change.
fn check_pacing(calls: &[Seen]) {
    // A followed redirect is one request.
    let calls: Vec<&Seen> = calls
        .iter()
        .filter(|s| s.path != "/user/agora_tester/" || s.method != "GET")
        .collect();
    for pair in calls.windows(2) {
        let gap = pair[1].at.duration_since(pair[0].at);
        assert!(
            gap >= Duration::from_millis(240),
            "{} then {} after {gap:?}",
            pair[0].path,
            pair[1].path
        );
    }
}

async fn reading(origin: String, log: Log, dir: PathBuf) {
    let b = backend(&origin, &dir, "reading");

    assert_eq!(b.whoami().await.unwrap(), Some(Username::new("agora_tester")));
    // The page's scripts cannot see the bridge or its handler.
    let probe = log
        .all()
        .into_iter()
        .find(|s| s.path.starts_with("/probe"))
        .expect("probe ran");
    assert_eq!(probe.path, "/probe?agora=undefined&handler=undefined");

    // 129 KB, so the body arrives in chunks.
    let page = b.listing(listing("gnome")).await.unwrap();
    assert_eq!(page.items.len(), 3);
    let call = log.calls().pop().unwrap();
    assert!(
        call.path
            .starts_with("/svc/shreddit/community-more-posts/hot/?name=gnome")
    );
    assert_eq!(call.accept, "text/vnd.reddit.partial+html, text/html;q=0.9");

    // The next page requests the URL the first page named.
    let after = page.after.unwrap();
    b.listing(ListingRequest {
        after: Some(after.clone()),
        ..listing("gnome")
    })
    .await
    .unwrap();
    assert_eq!(log.calls().pop().unwrap().path, after.as_str().replace("&amp;", "&"));

    let info = b.subreddit_info(SubredditName::new("gnome")).await.unwrap();
    assert_eq!(info.id, "t5_2qjhn");

    let (post, nodes) = b
        .post_with_comments(ThreadRequest {
            subreddit: SubredditName::new("gnome"),
            post: PostId::new("t3_1xkeknc"),
            sort: CommentSort::Top,
            focus: None,
        })
        .await
        .unwrap();
    assert_eq!(post.id.as_str(), "t3_1xkeknc");
    assert!(!nodes.is_empty());
    let tree = log.calls().pop().unwrap();
    assert_eq!(
        tree.path,
        "/svc/shreddit/comments/r/gnome/1xkeknc?render-mode=partial&sort=TOP"
    );

    // "More" posts its cursor with the CSRF cookie's value.
    let token = first_more(&nodes).unwrap_or_else(|| {
        let json = serde_json::json!({
            "src": "/svc/shreddit/more-comments/r/gnome/t3_1xkeknc?sort=TOP&startingDepth=3",
            "cursor": "abc=",
        });
        MoreToken::new(json.to_string())
    });
    let more = b
        .more_comments(PostId::new("t3_1xkeknc"), CommentSort::Top, token)
        .await
        .unwrap();
    assert!(!more.is_empty());
    let post_call = log.calls().pop().unwrap();
    assert_eq!(post_call.method, "POST");
    assert!(post_call.body.contains("csrf_token=tok123"), "{}", post_call.body);
    assert!(post_call.body.contains("cursor="), "{}", post_call.body);

    assert_eq!(b.subscriptions().await.unwrap(), vec![]);
    assert_eq!(
        log.calls().pop().unwrap().path,
        "/svc/shreddit/partial/x/common-left-nav?sig=abc"
    );

    // Writes are not wired up before slice 4, and send nothing.
    let before = log.calls().len();
    assert_eq!(
        b.vote(Thing::Post(post.id), VoteState::Up).await,
        Err(AgoraError::NotAvailable)
    );
    assert_eq!(log.calls().len(), before);

    check_pacing(&log.calls());

    // Self-test: every read once, nothing failing.
    let report = b.self_test().await;
    let failed: Vec<_> = report.iter().filter(|l| l.result.is_err()).collect();
    assert!(failed.is_empty(), "{failed:?}");
    check_pacing(&log.calls());
}

async fn dropping_a_view(origin: String, log: Log, dir: PathBuf) {
    let b = backend(&origin, &dir, "dropping");
    b.whoami().await.unwrap();
    let before = log.calls().len();
    let thread = |post: &str| ThreadRequest {
        subreddit: SubredditName::new("gnome"),
        post: PostId::new(post),
        sort: CommentSort::Best,
        focus: None,
    };
    // The first waits behind the pacing gap, so the second replaces it unsent.
    let first = b.post_with_comments(thread("t3_1xkeknc"));
    let second = b.post_with_comments(thread("t3_other"));
    let (first, second) = futures::join!(first, second);
    assert_eq!(first.unwrap_err(), AgoraError::Cancelled);
    assert!(second.is_ok());
    assert!(!log.calls()[before..].iter().any(|s| s.path.contains("1xkeknc")));
}

async fn pushback(origin: String, log: Log, dir: PathBuf, page: &str) {
    let name = format!("block-{page}");
    let b = backend(&origin, &dir, &name);
    let events = b.events();
    let result = b.listing(listing(page)).await;
    let Err(AgoraError::Blocked { retry_after }) = result else {
        panic!("{result:?}")
    };
    assert_eq!(retry_after, Duration::from_secs(300));
    assert_eq!(events.try_recv().unwrap(), BackendEvent::Blocked { retry_after });

    // Nothing else leaves the machine while stopped.
    let before = log.calls().len();
    let again = b.listing(listing("gnome")).await;
    assert!(matches!(again, Err(AgoraError::Blocked { .. })), "{again:?}");
    assert!(matches!(b.whoami().await, Err(AgoraError::Blocked { .. })));
    assert_eq!(log.calls().len(), before);

    // Restarting the app does not shorten it.
    let reopened = backend(&origin, &dir, &name);
    assert!(matches!(reopened.whoami().await, Err(AgoraError::Blocked { .. })));
    assert_eq!(log.calls().len(), before);
}

async fn signed_out(origin: String, log: Log, dir: PathBuf) {
    let b = backend(&origin, &dir, "expired");
    let events = b.events();
    assert_eq!(
        b.listing(listing("gone")).await.unwrap_err(),
        AgoraError::SessionExpired
    );
    assert_eq!(events.try_recv().unwrap(), BackendEvent::SessionExpired);
    let before = log.calls().len();
    assert_eq!(
        b.listing(listing("gnome")).await.unwrap_err(),
        AgoraError::SessionExpired,
        "no requests until a new sign-in"
    );
    assert_eq!(log.calls().len(), before);
    // whoami may still ask, and clears the state when it finds an account.
    assert!(b.whoami().await.unwrap().is_some());
    assert!(b.listing(listing("gnome")).await.is_ok());

    // Signing in again after an expiry, with the home page not loaded yet.
    let fresh = backend(&origin, &dir, "expired-again");
    let _ = fresh.listing(listing("gone")).await;
    let page_loads = |log: &Log| log.calls().iter().filter(|s| s.path == "/r/popular/").count();
    let before = page_loads(&log);
    let view = fresh.login_view(|| {});
    glib::timeout_future(Duration::from_millis(1500)).await;
    drop(view);
    assert!(
        fresh.whoami().await.unwrap().is_some(),
        "whoami must get past the home page"
    );
    assert!(fresh.listing(listing("gnome")).await.is_ok());
    assert!(page_loads(&log) <= before + 1, "no extra page loads");
}

fn main() {
    if gtk::init().is_err() {
        println!("bridge: no display, skipped (run under xvfb-run)");
        return;
    }
    let (origin, log) = start_server();
    let dir = std::env::temp_dir().join(format!("agora-bridge-{}", std::process::id()));
    let main_loop = glib::MainLoop::new(None, false);
    let quit = main_loop.clone();
    let cleanup = dir.clone();
    let failed = Arc::new(Mutex::new(false));
    let failed_flag = failed.clone();
    glib::spawn_future_local(async move {
        let run = async {
            println!("bridge: reading");
            reading(origin.clone(), log.clone(), dir.clone()).await;
            println!("bridge: dropping a view");
            dropping_a_view(origin.clone(), log.clone(), dir.clone()).await;
            println!("bridge: signed out");
            signed_out(origin.clone(), log.clone(), dir.clone()).await;
            println!("bridge: pushback (block page)");
            pushback(origin.clone(), log.clone(), dir.clone(), "blockme").await;
            println!("bridge: pushback (429)");
            pushback(origin.clone(), log.clone(), dir.clone(), "slow").await;
        };
        run.await;
        *failed_flag.lock().unwrap() = false;
        quit.quit();
    });
    // A panic inside the task ends the process with a failure; a hang ends here.
    glib::timeout_add_seconds_local_once(180, || panic!("bridge test timed out"));
    main_loop.run();
    let _ = std::fs::remove_dir_all(cleanup);
    assert!(!*failed.lock().unwrap());
    println!("bridge: ok");
}
