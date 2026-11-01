use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use super::{Context, LoadState, SubredditObject};

mod imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::SubscriptionsStore)]
    pub struct SubscriptionsStore {
        pub ctx: RefCell<Option<Rc<Context>>>,
        pub list: RefCell<Option<gio::ListStore>>,
        #[property(get, builder(LoadState::Idle))]
        pub state: Cell<LoadState>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SubscriptionsStore {
        const NAME: &'static str = "AgoraSubscriptionsStore";
        type Type = super::SubscriptionsStore;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SubscriptionsStore {}
}

glib::wrapper! {
    /// The user's subscriptions, sorted by name with favorites first (SPEC §5.1).
    pub struct SubscriptionsStore(ObjectSubclass<imp::SubscriptionsStore>);
}

impl SubscriptionsStore {
    /// Creates an empty store.
    pub fn new(ctx: Rc<Context>) -> Self {
        let obj: Self = glib::Object::new();
        obj.imp().list.replace(Some(gio::ListStore::new::<SubredditObject>()));
        obj.imp().ctx.replace(Some(ctx));
        obj
    }

    /// Subscriptions, as [`SubredditObject`]s.
    pub fn list(&self) -> gio::ListStore {
        self.imp().list.borrow().clone().unwrap()
    }

    /// Loads the list from the backend.
    pub fn refresh(&self) {
        if self.state() == LoadState::Loading {
            return;
        }
        let ctx = self.imp().ctx.borrow().clone().unwrap();
        self.set_state(LoadState::Loading);
        let backend = ctx.backend.clone();
        let favorites: Vec<String> = ctx
            .settings
            .strv("favorite-subreddits")
            .iter()
            .map(|s| s.to_lowercase())
            .collect();
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            let result = backend.subscriptions().await;
            let Some(obj) = weak.upgrade() else { return };
            match result {
                Ok(mut subs) => {
                    subs.sort_by_key(|s| {
                        let name = s.name.as_str().to_lowercase();
                        (!favorites.contains(&name), name)
                    });
                    let items: Vec<SubredditObject> = subs.into_iter().map(SubredditObject::new).collect();
                    obj.list().splice(0, obj.list().n_items(), &items);
                    obj.set_state(LoadState::Done);
                }
                Err(e) => {
                    tracing::warn!("subscriptions failed: {e}");
                    obj.set_state(LoadState::Error);
                }
            }
        });
    }

    fn set_state(&self, state: LoadState) {
        if self.imp().state.replace(state) != state {
            self.notify_state();
        }
    }
}
