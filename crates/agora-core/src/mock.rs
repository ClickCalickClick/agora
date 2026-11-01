//! `MockBackend`: an in-memory backend with invented sample data.
//!
//! Used by `AGORA_MOCK=1`, UI work and store tests. Nothing here talks to the
//! network. All posts, comments, users and texts are made up for Agora; the
//! subreddit names are only labels.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

use futures::FutureExt;
use futures::future::LocalBoxFuture;

use crate::backend::*;
use crate::error::{AgoraError, BackendResult};
use crate::model::*;

/// Page size of mock listings (SPEC §4.6).
pub const PAGE_SIZE: usize = 25;

/// Reference "now" for mock timestamps: 2026-11-01 12:00 UTC. Mock items are
/// dated relative to it so tests are stable; the UI formats them against the
/// real clock, so sample data looks a few hours or days old.
pub const MOCK_NOW: i64 = 1_793_534_400;

/// The signed-in mock account.
pub const MOCK_USER: &str = "sample_reader";

/// Sleeps for a duration; supplied by the caller so this crate stays free of
/// any main loop. The app passes a `glib::timeout_future`.
pub type Sleeper = Rc<dyn Fn(Duration) -> LocalBoxFuture<'static, ()>>;

/// Which operations fail, for testing error states.
#[derive(Debug, Clone, Default)]
pub struct FailureScript {
    failures: HashMap<&'static str, AgoraError>,
}

impl FailureScript {
    /// Makes every call of `operation` (a SPEC §4.5 name) fail with `error`.
    pub fn fail(mut self, operation: &'static str, error: AgoraError) -> Self {
        self.failures.insert(operation, error);
        self
    }

    /// Parses `AGORA_MOCK_FAIL`, a comma-separated list of `operation=error`
    /// where error is one of `network`, `notfound`, `forbidden`, `broken`,
    /// `blocked`, `expired`. Unknown entries are ignored.
    pub fn from_spec(spec: &str) -> Self {
        let mut script = Self::default();
        for entry in spec.split(',') {
            let Some((op, err)) = entry.trim().split_once('=') else {
                continue;
            };
            let Some(op) = OPERATIONS.iter().copied().find(|o| *o == op) else {
                continue;
            };
            let error = match err {
                "network" => AgoraError::Network("simulated network failure".into()),
                "notfound" => AgoraError::NotFound,
                "forbidden" => AgoraError::Forbidden,
                "broken" => AgoraError::ExtractorBroken { operation: op },
                "blocked" => AgoraError::Blocked {
                    retry_after: Duration::from_secs(300),
                },
                "expired" => AgoraError::SessionExpired,
                _ => continue,
            };
            script.failures.insert(op, error);
        }
        script
    }

    fn check(&self, operation: &str) -> BackendResult<()> {
        match self.failures.get(operation) {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }
}

/// Operation names of SPEC §4.5.
pub const OPERATIONS: &[&str] = &[
    "whoami",
    "listing",
    "post_with_comments",
    "more_comments",
    "subreddit_info",
    "subscriptions",
    "user_profile",
    "notifications",
    "search",
    "vote",
    "save",
    "hide",
    "subscribe",
    "mark_read",
    "submit",
    "comment",
    "edit",
    "delete",
    "report",
];

struct State {
    subreddits: Vec<Subreddit>,
    posts: Vec<Post>,
    /// Full comment trees by post, before "more" truncation.
    threads: HashMap<PostId, Vec<Tree>>,
    hidden: HashSet<PostId>,
    comment_votes: HashMap<CommentId, VoteState>,
    comment_saves: HashSet<CommentId>,
    signed_in: bool,
    next_id: u64,
    /// The clock sample data is relative to.
    now: i64,
}

/// A full comment tree node, before it is cut into what one response returns.
#[derive(Clone)]
struct Tree {
    comment: Comment,
    replies: Vec<Tree>,
}

/// In-memory [`Backend`] with sample data and scripted failures.
pub struct MockBackend {
    state: Rc<RefCell<State>>,
    events: (
        async_channel::Sender<BackendEvent>,
        async_channel::Receiver<BackendEvent>,
    ),
    failures: FailureScript,
    sleeper: Option<Sleeper>,
    latency: Duration,
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MockBackend {
    /// A signed-in mock session with the sample data and no delays.
    pub fn new() -> Self {
        let (subreddits, posts, threads) = sample::build();
        Self {
            state: Rc::new(RefCell::new(State {
                subreddits,
                posts,
                threads,
                hidden: HashSet::new(),
                comment_votes: HashMap::new(),
                comment_saves: HashSet::new(),
                signed_in: true,
                next_id: 1,
                now: MOCK_NOW,
            })),
            events: async_channel::unbounded(),
            failures: FailureScript::default(),
            sleeper: None,
            latency: Duration::ZERO,
        }
    }

    /// Shifts every sample timestamp so that [`MOCK_NOW`] becomes `now`, so
    /// sample items look minutes to days old against the real clock.
    pub fn with_now(self, now: i64) -> Self {
        let shift = now - MOCK_NOW;
        {
            let mut state = self.state.borrow_mut();
            for p in &mut state.posts {
                p.created += shift;
            }
            fn shift_trees(trees: &mut [Tree], shift: i64) {
                for t in trees {
                    t.comment.created += shift;
                    shift_trees(&mut t.replies, shift);
                }
            }
            for trees in state.threads.values_mut() {
                shift_trees(trees, shift);
            }
            state.now = now;
        }
        self
    }

    /// Makes operations fail as scripted.
    pub fn with_failures(mut self, failures: FailureScript) -> Self {
        self.failures = failures;
        self
    }

    /// Delays every operation by `latency` using `sleeper`, so loading states
    /// are visible.
    pub fn with_latency(mut self, sleeper: Sleeper, latency: Duration) -> Self {
        self.sleeper = Some(sleeper);
        self.latency = latency;
        self
    }

    /// Simulates signing out (clears the session) or in.
    pub fn set_signed_in(&self, signed_in: bool) {
        self.state.borrow_mut().signed_in = signed_in;
        let event = if signed_in {
            BackendEvent::SignedIn(Username::new(MOCK_USER))
        } else {
            BackendEvent::SignedOut
        };
        let _ = self.events.0.try_send(event);
    }

    fn op<T: 'static>(
        &self,
        operation: &'static str,
        f: impl FnOnce(&mut State) -> BackendResult<T> + 'static,
    ) -> BackendFuture<T> {
        let state = self.state.clone();
        let check = self.failures.check(operation);
        let events = self.events.0.clone();
        let delay = self.sleeper.clone().map(|s| s(self.latency));
        async move {
            if let Some(delay) = delay {
                delay.await;
            }
            if let Err(e) = check {
                match &e {
                    AgoraError::SessionExpired => {
                        let _ = events.try_send(BackendEvent::SessionExpired);
                    }
                    AgoraError::Blocked { retry_after } => {
                        let _ = events.try_send(BackendEvent::Blocked {
                            retry_after: *retry_after,
                        });
                    }
                    _ => {}
                }
                return Err(e);
            }
            let mut state = state.borrow_mut();
            if !state.signed_in && operation != "whoami" {
                let _ = events.try_send(BackendEvent::SessionExpired);
                return Err(AgoraError::SessionExpired);
            }
            f(&mut state)
        }
        .boxed_local()
    }
}

fn page<T: Clone>(items: &[T], after: Option<&Cursor>) -> Listing<T> {
    let start = after
        .and_then(|c| c.as_str().strip_prefix("offset:"))
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(0)
        .min(items.len());
    let end = (start + PAGE_SIZE).min(items.len());
    Listing {
        items: items[start..end].to_vec(),
        after: (end < items.len()).then(|| Cursor::new(format!("offset:{end}"))),
    }
}

fn sort_posts(posts: &mut [Post], sort: Sort, now: i64) {
    match sort {
        Sort::Best | Sort::Hot => posts.sort_by_key(|p| {
            // Score decayed by age, roughly like a hot ranking.
            let age_h = ((now - p.created).max(0) / 3600) as f64;
            let hot = (p.score.max(1) as f64).log10() - age_h / 12.0;
            (std::cmp::Reverse(p.pinned), std::cmp::Reverse((hot * 1000.0) as i64))
        }),
        Sort::New => posts.sort_by_key(|p| std::cmp::Reverse(p.created)),
        Sort::Top(range) => {
            posts.sort_by_key(|p| std::cmp::Reverse(p.score));
            let _ = range;
        }
        Sort::Rising => posts.sort_by_key(|p| std::cmp::Reverse(p.comment_count as i64 * 10 - (now - p.created) / 600)),
    }
}

fn top_range_seconds(range: TimeRange) -> i64 {
    match range {
        TimeRange::Hour => 3600,
        TimeRange::Day => 86_400,
        TimeRange::Week => 7 * 86_400,
        TimeRange::Month => 30 * 86_400,
        TimeRange::Year => 365 * 86_400,
        TimeRange::All => i64::MAX / 2,
    }
}

/// How many levels and siblings one response includes, like Reddit's cut-off.
const RESPONSE_DEPTH: u32 = 10;
const RESPONSE_SIBLINGS: usize = 8;

fn cut(trees: &[Tree], post: &PostId, base_depth: u32, start: usize) -> Vec<CommentNode> {
    let mut out = Vec::new();
    for (i, tree) in trees.iter().enumerate().skip(start) {
        if i - start >= RESPONSE_SIBLINGS {
            let remaining = &trees[i..];
            out.push(CommentNode::More {
                parent: tree.comment.parent.clone(),
                depth: tree.comment.depth,
                count: remaining.iter().map(count).sum::<u32>(),
                token: MoreToken::new(format!(
                    "{}|{}|{}",
                    post.as_str(),
                    tree.comment.parent.as_ref().map_or("", |p| p.as_str()),
                    i
                )),
            });
            break;
        }
        let replies = if tree.replies.is_empty() {
            Vec::new()
        } else if tree.comment.depth + 1 - base_depth >= RESPONSE_DEPTH {
            vec![CommentNode::Continue {
                parent: tree.comment.id.clone(),
                depth: tree.comment.depth + 1,
            }]
        } else {
            cut(&tree.replies, post, base_depth, 0)
        };
        out.push(CommentNode::Comment {
            comment: tree.comment.clone(),
            replies,
        });
    }
    out
}

fn count(tree: &Tree) -> u32 {
    1 + tree.replies.iter().map(count).sum::<u32>()
}

fn find<'a>(trees: &'a [Tree], id: &CommentId) -> Option<&'a Tree> {
    for t in trees {
        if &t.comment.id == id {
            return Some(t);
        }
        if let Some(found) = find(&t.replies, id) {
            return Some(found);
        }
    }
    None
}

fn find_mut<'a>(trees: &'a mut [Tree], id: &CommentId) -> Option<&'a mut Tree> {
    for t in trees {
        if &t.comment.id == id {
            return Some(t);
        }
        if let Some(found) = find_mut(&mut t.replies, id) {
            return Some(found);
        }
    }
    None
}

fn apply_comment_state(trees: &mut [Tree], votes: &HashMap<CommentId, VoteState>, saves: &HashSet<CommentId>) {
    for t in trees {
        if let Some(v) = votes.get(&t.comment.id) {
            let base = t.comment.score.unwrap_or(0) - t.comment.vote.delta();
            t.comment.vote = *v;
            t.comment.score = t.comment.score.map(|_| base + v.delta());
        }
        t.comment.saved = saves.contains(&t.comment.id);
        apply_comment_state(&mut t.replies, votes, saves);
    }
}

impl Backend for MockBackend {
    fn events(&self) -> async_channel::Receiver<BackendEvent> {
        self.events.1.clone()
    }

    fn whoami(&self) -> BackendFuture<Option<Username>> {
        self.op("whoami", |s| Ok(s.signed_in.then(|| Username::new(MOCK_USER))))
    }

    fn listing(&self, request: ListingRequest) -> BackendFuture<Listing<Post>> {
        self.op("listing", move |s| {
            let subscribed: HashSet<String> = s
                .subreddits
                .iter()
                .filter(|r| r.subscribed)
                .map(|r| r.name.as_str().to_ascii_lowercase())
                .collect();
            let mut posts: Vec<Post> = s
                .posts
                .iter()
                .filter(|p| !s.hidden.contains(&p.id))
                .filter(|p| match &request.feed {
                    Feed::Home => subscribed.contains(&p.subreddit.as_str().to_ascii_lowercase()),
                    Feed::Popular => p.score >= 40,
                    Feed::Subreddit(name) => p.subreddit.eq_ignore_case(name),
                    Feed::User(name) => p.author.as_ref() == Some(name),
                    Feed::Saved => p.saved,
                })
                .filter(|p| match request.sort {
                    Sort::Top(range) => s.now - p.created <= top_range_seconds(range),
                    _ => true,
                })
                .cloned()
                .collect();
            if let Feed::Subreddit(name) = &request.feed
                && !s.subreddits.iter().any(|r| r.name.eq_ignore_case(name))
            {
                return Err(AgoraError::NotFound);
            }
            if !matches!(request.feed, Feed::Subreddit(_)) {
                // Pinned posts stay pinned only inside their subreddit.
                for p in &mut posts {
                    p.pinned = false;
                }
            }
            if request.feed != Feed::Saved {
                sort_posts(&mut posts, request.sort, s.now);
            }
            Ok(page(&posts, request.after.as_ref()))
        })
    }

    fn post_with_comments(&self, request: ThreadRequest) -> BackendFuture<(Post, Vec<CommentNode>)> {
        self.op("post_with_comments", move |s| {
            let post = s
                .posts
                .iter()
                .find(|p| p.id == request.post)
                .cloned()
                .ok_or(AgoraError::NotFound)?;
            let (votes, saves) = (s.comment_votes.clone(), s.comment_saves.clone());
            let trees = s.threads.entry(request.post.clone()).or_default();
            apply_comment_state(trees, &votes, &saves);
            let mut trees = trees.clone();
            sort_trees(&mut trees, request.sort);
            let nodes = match &request.focus {
                Some(id) => {
                    let root = find(&trees, id).ok_or(AgoraError::NotFound)?;
                    cut(std::slice::from_ref(root), &post.id, root.comment.depth, 0)
                }
                None => cut(&trees, &post.id, 0, 0),
            };
            Ok((post, nodes))
        })
    }

    fn more_comments(&self, post: PostId, sort: CommentSort, token: MoreToken) -> BackendFuture<Vec<CommentNode>> {
        self.op("more_comments", move |s| {
            let mut parts = token.as_str().split('|');
            let (Some(p), Some(parent), Some(start)) = (parts.next(), parts.next(), parts.next()) else {
                return Err(AgoraError::Parse("bad more token".into()));
            };
            if p != post.as_str() {
                return Err(AgoraError::Parse("more token is for another post".into()));
            }
            let start: usize = start.parse().map_err(|_| AgoraError::Parse("bad more token".into()))?;
            let trees = s.threads.get(&post).ok_or(AgoraError::NotFound)?;
            let siblings = if parent.is_empty() {
                trees.as_slice()
            } else {
                &find(trees, &CommentId::new(parent))
                    .ok_or(AgoraError::NotFound)?
                    .replies
            };
            // The token's index is into the sorted siblings, as `cut` saw them.
            let mut siblings = siblings.to_vec();
            sort_trees(&mut siblings, sort);
            let base = siblings.first().map_or(0, |t| t.comment.depth);
            Ok(cut(&siblings, &post, base.saturating_sub(2), start))
        })
    }

    fn subreddit_info(&self, name: SubredditName) -> BackendFuture<Subreddit> {
        self.op("subreddit_info", move |s| {
            s.subreddits
                .iter()
                .find(|r| r.name.eq_ignore_case(&name))
                .cloned()
                .ok_or(AgoraError::NotFound)
        })
    }

    fn subscriptions(&self) -> BackendFuture<Vec<Subreddit>> {
        self.op("subscriptions", |s| {
            let mut subs: Vec<Subreddit> = s.subreddits.iter().filter(|r| r.subscribed).cloned().collect();
            subs.sort_by_key(|r| r.name.as_str().to_ascii_lowercase());
            Ok(subs)
        })
    }

    fn user_profile(&self, name: Username) -> BackendFuture<User> {
        self.op("user_profile", move |s| {
            let posts = s.posts.iter().filter(|p| p.author.as_ref() == Some(&name)).count() as i64;
            if posts == 0 && name.as_str() != MOCK_USER && !sample::USERS.contains(&name.as_str()) {
                return Err(AgoraError::NotFound);
            }
            let seed = name.as_str().bytes().map(i64::from).sum::<i64>();
            Ok(User {
                name,
                post_karma: Some(posts * 137 + seed * 3),
                comment_karma: Some(seed * 41),
                created: Some(s.now - seed * 86_400 * 3),
                avatar_url: None,
            })
        })
    }

    fn notifications(&self, after: Option<Cursor>) -> BackendFuture<Listing<Notification>> {
        self.op("notifications", move |_| {
            Ok(page(&sample::notifications(), after.as_ref()))
        })
    }

    fn search(&self, request: SearchRequest) -> BackendFuture<SearchResults> {
        self.op("search", move |s| {
            let q = request.query.to_lowercase();
            let mut results = SearchResults::default();
            match request.kind {
                SearchKind::Posts => {
                    let posts: Vec<Post> = s
                        .posts
                        .iter()
                        .filter(|p| request.subreddit.as_ref().is_none_or(|r| p.subreddit.eq_ignore_case(r)))
                        .filter(|p| p.title.to_lowercase().contains(&q))
                        .cloned()
                        .collect();
                    let listing = page(&posts, request.after.as_ref());
                    results.posts = listing.items;
                    results.after = listing.after;
                }
                SearchKind::Subreddits => {
                    results.subreddits = s
                        .subreddits
                        .iter()
                        .filter(|r| r.name.as_str().to_lowercase().contains(&q))
                        .cloned()
                        .collect();
                }
                SearchKind::Users => {
                    results.users = sample::USERS
                        .iter()
                        .filter(|u| u.contains(&q))
                        .map(|u| User {
                            name: Username::new(*u),
                            post_karma: None,
                            comment_karma: None,
                            created: None,
                            avatar_url: None,
                        })
                        .collect();
                }
            }
            Ok(results)
        })
    }

    fn vote(&self, target: Thing, vote: VoteState) -> BackendFuture<()> {
        self.op("vote", move |s| {
            match target {
                Thing::Post(id) => {
                    let p = s.posts.iter_mut().find(|p| p.id == id).ok_or(AgoraError::NotFound)?;
                    p.score += vote.delta() - p.vote.delta();
                    p.vote = vote;
                }
                Thing::Comment(id) => {
                    s.comment_votes.insert(id, vote);
                }
            }
            Ok(())
        })
    }

    fn save(&self, target: Thing, saved: bool) -> BackendFuture<()> {
        self.op("save", move |s| {
            match target {
                Thing::Post(id) => {
                    s.posts
                        .iter_mut()
                        .find(|p| p.id == id)
                        .ok_or(AgoraError::NotFound)?
                        .saved = saved;
                }
                Thing::Comment(id) => {
                    if saved {
                        s.comment_saves.insert(id);
                    } else {
                        s.comment_saves.remove(&id);
                    }
                }
            }
            Ok(())
        })
    }

    fn hide(&self, post: PostId, hidden: bool) -> BackendFuture<()> {
        self.op("hide", move |s| {
            if hidden {
                s.hidden.insert(post);
            } else {
                s.hidden.remove(&post);
            }
            Ok(())
        })
    }

    fn subscribe(&self, subreddit: Subreddit, subscribed: bool) -> BackendFuture<()> {
        self.op("subscribe", move |s| {
            s.subreddits
                .iter_mut()
                .find(|r| r.name.eq_ignore_case(&subreddit.name))
                .ok_or(AgoraError::NotFound)?
                .subscribed = subscribed;
            Ok(())
        })
    }

    fn mark_read(&self, _notification_ids: Vec<String>) -> BackendFuture<()> {
        self.op("mark_read", |_| Ok(()))
    }

    fn submit(&self, new: NewPost) -> BackendFuture<Post> {
        self.op("submit", move |s| {
            s.next_id += 1;
            let id = PostId::new(format!("t3_new{}", s.next_id));
            let (kind, body) = match new.content {
                NewPostContent::Text(t) => (PostKind::Text, Some(Body::Markdown(t))),
                NewPostContent::Link(url) => {
                    let domain = sample::domain_of(&url);
                    (PostKind::Link { url, domain }, None)
                }
            };
            let post = Post {
                permalink: format!("/r/{}/comments/{}/", new.subreddit, &id.as_str()[3..]),
                id,
                subreddit: new.subreddit,
                author: Some(Username::new(MOCK_USER)),
                title: new.title,
                kind,
                body,
                created: s.now,
                score: 1,
                comment_count: 0,
                vote: VoteState::Up,
                saved: false,
                flair: None,
                nsfw: new.nsfw,
                spoiler: new.spoiler,
                pinned: false,
                locked: false,
                oc: false,
                edited: false,
                thumbnail: None,
            };
            s.posts.insert(0, post.clone());
            Ok(post)
        })
    }

    fn comment(&self, parent: Thing, markdown: String) -> BackendFuture<Comment> {
        self.op("comment", move |s| {
            s.next_id += 1;
            let id = CommentId::new(format!("t1_new{}", s.next_id));
            let (post_id, parent_id, depth) = match &parent {
                Thing::Post(p) => (p.clone(), None, 0),
                Thing::Comment(c) => {
                    let (post, depth) = s
                        .threads
                        .iter()
                        .find_map(|(p, t)| find(t, c).map(|t| (p.clone(), t.comment.depth + 1)))
                        .ok_or(AgoraError::NotFound)?;
                    (post, Some(c.clone()), depth)
                }
            };
            let comment = Comment {
                id,
                post_id: post_id.clone(),
                parent: parent_id.clone(),
                author: Some(Username::new(MOCK_USER)),
                body: Body::Markdown(markdown),
                created: s.now,
                score: Some(1),
                vote: VoteState::Up,
                saved: false,
                is_submitter: false,
                pinned: false,
                edited: false,
                state: CommentState::Normal,
                depth,
            };
            let tree = Tree {
                comment: comment.clone(),
                replies: Vec::new(),
            };
            let trees = s.threads.entry(post_id).or_default();
            match parent_id {
                None => trees.insert(0, tree),
                Some(pid) => find_mut(trees, &pid)
                    .ok_or(AgoraError::NotFound)?
                    .replies
                    .insert(0, tree),
            }
            Ok(comment)
        })
    }

    fn edit(&self, target: Thing, markdown: String) -> BackendFuture<Body> {
        self.op("edit", move |s| {
            let body = Body::Markdown(markdown);
            match target {
                Thing::Post(id) => {
                    let p = s.posts.iter_mut().find(|p| p.id == id).ok_or(AgoraError::NotFound)?;
                    p.body = Some(body.clone());
                    p.edited = true;
                }
                Thing::Comment(id) => {
                    let t = s
                        .threads
                        .values_mut()
                        .find_map(|t| find_mut(t, &id))
                        .ok_or(AgoraError::NotFound)?;
                    t.comment.body = body.clone();
                    t.comment.edited = true;
                }
            }
            Ok(body)
        })
    }

    fn delete(&self, target: Thing) -> BackendFuture<()> {
        self.op("delete", move |s| {
            match target {
                Thing::Post(id) => s.posts.retain(|p| p.id != id),
                Thing::Comment(id) => {
                    let t = s
                        .threads
                        .values_mut()
                        .find_map(|t| find_mut(t, &id))
                        .ok_or(AgoraError::NotFound)?;
                    t.comment.state = CommentState::Deleted;
                    t.comment.author = None;
                    t.comment.body = Body::Markdown(String::new());
                }
            }
            Ok(())
        })
    }

    fn sign_out(&self) -> BackendFuture<()> {
        let events = self.events.0.clone();
        self.op("whoami", move |s| {
            s.signed_in = false;
            let _ = events.try_send(BackendEvent::SignedOut);
            Ok(())
        })
    }

    fn report(&self, _target: Thing, _reason: String) -> BackendFuture<()> {
        self.op("report", |_| Ok(()))
    }
}

fn sort_trees(trees: &mut [Tree], sort: CommentSort) {
    match sort {
        CommentSort::Best | CommentSort::Top | CommentSort::Controversial | CommentSort::Qa => trees.sort_by_key(|t| {
            (
                std::cmp::Reverse(t.comment.pinned),
                std::cmp::Reverse(t.comment.score.unwrap_or(0)),
            )
        }),
        CommentSort::New => trees.sort_by_key(|t| std::cmp::Reverse(t.comment.created)),
        CommentSort::Old => trees.sort_by_key(|t| t.comment.created),
    }
    for t in trees {
        sort_trees(&mut t.replies, sort);
    }
}

mod sample;

#[cfg(test)]
mod tests;
