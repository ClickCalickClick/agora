use std::cell::RefCell;
use std::rc::Rc;

use agora_core::{AgoraError, Backend};
use gettextrs::gettext;
use gtk::gio;

use super::SessionStore;
use crate::services::LocalData;

type ToastHandler = Box<dyn Fn(String, Option<Rc<dyn Fn()>>)>;

/// Makes the widget for signing in. Its argument runs when the page has moved
/// on, which is when `whoami` is worth asking (SPEC §4.4).
pub type SignInView = Rc<dyn Fn(Box<dyn Fn()>) -> gtk::Widget>;

/// What every store needs: the backend, settings, the session, and a way to
/// show an error toast. Created once by the application.
pub struct Context {
    /// The only way to reach Reddit.
    pub backend: Rc<dyn Backend>,
    /// `io.github.ClickCalickClick.Agora`.
    pub settings: gio::Settings,
    /// Signed-in state.
    pub session: SessionStore,
    /// True when running on `MockBackend`.
    pub mock: bool,
    /// The sign-in page; the real one is Reddit's, in a web view.
    pub sign_in_view: Option<SignInView>,
    /// `agora.db`, when it could be opened.
    pub local: Option<LocalData>,
    toast: RefCell<Option<ToastHandler>>,
}

impl Context {
    /// Creates the context.
    pub fn new(backend: Rc<dyn Backend>, settings: gio::Settings, mock: bool) -> Rc<Self> {
        Self::with_services(backend, settings, mock, None, None)
    }

    /// Creates the context with the sign-in page and the local database.
    pub fn with_services(
        backend: Rc<dyn Backend>,
        settings: gio::Settings,
        mock: bool,
        sign_in_view: Option<SignInView>,
        local: Option<LocalData>,
    ) -> Rc<Self> {
        let session = SessionStore::new(backend.clone());
        Rc::new(Self {
            backend,
            settings,
            session,
            mock,
            sign_in_view,
            local,
            toast: RefCell::default(),
        })
    }

    /// Sets the function that shows toasts (the window does this).
    pub fn set_toast_handler(&self, f: impl Fn(String, Option<Rc<dyn Fn()>>) + 'static) {
        self.toast.replace(Some(Box::new(f)));
    }

    /// Shows a toast with an optional Retry action (SPEC §4.7).
    pub fn toast(&self, message: String, retry: Option<Rc<dyn Fn()>>) {
        if let Some(f) = self.toast.borrow().as_ref() {
            f(message, retry);
        }
    }

    /// Shows the toast for a failed write.
    pub fn toast_error(&self, error: &AgoraError, retry: Option<Rc<dyn Fn()>>) {
        if matches!(
            error,
            AgoraError::Cancelled | AgoraError::SessionExpired | AgoraError::Blocked { .. }
        ) {
            // Cancelled is silent; the session banner covers the other two.
            return;
        }
        let retry = if error.is_retryable() { retry } else { None };
        self.toast(error_message(error), retry);
    }
}

/// User-facing text for an error (SPEC §2.1 writing voice).
pub fn error_message(error: &AgoraError) -> String {
    match error {
        AgoraError::Network(_) => gettext("Could not reach Reddit"),
        AgoraError::SessionExpired => gettext("Signed out of Reddit"),
        AgoraError::Blocked { .. } => gettext("Reddit asked Agora to slow down"),
        AgoraError::NotFound => gettext("Not found"),
        AgoraError::Forbidden => gettext("This is private or banned"),
        AgoraError::ExtractorBroken { .. } => gettext("Reddit changed something and this no longer works"),
        AgoraError::Parse(_) => gettext("Could not read Reddit's reply"),
        AgoraError::Io(_) => gettext("Could not save a file"),
        AgoraError::Cancelled => String::new(),
        AgoraError::NotAvailable => gettext("Voting, saving and joining arrive in a later build"),
        AgoraError::ChallengeRequired { .. } => gettext("Reddit wants to check that you are a person"),
    }
}
