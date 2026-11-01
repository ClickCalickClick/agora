//! Widgets and pages. Pages bind to stores (SPEC §3.4).

mod comment_row;
mod feed_page;
mod post_header;
mod post_page;
mod post_row;
mod sidebar;
mod subreddit_header;
mod window;

pub use feed_page::AgoraFeedPage;
pub use post_header::PostHeader;
pub use post_page::AgoraPostPage;
pub use post_row::AgoraPostRow;
pub use sidebar::AgoraSidebar;
pub use subreddit_header::SubredditHeader;
pub use window::AgoraWindow;
