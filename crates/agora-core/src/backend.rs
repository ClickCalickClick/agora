//! The `Backend` trait: the only way the UI reaches Reddit (SPEC §4.3).

use std::time::Duration;

use futures::future::LocalBoxFuture;

use crate::error::BackendResult;
use crate::model::*;

/// Future returned by every [`Backend`] operation. Not `Send`: backends live on
/// the GTK main thread and are polled on the GLib main context.
pub type BackendFuture<T> = LocalBoxFuture<'static, BackendResult<T>>;

/// Something the backend reports outside a single operation's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendEvent {
    /// The user signed in as this account.
    SignedIn(Username),
    /// The user signed out, or the session was cleared.
    SignedOut,
    /// An operation found the session logged out.
    SessionExpired,
    /// Reddit pushed back; traffic stops for `retry_after`.
    Blocked {
        /// Back-off before the next request.
        retry_after: Duration,
    },
}

/// Arguments of `listing`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ListingRequest {
    /// Which feed.
    pub feed: Feed,
    /// Sort order (ignored for [`Feed::Saved`]).
    pub sort: Sort,
    /// `None` for the first page.
    pub after: Option<Cursor>,
}

/// Arguments of `post_with_comments`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThreadRequest {
    /// The post.
    pub post: PostId,
    /// Comment order.
    pub sort: CommentSort,
    /// Load the thread rooted at this comment ("Continue this thread", or a
    /// comment permalink); `None` for the whole thread.
    pub focus: Option<CommentId>,
}

/// Arguments of `search`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchRequest {
    /// The text the user typed.
    pub query: String,
    /// Limit to one subreddit; `None` searches all of Reddit.
    pub subreddit: Option<SubredditName>,
    /// Posts, subreddits or users.
    pub kind: SearchKind,
    /// Post result order.
    pub sort: SearchSort,
    /// Time range for post results.
    pub range: TimeRange,
    /// `None` for the first page.
    pub after: Option<Cursor>,
}

/// The operations of SPEC §4.5. Every method returns a `'static` future, so
/// callers clone their `Rc<dyn Backend>` into the task that awaits it.
///
/// Writes are confirmed from the response of the request that made them; there
/// is no separate read-back.
pub trait Backend {
    /// Events outside single operations (sign-in, expiry, pushback). Every call
    /// returns a receiver of the same channel, and each event goes to one
    /// receiver only: the app has a single listener, `SessionStore`.
    fn events(&self) -> async_channel::Receiver<BackendEvent>;

    // Reads.

    /// The signed-in account, or `None` when signed out.
    fn whoami(&self) -> BackendFuture<Option<Username>>;
    /// One page of a feed.
    fn listing(&self, request: ListingRequest) -> BackendFuture<Listing<Post>>;
    /// A post and its first comments.
    fn post_with_comments(&self, request: ThreadRequest) -> BackendFuture<(Post, Vec<CommentNode>)>;
    /// The comments behind one "N more replies" row, in the sort the thread
    /// was loaded with (Reddit's partial takes the sort too).
    fn more_comments(&self, post: PostId, sort: CommentSort, token: MoreToken) -> BackendFuture<Vec<CommentNode>>;
    /// A subreddit's header, description and rules.
    fn subreddit_info(&self, name: SubredditName) -> BackendFuture<Subreddit>;
    /// The user's subscriptions.
    fn subscriptions(&self) -> BackendFuture<Vec<Subreddit>>;
    /// A user profile.
    fn user_profile(&self, name: Username) -> BackendFuture<User>;
    /// One page of notifications.
    fn notifications(&self, after: Option<Cursor>) -> BackendFuture<Listing<Notification>>;
    /// One page of search results.
    fn search(&self, request: SearchRequest) -> BackendFuture<SearchResults>;

    /// Signs out: clears the web session's data (SPEC §4.4) and emits
    /// [`BackendEvent::SignedOut`].
    fn sign_out(&self) -> BackendFuture<()>;

    // Writes.

    /// Votes on a post or comment.
    fn vote(&self, target: Thing, vote: VoteState) -> BackendFuture<()>;
    /// Saves or unsaves a post or comment.
    fn save(&self, target: Thing, saved: bool) -> BackendFuture<()>;
    /// Hides or unhides a post.
    fn hide(&self, post: PostId, hidden: bool) -> BackendFuture<()>;
    /// Joins or leaves a subreddit.
    fn subscribe(&self, subreddit: Subreddit, subscribed: bool) -> BackendFuture<()>;
    /// Marks notifications read.
    fn mark_read(&self, notification_ids: Vec<String>) -> BackendFuture<()>;
    /// Submits a new post (posting rules, SPEC §4.5).
    fn submit(&self, post: NewPost) -> BackendFuture<Post>;
    /// Replies to a post or comment with Markdown text (posting rules, SPEC §4.5).
    fn comment(&self, parent: Thing, markdown: String) -> BackendFuture<Comment>;
    /// Replaces the text of the user's own post or comment.
    fn edit(&self, target: Thing, markdown: String) -> BackendFuture<Body>;
    /// Deletes the user's own post or comment.
    fn delete(&self, target: Thing) -> BackendFuture<()>;
    /// Reports a post or comment with a reason as Reddit words it.
    fn report(&self, target: Thing, reason: String) -> BackendFuture<()>;
}
