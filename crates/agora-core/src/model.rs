//! Reddit models shared by the backend and the UI.

use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! name_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            /// Wraps a value as Reddit gives it.
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// The value as Reddit gives it.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

name_type!(
    /// A subreddit name without the `r/` prefix, in the case Reddit shows it.
    /// Compare with [`SubredditName::eq_ignore_case`]: Reddit names are case-insensitive.
    SubredditName
);
name_type!(
    /// A username without the `u/` prefix.
    Username
);
name_type!(
    /// A post's full name, `t3_…`.
    PostId
);
name_type!(
    /// A comment's full name, `t1_…`.
    CommentId
);
name_type!(
    /// An opaque "next page" cursor returned by a listing.
    Cursor
);
name_type!(
    /// An opaque token that loads one "N more replies" row (`more_comments`).
    MoreToken
);

impl SubredditName {
    /// Case-insensitive comparison, as Reddit compares subreddit names.
    pub fn eq_ignore_case(&self, other: &SubredditName) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }

    /// `r/name`.
    pub fn prefixed(&self) -> String {
        format!("r/{}", self.0)
    }
}

impl Username {
    /// `u/name`.
    pub fn prefixed(&self) -> String {
        format!("u/{}", self.0)
    }
}

/// A post or comment, for operations that accept either.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Thing {
    /// A post.
    Post(PostId),
    /// A comment.
    Comment(CommentId),
}

/// The text of a post or comment, in the format the source gave it.
/// `agora-markdown` turns either into the same document AST (SPEC §4.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Body {
    /// Reddit-flavored Markdown (composer, mock data).
    Markdown(String),
    /// HTML as reddit.com's partials serve it.
    Html(String),
}

/// The user's vote on a post or comment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VoteState {
    /// Upvoted.
    Up,
    /// Not voted.
    #[default]
    None,
    /// Downvoted.
    Down,
}

impl VoteState {
    /// The score change this vote contributes compared with no vote.
    pub fn delta(self) -> i64 {
        match self {
            Self::Up => 1,
            Self::None => 0,
            Self::Down => -1,
        }
    }
}

/// What a post contains.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostKind {
    /// A text (self) post; the text is in [`Post::body`].
    Text,
    /// A link to another site.
    Link {
        /// The link target.
        url: String,
        /// The host name shown on the link card.
        domain: String,
    },
    /// A single image (slice 3).
    Image {
        /// Full-size image URL on a Reddit media CDN.
        url: String,
    },
    /// A gallery of images (slice 3).
    Gallery {
        /// Image URLs in order.
        urls: Vec<String>,
    },
    /// A video (slice 3).
    Video {
        /// Playlist or fallback URL.
        url: String,
    },
}

/// A post as listings and the post view show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Post {
    /// `t3_…`.
    pub id: PostId,
    /// The subreddit it was posted in.
    pub subreddit: SubredditName,
    /// `None` when the account was deleted.
    pub author: Option<Username>,
    /// The title as plain text.
    pub title: String,
    /// What the post contains.
    pub kind: PostKind,
    /// The text of a text post, and optional text on other kinds.
    pub body: Option<Body>,
    /// Path on reddit.com, `/r/{sub}/comments/{id}/{slug}/`.
    pub permalink: String,
    /// Creation time, seconds since the Unix epoch (UTC).
    pub created: i64,
    /// Net score as Reddit shows it, including the user's own vote.
    pub score: i64,
    /// Number of comments.
    pub comment_count: u64,
    /// The user's vote.
    pub vote: VoteState,
    /// Whether the user saved it.
    pub saved: bool,
    /// Link flair text.
    pub flair: Option<String>,
    /// Marked NSFW.
    pub nsfw: bool,
    /// Marked as a spoiler.
    pub spoiler: bool,
    /// Pinned by moderators.
    pub pinned: bool,
    /// Locked: no new comments.
    pub locked: bool,
    /// Marked as original content.
    pub oc: bool,
    /// Edited after posting.
    pub edited: bool,
    /// Small preview image on a Reddit media CDN.
    pub thumbnail: Option<String>,
}

/// Whether a comment still has its text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommentState {
    /// Normal.
    #[default]
    Normal,
    /// Deleted by its author: Reddit shows "[deleted]".
    Deleted,
    /// Removed by moderators: Reddit shows "[removed]".
    Removed,
}

/// One comment, without its replies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    /// `t1_…`.
    pub id: CommentId,
    /// The post it belongs to.
    pub post_id: PostId,
    /// `None` for a top-level comment.
    pub parent: Option<CommentId>,
    /// `None` when deleted.
    pub author: Option<Username>,
    /// The text. Empty for deleted and removed comments.
    pub body: Body,
    /// Creation time, seconds since the Unix epoch (UTC).
    pub created: i64,
    /// Net score including the user's own vote; `None` while Reddit hides it.
    pub score: Option<i64>,
    /// The user's vote.
    pub vote: VoteState,
    /// Whether the user saved it.
    pub saved: bool,
    /// Written by the post's author.
    pub is_submitter: bool,
    /// Pinned by moderators.
    pub pinned: bool,
    /// Edited after posting.
    pub edited: bool,
    /// Deleted or removed.
    pub state: CommentState,
    /// Depth in the thread, 0 for top-level comments.
    pub depth: u32,
}

/// One node of a comment tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommentNode {
    /// A comment and its loaded replies.
    Comment {
        /// The comment.
        comment: Comment,
        /// Replies that came with it.
        replies: Vec<CommentNode>,
    },
    /// "N more replies": replies that load through `more_comments`.
    More {
        /// The comment they reply to; `None` for more top-level comments.
        parent: Option<CommentId>,
        /// Depth the loaded comments start at.
        depth: u32,
        /// How many comments Reddit says are behind it (may be 0 when unknown).
        count: u32,
        /// What `more_comments` needs to load them.
        token: MoreToken,
    },
    /// "Continue this thread": the branch goes deeper than Reddit sends in one
    /// response. Opening it loads the thread focused on `parent`.
    Continue {
        /// The comment whose replies continue.
        parent: CommentId,
        /// Depth of the replies.
        depth: u32,
    },
}

/// A page of results with the cursor for the next page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listing<T> {
    /// Items in order.
    pub items: Vec<T>,
    /// `None` on the last page.
    pub after: Option<Cursor>,
}

/// Which feed a listing comes from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Feed {
    /// The user's subscriptions.
    Home,
    /// Reddit's Popular feed.
    Popular,
    /// One subreddit.
    Subreddit(SubredditName),
    /// Posts by one user.
    User(Username),
    /// The signed-in user's saved posts.
    Saved,
}

/// Time range for Top sorting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeRange {
    /// Past hour.
    Hour,
    /// Past 24 hours.
    #[default]
    Day,
    /// Past week.
    Week,
    /// Past month.
    Month,
    /// Past year.
    Year,
    /// All time.
    All,
}

impl TimeRange {
    /// All values in menu order.
    pub const ALL: [TimeRange; 6] = [Self::Hour, Self::Day, Self::Week, Self::Month, Self::Year, Self::All];

    /// The GSettings / URL value (`hour`, `day`, …).
    pub fn key(self) -> &'static str {
        match self {
            Self::Hour => "hour",
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
            Self::All => "all",
        }
    }

    /// Parses [`TimeRange::key`].
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.key() == key)
    }
}

/// Sort order of a post listing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sort {
    /// Reddit's default ranking.
    #[default]
    Best,
    /// Hot.
    Hot,
    /// Newest first.
    New,
    /// Highest score in a time range.
    Top(TimeRange),
    /// Rising.
    Rising,
}

impl Sort {
    /// The GSettings / URL value without the time range (`best`, `hot`, …).
    pub fn key(self) -> &'static str {
        match self {
            Self::Best => "best",
            Self::Hot => "hot",
            Self::New => "new",
            Self::Top(_) => "top",
            Self::Rising => "rising",
        }
    }

    /// Parses [`Sort::key`]; `top` uses `range`.
    pub fn from_key(key: &str, range: TimeRange) -> Option<Self> {
        Some(match key {
            "best" => Self::Best,
            "hot" => Self::Hot,
            "new" => Self::New,
            "top" => Self::Top(range),
            "rising" => Self::Rising,
            _ => return None,
        })
    }
}

/// Sort order of a comment tree.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommentSort {
    /// Reddit's default ranking.
    #[default]
    Best,
    /// Highest score.
    Top,
    /// Newest first.
    New,
    /// Controversial.
    Controversial,
    /// Oldest first.
    Old,
    /// Q&A: answers from the post's author first.
    Qa,
}

impl CommentSort {
    /// All values in menu order.
    pub const ALL: [CommentSort; 6] = [
        Self::Best,
        Self::Top,
        Self::New,
        Self::Controversial,
        Self::Old,
        Self::Qa,
    ];

    /// The GSettings / URL value.
    pub fn key(self) -> &'static str {
        match self {
            Self::Best => "best",
            Self::Top => "top",
            Self::New => "new",
            Self::Controversial => "controversial",
            Self::Old => "old",
            Self::Qa => "qa",
        }
    }

    /// Parses [`CommentSort::key`].
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.key() == key)
    }
}

/// A subreddit rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// Short title.
    pub title: String,
    /// Longer explanation.
    pub description: Option<Body>,
}

/// A subreddit as the header, sidebar and search show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subreddit {
    /// `t5_…`, needed by `subscribe`. May be empty when only the name is known.
    pub id: String,
    /// The name.
    pub name: SubredditName,
    /// Display title.
    pub title: Option<String>,
    /// Short public description as plain text.
    pub description: Option<String>,
    /// Icon on a Reddit media CDN.
    pub icon_url: Option<String>,
    /// Banner on a Reddit media CDN.
    pub banner_url: Option<String>,
    /// Member count, when Reddit shows it.
    pub members: Option<u64>,
    /// Whether the user is subscribed.
    pub subscribed: bool,
    /// Marked NSFW.
    pub nsfw: bool,
    /// Creation time, seconds since the Unix epoch (UTC).
    pub created: Option<i64>,
    /// Rules, in order.
    pub rules: Vec<Rule>,
}

impl Subreddit {
    /// A subreddit known only by name.
    pub fn from_name(name: SubredditName) -> Self {
        Self {
            id: String::new(),
            name,
            title: None,
            description: None,
            icon_url: None,
            banner_url: None,
            members: None,
            subscribed: false,
            nsfw: false,
            created: None,
            rules: Vec::new(),
        }
    }
}

/// A user profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    /// The name.
    pub name: Username,
    /// Post karma, when shown.
    pub post_karma: Option<i64>,
    /// Comment karma, when shown.
    pub comment_karma: Option<i64>,
    /// Account creation ("cake day"), seconds since the Unix epoch (UTC).
    pub created: Option<i64>,
    /// Avatar on a Reddit media CDN.
    pub avatar_url: Option<String>,
}

/// What a notification is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationKind {
    /// A reply to the user's post.
    PostReply,
    /// A reply to the user's comment.
    CommentReply,
    /// A mention of the user's name.
    Mention,
    /// Anything else Reddit notifies about.
    Other,
}

/// One entry from `/notifications` (SPEC §4.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notification {
    /// Reddit's id for it.
    pub id: String,
    /// What it is about.
    pub kind: NotificationKind,
    /// Title line as Reddit words it.
    pub title: String,
    /// Preview text, if any.
    pub body: Option<String>,
    /// Path on reddit.com it opens.
    pub permalink: String,
    /// Time, seconds since the Unix epoch (UTC).
    pub created: i64,
    /// Whether the user has seen it.
    pub read: bool,
}

/// What a search looks for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SearchKind {
    /// Posts.
    #[default]
    Posts,
    /// Subreddits.
    Subreddits,
    /// Users.
    Users,
}

/// Sort order of post search results.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SearchSort {
    /// Best match.
    #[default]
    Relevance,
    /// Hot.
    Hot,
    /// Highest score.
    Top,
    /// Newest first.
    New,
    /// Most comments.
    Comments,
}

/// Search results; only the list matching the request's kind is filled.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResults {
    /// Matching posts.
    pub posts: Vec<Post>,
    /// Matching subreddits.
    pub subreddits: Vec<Subreddit>,
    /// Matching users.
    pub users: Vec<User>,
    /// Next page.
    pub after: Option<Cursor>,
}

/// A new post written in the composer (slice 4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewPost {
    /// Where to post.
    pub subreddit: SubredditName,
    /// Title, at most 300 characters.
    pub title: String,
    /// Text post body or link.
    pub content: NewPostContent,
    /// Required or chosen flair id.
    pub flair_id: Option<String>,
    /// Mark NSFW.
    pub nsfw: bool,
    /// Mark as spoiler.
    pub spoiler: bool,
}

/// Content of a [`NewPost`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NewPostContent {
    /// Markdown text.
    Text(String),
    /// A link.
    Link(String),
}
