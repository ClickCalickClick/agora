use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use agora_core::Backend;
use agora_core::mock::{FailureScript, MockBackend};
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::config;
use crate::stores::Context;
use crate::ui::AgoraWindow;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct AgoraApplication {
        pub ctx: RefCell<Option<Rc<Context>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AgoraApplication {
        const NAME: &'static str = "AgoraApplication";
        type Type = super::AgoraApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for AgoraApplication {}

    impl ApplicationImpl for AgoraApplication {
        fn startup(&self) {
            self.parent_startup();
            let app = self.obj();
            app.setup_actions();
            app.load_css();
        }

        fn activate(&self) {
            let app = self.obj();
            if let Some(window) = app.active_window() {
                window.present();
                return;
            }
            let window = AgoraWindow::new(&*app, app.context());
            window.present();
        }
    }

    impl GtkApplicationImpl for AgoraApplication {}
    impl AdwApplicationImpl for AgoraApplication {}
}

glib::wrapper! {
    /// The application: actions, accelerators, About, and the shared [`Context`].
    pub struct AgoraApplication(ObjectSubclass<imp::AgoraApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl AgoraApplication {
    /// Creates the application.
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", config::APP_ID)
            .property("resource-base-path", config::RESOURCE_BASE)
            .build()
    }

    /// The shared context, created on first use.
    pub fn context(&self) -> Rc<Context> {
        if let Some(ctx) = self.imp().ctx.borrow().clone() {
            return ctx;
        }
        let settings = gio::Settings::new(config::SCHEMA_ID);
        // Slice 1 has no real backend yet: sample data is used either way, and
        // the window says so unless AGORA_MOCK=1 asked for it (SPEC §19).
        let mut mock = MockBackend::new().with_now(crate::util::now());
        if let Ok(spec) = std::env::var("AGORA_MOCK_FAIL") {
            mock = mock.with_failures(FailureScript::from_spec(&spec));
        }
        let latency = std::env::var("AGORA_MOCK_LATENCY_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(350);
        mock = mock.with_latency(
            Rc::new(|d: Duration| Box::pin(glib::timeout_future(d)) as futures::future::LocalBoxFuture<'static, ()>),
            Duration::from_millis(latency),
        );
        let backend: Rc<dyn Backend> = Rc::new(mock);
        let ctx = Context::new(backend, settings, true);
        self.imp().ctx.replace(Some(ctx.clone()));
        ctx
    }

    fn setup_actions(&self) {
        let quit = gio::ActionEntry::builder("quit")
            .activate(|app: &Self, _, _| {
                for window in app.windows() {
                    window.close();
                }
                app.quit();
            })
            .build();
        let about = gio::ActionEntry::builder("about")
            .activate(|app: &Self, _, _| app.show_about())
            .build();
        let shortcuts = gio::ActionEntry::builder("shortcuts")
            .activate(|app: &Self, _, _| {
                let builder = gtk::Builder::from_resource(&format!("{}/ui/shortcuts_dialog.ui", config::RESOURCE_BASE));
                let dialog: adw::Dialog = builder.object("shortcuts_dialog").unwrap();
                dialog.present(app.active_window().as_ref());
            })
            .build();
        self.add_action_entries([quit, about, shortcuts]);

        self.set_accels_for_action("app.quit", &["<Control>q"]);
        self.set_accels_for_action("app.shortcuts", &["<Control>question"]);
        self.set_accels_for_action("window.close", &["<Control>w"]);
        self.set_accels_for_action("win.toggle-sidebar", &["F9"]);
        self.set_accels_for_action("win.refresh", &["F5", "<Control>r"]);
        self.set_accels_for_action("win.zoom-in", &["<Control>plus", "<Control>equal", "<Control>KP_Add"]);
        self.set_accels_for_action("win.zoom-out", &["<Control>minus", "<Control>KP_Subtract"]);
        self.set_accels_for_action("win.zoom-reset", &["<Control>0", "<Control>KP_0"]);
    }

    fn load_css(&self) {
        let provider = gtk::CssProvider::new();
        provider.load_from_resource(&format!("{}/style/style.css", config::RESOURCE_BASE));
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
    }

    fn show_about(&self) {
        let about = adw::AboutDialog::builder()
            .application_name(gettext("Agora"))
            .application_icon(config::APP_ID)
            .developer_name("Jared")
            .version(config::VERSION)
            .website(config::REPO_URL)
            .issue_url(format!("{}/issues", config::REPO_URL))
            .license_type(gtk::License::Gpl30)
            .comments(gettext("An unofficial client for browsing and discussing Reddit"))
            .build();
        about.add_acknowledgement_section(
            Some(&gettext("Inspired by")),
            &["Giara by Gabriele Musco and contributors https://gitlab.gnome.org/World/giara"],
        );
        about.add_legal_section(&gettext("Disclaimer"), None, gtk::License::Custom, Some(&disclaimer()));
        about.add_link(&gettext("Code of Conduct"), "https://conduct.gnome.org/");
        about.present(self.active_window().as_ref());
    }
}

impl Default for AgoraApplication {
    fn default() -> Self {
        Self::new()
    }
}

/// The disclaimer from `DISCLAIMER.md`, translatable (SPEC §18.1).
pub fn disclaimer() -> String {
    [
        gettext(
            "Agora is an unofficial, independent project. It is not affiliated with, endorsed by, or sponsored by Reddit, Inc. “Reddit” is a trademark of Reddit, Inc. and is used only to describe what this app works with.",
        ),
        gettext(
            "Agora is for personal use. It shows Reddit through your own reddit.com login, inside an embedded web browser. Automated use of reddit.com may violate Reddit's User Agreement; you are responsible for how you use it and for your account.",
        ),
        gettext("Agora depends on reddit.com's website and may stop working at any time."),
        gettext(
            "The software is provided “as is”, without warranty of any kind, as stated in the GPL-3.0 license. Use it at your own risk.",
        ),
    ]
    .join("\n\n")
}
