use std::cell::OnceCell;
use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use crate::stores::{Context, SessionState};

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/ClickCalickClick/Agora/ui/welcome.ui")]
    pub struct AgoraWelcome {
        #[template_child]
        pub carousel: TemplateChild<adw::Carousel>,
        pub ctx: OnceCell<Rc<Context>>,
        pub on_sign_in: OnceCell<Box<dyn Fn()>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AgoraWelcome {
        const NAME: &'static str = "AgoraWelcome";
        type Type = super::AgoraWelcome;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AgoraWelcome {}
    impl WidgetImpl for AgoraWelcome {}
    impl AdwDialogImpl for AgoraWelcome {}
}

glib::wrapper! {
    /// First launch: disclaimer and risk acknowledgement, sign-in, tips (SPEC §6).
    pub struct AgoraWelcome(ObjectSubclass<imp::AgoraWelcome>)
        @extends gtk::Widget, adw::Dialog,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[gtk::template_callbacks]
impl AgoraWelcome {
    #[template_callback]
    fn on_continue(&self) {
        let ctx = self.imp().ctx.get().unwrap();
        // Nothing reaches Reddit before this line (SPEC §6).
        let _ = ctx.settings.set_boolean("risk-acknowledged", true);
        self.scroll_to(1);
        ctx.session.check();
    }

    #[template_callback]
    fn on_sign_in(&self) {
        if let Some(f) = self.imp().on_sign_in.get() {
            f();
        }
    }

    #[template_callback]
    fn on_done(&self) {
        self.close();
    }
}

impl AgoraWelcome {
    /// Creates the dialog; `on_sign_in` opens the sign-in dialog.
    pub fn new(ctx: Rc<Context>, on_sign_in: impl Fn() + 'static) -> Self {
        let obj: Self = glib::Object::new();
        let handler = ctx.session.connect_state_notify(glib::clone!(
            #[weak]
            obj,
            move |session| {
                if session.state() == SessionState::SignedIn {
                    obj.scroll_to(2);
                }
            }
        ));
        let session = ctx.session.clone();
        let handler = std::cell::Cell::new(Some(handler));
        obj.connect_closed(move |_| {
            if let Some(handler) = handler.take() {
                session.disconnect(handler);
            }
        });
        let imp = obj.imp();
        let _ = imp.ctx.set(ctx);
        let _ = imp.on_sign_in.set(Box::new(on_sign_in));
        obj
    }

    fn scroll_to(&self, page: u32) {
        let carousel = &*self.imp().carousel;
        carousel.scroll_to(&carousel.nth_page(page), true);
    }
}
