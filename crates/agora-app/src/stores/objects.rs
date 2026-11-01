//! GObject wrappers for the items in store list models.

use std::cell::{Cell, RefCell};

use agora_core::{Comment, Post, Subreddit, VoteState};
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

/// Vote state as an integer property: 1 up, 0 none, -1 down.
pub fn vote_to_int(v: VoteState) -> i32 {
    match v {
        VoteState::Up => 1,
        VoteState::None => 0,
        VoteState::Down => -1,
    }
}

/// Inverse of [`vote_to_int`].
pub fn int_to_vote(v: i32) -> VoteState {
    match v.signum() {
        1 => VoteState::Up,
        -1 => VoteState::Down,
        _ => VoteState::None,
    }
}

mod post_imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::PostObject)]
    pub struct PostObject {
        pub post: RefCell<Option<Post>>,
        /// Net score including the user's vote.
        #[property(get, set)]
        pub score: Cell<i64>,
        /// 1 up, 0 none, -1 down.
        #[property(get, set, minimum = -1, maximum = 1)]
        pub vote: Cell<i32>,
        #[property(get, set)]
        pub saved: Cell<bool>,
        /// Opened during this session (SPEC §13: read state is in memory only).
        #[property(get, set)]
        pub read: Cell<bool>,
        #[property(get, set)]
        pub comment_count: Cell<u64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PostObject {
        const NAME: &'static str = "AgoraPostObject";
        type Type = super::PostObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for PostObject {}
}

glib::wrapper! {
    /// A post in a feed or at the top of a thread.
    pub struct PostObject(ObjectSubclass<post_imp::PostObject>);
}

impl PostObject {
    /// Wraps a post.
    pub fn new(post: Post) -> Self {
        let obj: Self = glib::Object::builder()
            .property("score", post.score)
            .property("vote", vote_to_int(post.vote))
            .property("saved", post.saved)
            .property("comment-count", post.comment_count)
            .build();
        obj.imp().post.replace(Some(post));
        obj
    }

    /// The post as last loaded. Live values (score, vote, saved) are the
    /// properties, which writes update.
    pub fn post(&self) -> Post {
        self.imp().post.borrow().clone().expect("PostObject without post")
    }

    /// Calls `f` with the post without cloning it.
    pub fn with<R>(&self, f: impl FnOnce(&Post) -> R) -> R {
        f(self.imp().post.borrow().as_ref().expect("PostObject without post"))
    }

    /// Replaces the post with a fresh copy and updates the properties.
    pub fn update(&self, post: Post) {
        self.set_score(post.score);
        self.set_vote(vote_to_int(post.vote));
        self.set_saved(post.saved);
        self.set_comment_count(post.comment_count);
        self.imp().post.replace(Some(post));
    }

    /// The vote as a [`VoteState`].
    pub fn vote_state(&self) -> VoteState {
        int_to_vote(self.vote())
    }
}

/// What a row of the flattened comment list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, glib::Enum)]
#[enum_type(name = "AgoraCommentRowKind")]
pub enum CommentRowKind {
    /// A comment.
    #[default]
    Comment,
    /// "N more replies".
    More,
    /// "Continue this thread".
    Continue,
}

mod comment_imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::CommentObject)]
    pub struct CommentObject {
        pub comment: RefCell<Option<Comment>>,
        pub token: RefCell<Option<agora_core::MoreToken>>,
        pub continue_parent: RefCell<Option<agora_core::CommentId>>,
        /// Rows hidden under this one while it is collapsed.
        pub stash: RefCell<Vec<super::CommentObject>>,
        #[property(get, builder(CommentRowKind::Comment))]
        pub kind: Cell<CommentRowKind>,
        /// Depth used for indentation and thread lines.
        #[property(get, set)]
        pub depth: Cell<u32>,
        #[property(get, set)]
        pub collapsed: Cell<bool>,
        /// For a collapsed comment: how many replies are hidden under it.
        /// For a "more" row: how many comments it loads.
        #[property(get, set)]
        pub count: Cell<u32>,
        #[property(get, set)]
        pub score: Cell<i64>,
        #[property(get, set)]
        pub score_hidden: Cell<bool>,
        #[property(get, set, minimum = -1, maximum = 1)]
        pub vote: Cell<i32>,
        #[property(get, set)]
        pub saved: Cell<bool>,
        /// Which thread line (by depth) is highlighted on this row, or -1.
        #[property(get, set, minimum = -1)]
        pub highlight_line: Cell<i32>,
        /// A "more" row that is loading.
        #[property(get, set)]
        pub loading: Cell<bool>,
        /// Fade the row in the next time it is bound (after expanding).
        pub fade_in: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CommentObject {
        const NAME: &'static str = "AgoraCommentObject";
        type Type = super::CommentObject;

        fn new() -> Self {
            Self {
                highlight_line: Cell::new(-1),
                ..Default::default()
            }
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for CommentObject {}
}

glib::wrapper! {
    /// One row of a flattened comment tree (SPEC §8.1).
    pub struct CommentObject(ObjectSubclass<comment_imp::CommentObject>);
}

impl CommentObject {
    /// A comment row.
    pub fn comment(comment: Comment) -> Self {
        let obj: Self = glib::Object::builder()
            .property("depth", comment.depth)
            .property("score", comment.score.unwrap_or(0))
            .property("score-hidden", comment.score.is_none())
            .property("vote", vote_to_int(comment.vote))
            .property("saved", comment.saved)
            .build();
        obj.imp().comment.replace(Some(comment));
        obj
    }

    /// A "N more replies" row.
    pub fn more(depth: u32, count: u32, token: agora_core::MoreToken) -> Self {
        let obj: Self = glib::Object::builder()
            .property("depth", depth)
            .property("count", count)
            .build();
        obj.imp().kind.set(CommentRowKind::More);
        obj.imp().token.replace(Some(token));
        obj
    }

    /// A "Continue this thread" row.
    pub fn continue_thread(depth: u32, parent: agora_core::CommentId) -> Self {
        let obj: Self = glib::Object::builder().property("depth", depth).build();
        obj.imp().kind.set(CommentRowKind::Continue);
        obj.imp().continue_parent.replace(Some(parent));
        obj
    }

    /// The comment of a comment row.
    pub fn data(&self) -> Option<Comment> {
        self.imp().comment.borrow().clone()
    }

    /// Calls `f` with the comment of a comment row.
    pub fn with<R>(&self, f: impl FnOnce(&Comment) -> R) -> Option<R> {
        self.imp().comment.borrow().as_ref().map(f)
    }

    /// The token of a "more" row.
    pub fn token(&self) -> Option<agora_core::MoreToken> {
        self.imp().token.borrow().clone()
    }

    /// The comment a "continue" row continues.
    pub fn continue_parent(&self) -> Option<agora_core::CommentId> {
        self.imp().continue_parent.borrow().clone()
    }

    /// The vote as a [`VoteState`].
    pub fn vote_state(&self) -> VoteState {
        int_to_vote(self.vote())
    }

    /// Takes the fade-in flag.
    pub fn take_fade_in(&self) -> bool {
        self.imp().fade_in.replace(false)
    }

    /// Sets the fade-in flag.
    pub fn mark_fade_in(&self) {
        self.imp().fade_in.set(true);
    }

    /// Rows hidden while collapsed.
    pub fn stash(&self) -> &RefCell<Vec<CommentObject>> {
        &self.imp().stash
    }
}

mod subreddit_imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::SubredditObject)]
    pub struct SubredditObject {
        pub subreddit: RefCell<Option<Subreddit>>,
        #[property(get, set)]
        pub subscribed: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SubredditObject {
        const NAME: &'static str = "AgoraSubredditObject";
        type Type = super::SubredditObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SubredditObject {}
}

glib::wrapper! {
    /// A subreddit in the sidebar or a feed header.
    pub struct SubredditObject(ObjectSubclass<subreddit_imp::SubredditObject>);
}

impl SubredditObject {
    /// Wraps a subreddit.
    pub fn new(subreddit: Subreddit) -> Self {
        let obj: Self = glib::Object::builder()
            .property("subscribed", subreddit.subscribed)
            .build();
        obj.imp().subreddit.replace(Some(subreddit));
        obj
    }

    /// The subreddit.
    pub fn subreddit(&self) -> Subreddit {
        self.imp()
            .subreddit
            .borrow()
            .clone()
            .expect("SubredditObject without subreddit")
    }
}

/// State of a list that loads in pages, shown by its footer row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, glib::Enum)]
#[enum_type(name = "AgoraLoadState")]
pub enum LoadState {
    /// Nothing loaded yet, or nothing loading.
    #[default]
    Idle,
    /// A request is in flight.
    Loading,
    /// The last request failed; the message is in the store's `error` property.
    Error,
    /// Everything is loaded.
    Done,
}

mod footer_imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::FooterObject)]
    pub struct FooterObject {
        #[property(get, set, builder(LoadState::Idle))]
        pub state: Cell<LoadState>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FooterObject {
        const NAME: &'static str = "AgoraFooterObject";
        type Type = super::FooterObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for FooterObject {}
}

glib::wrapper! {
    /// The last row of a paged list: spinner, error with Retry, or nothing.
    pub struct FooterObject(ObjectSubclass<footer_imp::FooterObject>);
}

impl Default for FooterObject {
    fn default() -> Self {
        glib::Object::new()
    }
}
