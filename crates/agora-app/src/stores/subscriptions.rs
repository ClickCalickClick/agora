use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use super::{Context, LoadState, SessionState, SubredditObject};

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
        // Loads at sign-in, and again if the load failed before a block ended.
        ctx.session.connect_state_notify(glib::clone!(
            #[weak]
            obj,
            move |session| match session.state() {
                SessionState::SignedIn => {
                    if matches!(obj.state(), LoadState::Idle | LoadState::Error) {
                        obj.refresh();
                    }
                }
                SessionState::SignedOut => {
                    obj.list().remove_all();
                    obj.set_state(LoadState::Idle);
                }
                _ => {}
            }
        ));
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
        if !ctx.session.can_request() {
            return;
        }
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
                    let retry = obj.downgrade();
                    obj.imp().ctx.borrow().as_ref().unwrap().toast_error(
                        &e,
                        Some(Rc::new(move || {
                            if let Some(store) = retry.upgrade() {
                                store.refresh();
                            }
                        })),
                    );
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

#[cfg(test)]
mod tests {
    use agora_core::mock::FailureScript;
    use agora_core::{AgoraError, MockBackend};

    use super::*;
    use crate::stores::test_util::{context, run_until, unchecked_context, with_main_context};

    #[test]
    fn nothing_is_sent_before_sign_in_and_loads_after() {
        with_main_context(|| {
            let ctx = unchecked_context(MockBackend::new());
            let store = SubscriptionsStore::new(ctx.clone());
            store.refresh();
            assert_eq!(store.state(), LoadState::Idle);
            ctx.session.check();
            run_until(|| store.state() == LoadState::Done);
            assert!(store.list().n_items() > 0);
        })
    }

    #[test]
    fn failed_load_stays_retryable() {
        with_main_context(|| {
            let script = FailureScript::default().fail("subscriptions", AgoraError::Network("x".into()));
            let store = SubscriptionsStore::new(context(MockBackend::new().with_failures(script)));
            store.refresh();
            run_until(|| store.state() == LoadState::Error);
        })
    }
}
