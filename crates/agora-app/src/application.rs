use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use agora_core::Backend;
use agora_core::mock::{FailureScript, MockBackend};
use agora_reddit_web::{Config, RedditWebBackend};
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::config;
use crate::services::LocalData;
use crate::stores::{Context, SignInView};
use crate::ui::AgoraWindow;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct AgoraApplication {
        pub ctx: RefCell<Option<Rc<Context>>>,
        pub reddit: RefCell<Option<RedditWebBackend>>,
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
            if std::env::args().any(|a| a == "--self-test") {
                app.run_self_test();
                return;
            }
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
            .property("flags", Self::flags())
            .build()
    }

    fn flags() -> gio::ApplicationFlags {
        // The self-test prints in its own process rather than the running app.
        if std::env::args().any(|a| a == "--self-test") {
            gio::ApplicationFlags::NON_UNIQUE
        } else {
            gio::ApplicationFlags::FLAGS_NONE
        }
    }

    /// The shared context, created on first use.
    pub fn context(&self) -> Rc<Context> {
        if let Some(ctx) = self.imp().ctx.borrow().clone() {
            return ctx;
        }
        let settings = gio::Settings::new(config::SCHEMA_ID);
        let ctx = if std::env::var_os("AGORA_MOCK").is_some_and(|v| v != "0") {
            Self::mock_context(settings)
        } else {
            self.reddit_context(settings)
        };
        self.imp().ctx.replace(Some(ctx.clone()));
        ctx
    }

    /// The real backend. Creating it sends nothing (SPEC §6).
    fn reddit_context(&self, settings: gio::Settings) -> Rc<Context> {
        let reddit = RedditWebBackend::new(Config::reddit());
        self.imp().reddit.replace(Some(reddit.clone()));
        let path = glib::user_data_dir().join("agora").join("agora.db");
        let local = LocalData::open(&path)
            .inspect_err(|e| tracing::warn!("could not open {}: {e}", path.display()))
            .ok();
        let login = reddit.clone();
        let sign_in: SignInView = Rc::new(move |on_navigated| login.login_view(on_navigated));
        Context::with_services(Rc::new(reddit), settings, false, Some(sign_in), local)
    }

    /// `AGORA_MOCK=1`: sample data (SPEC §19). `AGORA_MOCK_SIGNED_OUT=1` starts
    /// signed out, and the sign-in page is one button.
    fn mock_context(settings: gio::Settings) -> Rc<Context> {
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
        let mock = Rc::new(mock);
        if std::env::var_os("AGORA_MOCK_SIGNED_OUT").is_some() {
            mock.set_signed_in(false);
        }
        let signer = mock.clone();
        let sign_in: SignInView = Rc::new(move |on_navigated| {
            let button = gtk::Button::builder()
                .label("Sign In (sample data)")
                .halign(gtk::Align::Center)
                .valign(gtk::Align::Center)
                .build();
            let signer = signer.clone();
            button.connect_clicked(move |_| {
                signer.set_signed_in(true);
                on_navigated();
            });
            button.upcast()
        });
        let backend: Rc<dyn Backend> = mock;
        Context::with_services(backend, settings, true, Some(sign_in), None)
    }

    /// `agora --self-test` (SPEC §4.5): runs every read once and prints a line
    /// per operation. Exits with status 1 if any failed.
    fn run_self_test(&self) {
        let ctx = self.context();
        let reddit = self.imp().reddit.borrow().clone();
        let guard = self.hold();
        let app = self.clone();
        glib::spawn_future_local(async move {
            let code = match reddit {
                None => {
                    println!("The self-test needs real Reddit; unset AGORA_MOCK.");
                    1
                }
                Some(_) if !ctx.settings.boolean("risk-acknowledged") => {
                    println!("Open Agora once and accept the risk notice first.");
                    1
                }
                Some(reddit) => {
                    let mut failed = false;
                    for line in reddit.self_test().await {
                        match line.result {
                            Ok(note) => println!("ok    {:<24} {note}", line.operation),
                            Err(error) => {
                                failed = true;
                                println!("FAIL  {:<24} {error}", line.operation);
                            }
                        }
                    }
                    i32::from(failed)
                }
            };
            drop(guard);
            app.quit();
            if code != 0 {
                std::process::exit(code);
            }
        });
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
        self.set_accels_for_action("win.back", &["<Alt>Left"]);
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
            .translator_credits(gettext("translator-credits"))
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
