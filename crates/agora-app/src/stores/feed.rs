use std::cell::{Cell, RefCell};
use std::rc::Rc;

use agora_core::{AgoraError, Cursor, Feed, ListingRequest, Sort, TimeRange};
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use super::context::error_message;
use super::{Context, LoadState, PostObject, SubredditObject};

mod imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::FeedStore)]
    pub struct FeedStore {
        pub ctx: RefCell<Option<Rc<Context>>>,
        pub feed: RefCell<Option<Feed>>,
        pub sort: Cell<Sort>,
        pub after: RefCell<Option<Cursor>>,
        /// Bumped on every reset so late replies for an old feed are dropped.
        pub generation: Cell<u64>,
        pub posts: RefCell<Option<gio::ListStore>>,
        /// Title for the list pane header ("Home", "r/gnome").
        #[property(get)]
        pub title: RefCell<String>,
        #[property(get, builder(LoadState::Idle))]
        pub state: Cell<LoadState>,
        /// Message for the last error, empty otherwise.
        #[property(get)]
        pub error: RefCell<String>,
        /// The subreddit this feed shows, with header info once loaded.
        #[property(get, nullable)]
        pub subreddit: RefCell<Option<SubredditObject>>,
        /// `best`, `hot`, `new`, `top`, `rising`.
        #[property(get)]
        pub sort_key: RefCell<String>,
        /// Time range key while sorting by Top.
        #[property(get)]
        pub top_range: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FeedStore {
        const NAME: &'static str = "AgoraFeedStore";
        type Type = super::FeedStore;
    }

    #[glib::derived_properties]
    impl ObjectImpl for FeedStore {}
}

glib::wrapper! {
    /// One feed's posts, loaded a page at a time (SPEC §4.6, §7.1).
    pub struct FeedStore(ObjectSubclass<imp::FeedStore>);
}

impl FeedStore {
    /// Creates an empty store; call [`FeedStore::set_feed`].
    pub fn new(ctx: Rc<Context>) -> Self {
        let obj: Self = glib::Object::new();
        let imp = obj.imp();
        let range = TimeRange::from_key(&ctx.settings.string("default-top-range")).unwrap_or_default();
        let sort = Sort::from_key(&ctx.settings.string("default-sort"), range).unwrap_or_default();
        imp.sort.set(sort);
        imp.sort_key.replace(sort.key().into());
        imp.top_range.replace(range.key().into());
        imp.posts.replace(Some(gio::ListStore::new::<PostObject>()));
        imp.ctx.replace(Some(ctx));
        obj
    }

    /// The posts, as [`PostObject`]s.
    pub fn posts(&self) -> gio::ListStore {
        self.imp().posts.borrow().clone().unwrap()
    }

    /// The feed shown.
    pub fn feed(&self) -> Option<Feed> {
        self.imp().feed.borrow().clone()
    }

    /// The current sort.
    pub fn sort(&self) -> Sort {
        self.imp().sort.get()
    }

    fn ctx(&self) -> Rc<Context> {
        self.imp().ctx.borrow().clone().unwrap()
    }

    /// Shows another feed and loads its first page.
    pub fn set_feed(&self, feed: Feed) {
        let imp = self.imp();
        let title = match &feed {
            Feed::Home => gettext("Home"),
            Feed::Popular => gettext("Popular"),
            Feed::Saved => gettext("Saved"),
            Feed::Subreddit(name) => name.prefixed(),
            Feed::User(name) => name.prefixed(),
        };
        imp.title.replace(title);
        self.notify_title();
        let subreddit = match &feed {
            Feed::Subreddit(name) => Some(SubredditObject::new(agora_core::Subreddit::from_name(name.clone()))),
            _ => None,
        };
        imp.subreddit.replace(subreddit.clone());
        self.notify_subreddit();
        imp.feed.replace(Some(feed));
        self.refresh();
        if let Some(sub) = subreddit {
            self.load_subreddit_info(sub);
        }
    }

    /// Changes the sort and reloads.
    pub fn set_sort(&self, sort: Sort) {
        let imp = self.imp();
        if imp.sort.get() == sort {
            return;
        }
        imp.sort.set(sort);
        imp.sort_key.replace(sort.key().into());
        self.notify_sort_key();
        if let Sort::Top(range) = sort {
            imp.top_range.replace(range.key().into());
            self.notify_top_range();
        }
        self.refresh();
    }

    /// Drops what is loaded and loads the first page again.
    pub fn refresh(&self) {
        let imp = self.imp();
        imp.generation.set(imp.generation.get() + 1);
        imp.after.replace(None);
        self.posts().remove_all();
        self.set_state(LoadState::Idle);
        self.load_more();
    }

    /// Loads the next page, unless one is loading or all are loaded.
    pub fn load_more(&self) {
        let imp = self.imp();
        if matches!(self.state(), LoadState::Loading | LoadState::Done) {
            return;
        }
        let Some(feed) = self.feed() else { return };
        let ctx = self.ctx();
        if !ctx.session.can_request() && !ctx.mock {
            return;
        }
        let request = ListingRequest {
            feed,
            sort: imp.sort.get(),
            after: imp.after.borrow().clone(),
        };
        let generation = imp.generation.get();
        self.set_state(LoadState::Loading);
        let backend = ctx.backend.clone();
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            let result = backend.listing(request).await;
            let Some(obj) = weak.upgrade() else { return };
            if obj.imp().generation.get() != generation {
                return;
            }
            match result {
                Ok(listing) => {
                    let items: Vec<PostObject> = listing.items.into_iter().map(PostObject::new).collect();
                    obj.posts().extend_from_slice(&items);
                    let done = listing.after.is_none();
                    obj.imp().after.replace(listing.after);
                    obj.set_state(if done { LoadState::Done } else { LoadState::Idle });
                }
                Err(AgoraError::Cancelled) => obj.set_state(LoadState::Idle),
                Err(e) => obj.set_error(&e),
            }
        });
    }

    fn load_subreddit_info(&self, sub: SubredditObject) {
        let ctx = self.ctx();
        let backend = ctx.backend.clone();
        let name = sub.subreddit().name;
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            let result = backend.subreddit_info(name).await;
            let Some(obj) = weak.upgrade() else { return };
            if obj.subreddit().as_ref() != Some(&sub) {
                return;
            }
            match result {
                Ok(info) => {
                    obj.imp().subreddit.replace(Some(SubredditObject::new(info)));
                    obj.notify_subreddit();
                }
                Err(e) => tracing::info!("subreddit_info failed: {e}"),
            }
        });
    }

    fn set_state(&self, state: LoadState) {
        let imp = self.imp();
        if state != LoadState::Error && !imp.error.borrow().is_empty() {
            imp.error.replace(String::new());
            self.notify_error();
        }
        if imp.state.get() != state {
            imp.state.set(state);
            self.notify_state();
        }
    }

    fn set_error(&self, error: &AgoraError) {
        self.imp().error.replace(error_message(error));
        self.notify_error();
        self.set_state(LoadState::Error);
    }
}
