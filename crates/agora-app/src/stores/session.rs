use std::cell::{Cell, RefCell};
use std::rc::Rc;

use agora_core::{Backend, BackendEvent};
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
}

mod imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::SessionStore)]
    pub struct SessionStore {
        pub backend: RefCell<Option<Rc<dyn Backend>>>,
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
                Err(agora_core::AgoraError::Blocked { .. }) => obj.set(SessionState::Blocked, ""),
                Err(agora_core::AgoraError::SessionExpired) => obj.set(SessionState::Expired, ""),
                Err(e) => tracing::warn!("whoami failed: {e}"),
            }
        });
    }

    /// Whether requests may be sent.
    pub fn can_request(&self) -> bool {
        self.state() == SessionState::SignedIn
    }

    fn handle(&self, event: BackendEvent) {
        match event {
            BackendEvent::SignedIn(name) => self.set(SessionState::SignedIn, name.as_str()),
            BackendEvent::SignedOut => self.set(SessionState::SignedOut, ""),
            BackendEvent::SessionExpired => self.set(SessionState::Expired, ""),
            BackendEvent::Blocked { .. } => self.set(SessionState::Blocked, &self.username()),
        }
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
