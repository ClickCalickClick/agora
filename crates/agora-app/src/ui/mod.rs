//! Widgets and pages. Pages bind to stores (SPEC §3.4).

mod comment_row;
mod feed_page;
mod post_header;
mod post_page;
mod post_row;
mod sidebar;
mod subreddit_header;
mod window;

use gtk::glib;
use gtk::prelude::*;

pub use feed_page::AgoraFeedPage;
pub use post_header::PostHeader;
pub use post_page::AgoraPostPage;
pub use post_row::AgoraPostRow;
pub use sidebar::AgoraSidebar;
pub use subreddit_header::SubredditHeader;
pub use window::AgoraWindow;

const WATCH_KEY: &str = "agora-watch";

/// A signal handler a list row widget holds on a store object.
struct Watch(glib::Object, glib::SignalHandlerId);

/// Remembers `handler` on `widget` so the list factory can disconnect it when
/// the row is unbound (see [`release_watch`]).
fn attach_watch(widget: &impl IsA<gtk::Widget>, object: &impl IsA<glib::Object>, handler: glib::SignalHandlerId) {
    release_watch(widget);
    // SAFETY: the key is only ever used with the `Watch` type.
    unsafe { widget.set_data(WATCH_KEY, Watch(object.clone().upcast(), handler)) };
}

/// Disconnects the handler stored by [`attach_watch`], if any.
fn release_watch(widget: &impl IsA<gtk::Widget>) {
    // SAFETY: the key is only ever used with the `Watch` type.
    if let Some(Watch(object, handler)) = unsafe { widget.steal_data::<Watch>(WATCH_KEY) } {
        object.disconnect(handler);
    }
}
