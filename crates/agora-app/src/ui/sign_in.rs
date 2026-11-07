use adw::prelude::*;
use gettextrs::gettext;
use gtk::glib;

use crate::stores::{SessionState, SessionStore, SignInView};

glib::wrapper! {
    /// Reddit's own login page in a dialog (SPEC §4.4). The person signs in
    /// there; Agora only asks who is signed in once the page moves on.
    pub struct AgoraSignIn(ObjectSubclass<imp::AgoraSignIn>)
        @extends gtk::Widget, adw::Dialog,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

mod imp {
    use adw::subclass::prelude::*;
    use gtk::glib;

    #[derive(Default)]
    pub struct AgoraSignIn;

    #[glib::object_subclass]
    impl ObjectSubclass for AgoraSignIn {
        const NAME: &'static str = "AgoraSignIn";
        type Type = super::AgoraSignIn;
        type ParentType = adw::Dialog;
    }

    impl ObjectImpl for AgoraSignIn {}
    impl WidgetImpl for AgoraSignIn {}
    impl AdwDialogImpl for AgoraSignIn {}
}

impl AgoraSignIn {
    /// Opens the page and closes itself once `session` is signed in.
    pub fn new(session: SessionStore, view: &SignInView) -> Self {
        let obj: Self = glib::Object::builder()
            .property("title", gettext("Sign In to Reddit"))
            .property("content-width", 520)
            .property("content-height", 720)
            .build();
        let checker = session.clone();
        let page = view(Box::new(move || checker.check()));
        page.set_hexpand(true);
        page.set_vexpand(true);
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        toolbar.set_content(Some(&page));
        obj.set_child(Some(&toolbar));
        let handler = session.connect_state_notify(glib::clone!(
            #[weak]
            obj,
            move |session| {
                if session.state() == SessionState::SignedIn {
                    obj.close();
                }
            }
        ));
        let handler = std::cell::Cell::new(Some(handler));
        obj.connect_closed(move |_| {
            if let Some(handler) = handler.take() {
                session.disconnect(handler);
            }
        });
        obj
    }
}
