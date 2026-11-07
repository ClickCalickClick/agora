use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use agora_core::{AgoraError, Backend, BackendEvent};
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

/// Session state shown by the window (SPEC §4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, glib::Enum)]
#[enum_type(name = "AgoraSessionState")]
pub enum SessionState {
    /// Not known yet.
    #[default]
    Unknown,
    /// Signed in; `username` is set.
    SignedIn,
    /// Signed out.
    SignedOut,
    /// An operation found the session logged out; requests stop.
    Expired,
    /// Reddit pushed back; requests stop until the back-off ends.
    Blocked,
    /// The sign-in check failed (network, or Reddit changed); the user can retry.
    CheckFailed,
}

mod imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::SessionStore)]
    pub struct SessionStore {
        pub backend: RefCell<Option<Rc<dyn Backend>>>,
        /// Bumped by every block, so only the newest back-off timer acts.
        pub block_generation: Cell<u64>,
        /// Empty while signed out.
        #[property(get)]
        pub username: RefCell<String>,
        #[property(get, builder(SessionState::Unknown))]
        pub state: Cell<SessionState>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SessionStore {
        const NAME: &'static str = "AgoraSessionStore";
        type Type = super::SessionStore;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SessionStore {}
}

glib::wrapper! {
    /// Who is signed in, and whether requests may be sent (SPEC §4.4, §4.6).
    pub struct SessionStore(ObjectSubclass<imp::SessionStore>);
}

impl SessionStore {
    /// Creates the store and starts listening to backend events.
    pub fn new(backend: Rc<dyn Backend>) -> Self {
        let obj: Self = glib::Object::new();
        let events = backend.events();
        obj.imp().backend.replace(Some(backend));
        let weak = obj.downgrade();
        glib::spawn_future_local(async move {
            while let Ok(event) = events.recv().await {
                let Some(obj) = weak.upgrade() else { break };
                obj.handle(event);
            }
        });
        obj
    }

    /// Asks the backend who is signed in.
    pub fn check(&self) {
        let Some(backend) = self.imp().backend.borrow().clone() else {
            return;
        };
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            let result = backend.whoami().await;
            let Some(obj) = weak.upgrade() else { return };
            match result {
                Ok(Some(name)) => obj.set(SessionState::SignedIn, name.as_str()),
                Ok(None) => obj.set(SessionState::SignedOut, ""),
                Err(AgoraError::Blocked { retry_after }) => obj.enter_blocked(retry_after),
                Err(AgoraError::SessionExpired) => obj.set(SessionState::Expired, ""),
                Err(e) => {
                    tracing::warn!("whoami failed: {e}");
                    obj.set(SessionState::CheckFailed, "");
                }
            }
        });
    }

    /// Signs out: clears the web session (SPEC §4.4). The backend's `SignedOut`
    /// event moves the state.
    pub fn sign_out(&self, on_error: impl FnOnce(AgoraError) + 'static) {
        let Some(backend) = self.imp().backend.borrow().clone() else {
            return;
        };
        glib::spawn_future_local(async move {
            if let Err(e) = backend.sign_out().await {
                on_error(e);
            }
        });
    }

    /// Whether requests may be sent.
    pub fn can_request(&self) -> bool {
        self.state() == SessionState::SignedIn
    }

    /// Why requests may not be sent, for the error a page shows. `None` while
    /// signed in, and while the first check is still running.
    pub fn refusal(&self) -> Option<AgoraError> {
        match self.state() {
            SessionState::Unknown | SessionState::SignedIn => None,
            SessionState::SignedOut | SessionState::Expired => Some(AgoraError::SessionExpired),
            SessionState::Blocked => Some(AgoraError::Blocked {
                retry_after: Duration::ZERO,
            }),
            SessionState::CheckFailed => Some(AgoraError::Network(String::new())),
        }
    }

    fn handle(&self, event: BackendEvent) {
        match event {
            BackendEvent::SignedIn(name) => self.set(SessionState::SignedIn, name.as_str()),
            BackendEvent::SignedOut => self.set(SessionState::SignedOut, ""),
            BackendEvent::SessionExpired => self.set(SessionState::Expired, ""),
            BackendEvent::Blocked { retry_after } => self.enter_blocked(retry_after),
        }
    }

    /// Stops requests, and checks the session again when the back-off ends
    /// (SPEC §4.6). The check is the only thing that leaves `Blocked`.
    fn enter_blocked(&self, retry_after: Duration) {
        let imp = self.imp();
        imp.block_generation.set(imp.block_generation.get() + 1);
        let generation = imp.block_generation.get();
        self.set(SessionState::Blocked, &self.username());
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            glib::timeout_future(retry_after).await;
            let Some(obj) = weak.upgrade() else { return };
            if obj.imp().block_generation.get() == generation && obj.state() == SessionState::Blocked {
                obj.check();
            }
        });
    }

    fn set(&self, state: SessionState, username: &str) {
        let imp = self.imp();
        if *imp.username.borrow() != username {
            imp.username.replace(username.to_string());
            self.notify_username();
        }
        if imp.state.get() != state {
            imp.state.set(state);
            self.notify_state();
        }
    }
}

#[cfg(test)]
mod tests {
    use agora_core::MockBackend;
    use agora_core::mock::FailureScript;

    use super::*;
    use crate::stores::test_util::{context, run_until, with_main_context};

    #[test]
    fn failed_check_is_visible_and_not_stuck_in_unknown() {
        with_main_context(|| {
            let ctx = context(
                MockBackend::new()
                    .with_failures(FailureScript::default().fail("whoami", AgoraError::Network("x".into()))),
            );
            assert_eq!(ctx.session.state(), SessionState::CheckFailed);
            assert!(!ctx.session.can_request());
            assert!(ctx.session.refusal().is_some());
        })
    }

    #[test]
    fn blocked_ends_with_a_new_check_after_the_backoff() {
        with_main_context(|| {
            let blocked = AgoraError::Blocked {
                retry_after: Duration::from_millis(20),
            };
            let ctx = context(MockBackend::new().with_failures(FailureScript::default().fail("listing", blocked)));
            assert!(ctx.session.can_request());
            let request = agora_core::ListingRequest {
                feed: agora_core::Feed::Home,
                sort: agora_core::Sort::Best,
                after: None,
            };
            let fut = ctx.backend.listing(request);
            glib::spawn_future_local(async move {
                let _ = fut.await;
            });
            run_until(|| ctx.session.state() == SessionState::Blocked);
            assert!(!ctx.session.can_request());
            run_until(|| ctx.session.state() == SessionState::SignedIn);
        })
    }
}
