use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use agora_core::{Feed, SubredditName, Username};
use gettextrs::gettext;
use gtk::{gio, glib};

use super::{AgoraFeedPage, AgoraPostPage, AgoraSidebar};
use crate::render::LinkHandler;
use crate::stores::{Context, FeedStore, PostObject, SessionState, SubscriptionsStore, ThreadStore, Writes};
use crate::{config, util};

/// What the banner button does.
type BannerAction = Rc<dyn Fn()>;

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/ClickCalickClick/Agora/ui/window.ui")]
    pub struct AgoraWindow {
        #[template_child]
        pub toasts: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub banner: TemplateChild<adw::Banner>,
        #[template_child]
        pub outer: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub inner: TemplateChild<adw::NavigationSplitView>,
        #[template_child]
        pub sidebar: TemplateChild<AgoraSidebar>,
        #[template_child]
        pub feed_page: TemplateChild<AgoraFeedPage>,
        #[template_child]
        pub post_page: TemplateChild<AgoraPostPage>,
        pub ctx: OnceCell<Rc<Context>>,
        pub feed: OnceCell<FeedStore>,
        pub thread: OnceCell<ThreadStore>,
        pub subscriptions: OnceCell<SubscriptionsStore>,
        pub zoom_css: OnceCell<gtk::CssProvider>,
        pub banner_action: RefCell<Option<Rc<dyn Fn()>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AgoraWindow {
        const NAME: &'static str = "AgoraWindow";
        type Type = super::AgoraWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            AgoraSidebar::ensure_type();
            AgoraFeedPage::ensure_type();
            AgoraPostPage::ensure_type();
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AgoraWindow {}
    impl WidgetImpl for AgoraWindow {}

    impl WindowImpl for AgoraWindow {
        fn close_request(&self) -> glib::Propagation {
            if let Some(ctx) = self.ctx.get() {
                let obj = self.obj();
                let s = &ctx.settings;
                let (w, h) = obj.default_size();
                let _ = s.set_int("window-width", w.max(360));
                let _ = s.set_int("window-height", h.max(360));
                let _ = s.set_boolean("window-maximized", obj.is_maximized());
            }
            self.parent_close_request()
        }
    }

    impl ApplicationWindowImpl for AgoraWindow {}
    impl AdwApplicationWindowImpl for AgoraWindow {}
}

glib::wrapper! {
    /// The main window: sidebar, post list and post view (SPEC §5.1).
    pub struct AgoraWindow(ObjectSubclass<imp::AgoraWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

#[gtk::template_callbacks]
impl AgoraWindow {
    #[template_callback]
    fn on_banner_button(&self) {
        let action = self.imp().banner_action.borrow().clone();
        if let Some(action) = action {
            action();
        }
    }
}

impl AgoraWindow {
    /// Creates the window for `app`.
    pub fn new(app: &impl IsA<gtk::Application>, ctx: Rc<Context>) -> Self {
        let obj: Self = glib::Object::builder().property("application", app).build();
        if config::is_devel() {
            obj.add_css_class("devel");
        }
        obj.setup(ctx);
        obj
    }

    fn ctx(&self) -> &Rc<Context> {
        self.imp().ctx.get().unwrap()
    }

    fn setup(&self, ctx: Rc<Context>) {
        let imp = self.imp();
        let s = &ctx.settings;
        self.set_default_size(s.int("window-width"), s.int("window-height"));
        if s.boolean("window-maximized") {
            self.maximize();
        }

        let toasts = imp.toasts.downgrade();
        ctx.set_toast_handler(move |message, retry| {
            let Some(toasts) = toasts.upgrade() else { return };
            let toast = adw::Toast::new(&message);
            if let Some(retry) = retry {
                toast.set_button_label(Some(&gettext("_Retry")));
                toast.connect_button_clicked(move |_| retry());
            }
            toasts.add_toast(toast);
        });

        let feed = FeedStore::new(ctx.clone());
        let thread = ThreadStore::new(ctx.clone());
        let subscriptions = SubscriptionsStore::new(ctx.clone());
        let writes = Writes::new(ctx.clone());
        let on_link = self.link_handler();

        imp.sidebar.setup(&subscriptions, {
            let win = self.downgrade();
            move |feed| {
                if let Some(win) = win.upgrade() {
                    win.show_feed(feed);
                }
            }
        });
        imp.feed_page.setup(&feed, &writes, {
            let win = self.downgrade();
            move |post| {
                if let Some(win) = win.upgrade() {
                    win.open_post(post);
                }
            }
        });
        imp.post_page.setup(&ctx, &thread, &writes, on_link);

        let _ = imp.ctx.set(ctx.clone());
        let _ = imp.feed.set(feed);
        let _ = imp.thread.set(thread);
        let _ = imp.subscriptions.set(subscriptions.clone());

        self.setup_actions();
        self.setup_zoom();
        self.setup_keyboard();

        ctx.session.connect_state_notify(glib::clone!(
            #[weak(rename_to = win)]
            self,
            move |_| win.update_banner()
        ));
        self.update_banner();
        ctx.session.check();
        subscriptions.refresh();

        let default_feed = if s.string("default-feed") == "popular" {
            Feed::Popular
        } else {
            Feed::Home
        };
        imp.sidebar.select_feed(&default_feed);
        self.show_feed(default_feed);

        // Development aid for headless screenshots in mock mode only:
        // AGORA_DEV_OPEN=t3_… opens that post after start-up.
        if ctx.mock
            && let Ok(id) = std::env::var("AGORA_DEV_OPEN")
        {
            imp.thread.get().unwrap().open_id(agora_core::PostId::new(id));
            imp.inner.set_show_content(true);
        }
    }

    fn setup_actions(&self) {
        let imp = self.imp();
        let show_sidebar = gio::PropertyAction::new("toggle-sidebar", &*imp.outer, "show-sidebar");
        self.add_action(&show_sidebar);

        let refresh = gio::ActionEntry::builder("refresh")
            .activate(|win: &Self, _, _| {
                if win.post_has_focus() {
                    win.imp().thread.get().unwrap().reload();
                } else {
                    win.imp().feed.get().unwrap().refresh();
                }
            })
            .build();
        let back = gio::ActionEntry::builder("back")
            .activate(|win: &Self, _, _| win.go_back())
            .build();
        let zoom = |name: &'static str, f: fn(f64) -> f64| {
            gio::ActionEntry::builder(name)
                .activate(move |win: &Self, _, _| {
                    let s = &win.ctx().settings;
                    let z = (f(s.double("content-zoom")) * 10.0).round() / 10.0;
                    let _ = s.set_double("content-zoom", z.clamp(0.8, 2.0));
                })
                .build()
        };
        let subscribe = gio::ActionEntry::builder("toggle-subscribe")
            .parameter_type(Some(glib::VariantTy::STRING))
            .activate(|win: &Self, _, _| win.toggle_subscribe())
            .build();
        self.add_action_entries([
            subscribe,
            refresh,
            back,
            zoom("zoom-in", |z| z + 0.1),
            zoom("zoom-out", |z| z - 0.1),
            zoom("zoom-reset", |_| 1.0),
        ]);
    }

    /// Content zoom (SPEC §7.2): scales post and comment text only.
    fn setup_zoom(&self) {
        let provider = gtk::CssProvider::new();
        gtk::style_context_add_provider_for_display(
            &WidgetExt::display(self),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
        let apply = glib::clone!(
            #[weak]
            provider,
            move |s: &gio::Settings| {
                let z = s.double("content-zoom");
                provider.load_from_string(&format!(".markdown {{ font-size: {:.0}%; }}", z * 100.0));
            }
        );
        let settings = &self.ctx().settings;
        apply(settings);
        settings.connect_changed(Some("content-zoom"), move |s, _| apply(s));
        let _ = self.imp().zoom_css.set(provider);
    }

    /// Single-letter shortcuts (SPEC §11); disabled while a text entry has focus.
    fn setup_keyboard(&self) {
        let controller = gtk::EventControllerKey::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        controller.connect_key_pressed(glib::clone!(
            #[weak(rename_to = win)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, modifiers| {
                if !(modifiers & (gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::ALT_MASK)).is_empty() {
                    return glib::Propagation::Proceed;
                }
                if gtk::prelude::GtkWindowExt::focus(&win)
                    .is_some_and(|w| w.is::<gtk::Text>() || w.is::<gtk::TextView>())
                {
                    return glib::Propagation::Proceed;
                }
                win.handle_key(key)
            }
        ));
        self.add_controller(controller);
    }

    fn handle_key(&self, key: gtk::gdk::Key) -> glib::Propagation {
        use gtk::gdk::Key;
        let imp = self.imp();
        let in_post = self.post_has_focus() || (imp.inner.is_collapsed() && imp.inner.shows_content());
        let handled = match key {
            Key::j => {
                if in_post {
                    imp.post_page.move_selection(1)
                } else {
                    imp.feed_page.move_selection(1)
                }
            }
            Key::k => {
                if in_post {
                    imp.post_page.move_selection(-1)
                } else {
                    imp.feed_page.move_selection(-1)
                }
            }
            Key::o => !in_post && imp.feed_page.open_selected(),
            Key::h => {
                self.go_back();
                true
            }
            Key::a | Key::z | Key::s => {
                let action = match key {
                    Key::a => "up",
                    Key::z => "down",
                    _ => "save",
                };
                if in_post {
                    imp.post_page.act_on_selected(action)
                } else {
                    imp.feed_page.act_on_selected(action)
                }
            }
            // Space on a focused button presses the button, as usual.
            Key::space if in_post && !self.focus_is_button() => imp.post_page.toggle_selected(),
            _ => false,
        };
        if handled {
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    }

    fn focus_is_button(&self) -> bool {
        // Space belongs to the focused control unless it is a comment row itself.
        gtk::prelude::GtkWindowExt::focus(self).is_some_and(|w| {
            w.is::<gtk::Button>()
                || w.is::<gtk::ListBoxRow>()
                || w.ancestor(gtk::Button::static_type()).is_some()
                || w.ancestor(gtk::MenuButton::static_type()).is_some()
                || !w.is_ancestor(&*self.imp().post_page)
        })
    }

    fn post_has_focus(&self) -> bool {
        gtk::prelude::GtkWindowExt::focus(self).is_some_and(|w| w.is_ancestor(&*self.imp().post_page))
    }

    fn go_back(&self) {
        let imp = self.imp();
        let thread = imp.thread.get().unwrap();
        if thread.focused() {
            thread.unfocus();
        } else if imp.inner.is_collapsed() && imp.inner.shows_content() {
            imp.inner.set_show_content(false);
            imp.feed_page.focus_list();
        } else if self.post_has_focus() {
            imp.feed_page.focus_list();
        }
    }

    /// Shows a feed in the list pane.
    pub fn show_feed(&self, feed: Feed) {
        let imp = self.imp();
        imp.feed.get().unwrap().set_feed(feed);
        imp.thread.get().unwrap().close();
        imp.inner.set_show_content(false);
        if imp.outer.is_collapsed() {
            imp.outer.set_show_sidebar(false);
        }
        imp.feed_page.focus_list();
    }

    /// Opens a post in the post pane.
    pub fn open_post(&self, post: &PostObject) {
        let imp = self.imp();
        if self.ctx().settings.boolean("mark-read-on-open") {
            post.set_read(true);
        }
        imp.thread.get().unwrap().open(post);
        imp.inner.set_show_content(true);
    }

    fn link_handler(&self) -> LinkHandler {
        let win = self.downgrade();
        Rc::new(move |uri: &str| {
            if let Some(win) = win.upgrade() {
                win.open_link(uri);
            }
        })
    }

    /// Opens a link: Reddit names in Agora, everything else in the browser
    /// (SPEC §12.1; the full URL router arrives in slice 5).
    pub fn open_link(&self, uri: &str) {
        if let Some(name) = uri.strip_prefix("agora:r/") {
            let feed = Feed::Subreddit(SubredditName::new(name));
            self.imp().sidebar.select_feed(&feed);
            self.show_feed(feed);
            return;
        }
        if let Some(name) = uri.strip_prefix("agora:u/") {
            self.show_feed(Feed::User(Username::new(name)));
            return;
        }
        let uri = if uri.starts_with('/') {
            util::reddit_url(uri)
        } else {
            uri.to_string()
        };
        gtk::UriLauncher::new(&uri).launch(Some(self), gio::Cancellable::NONE, |_| {});
    }

    /// Joins or leaves the subreddit shown in the list pane.
    fn toggle_subscribe(&self) {
        let imp = self.imp();
        let Some(sub) = imp.feed.get().unwrap().subreddit() else {
            return;
        };
        let subscribed = !sub.subscribed();
        sub.set_subscribed(subscribed);
        let fut = self.ctx().backend.subscribe(sub.subreddit(), subscribed);
        let win = self.downgrade();
        glib::spawn_future_local(async move {
            let result = fut.await;
            let Some(win) = win.upgrade() else { return };
            match result {
                Ok(()) => win.imp().subscriptions.get().unwrap().refresh(),
                Err(e) => {
                    sub.set_subscribed(!subscribed);
                    win.ctx().toast_error(&e, None);
                }
            }
        });
    }

    fn update_banner(&self) {
        let imp = self.imp();
        let ctx = self.ctx();
        let (title, button, action): (String, Option<String>, Option<BannerAction>) = match ctx.session.state() {
            SessionState::Expired => (gettext("Signed out of Reddit"), None, None),
            SessionState::Blocked => (
                gettext("Reddit asked Agora to slow down. Agora will wait before trying again."),
                None,
                None,
            ),
            _ if ctx.mock => (
                gettext("Showing sample data. Signing in to Reddit arrives in a later build."),
                Some(gettext("_About")),
                Some(Rc::new({
                    let win = self.downgrade();
                    move || {
                        if let Some(win) = win.upgrade() {
                            let _ = WidgetExt::activate_action(&win, "app.about", None);
                        }
                    }
                })),
            ),
            _ => (String::new(), None, None),
        };
        imp.banner.set_title(&title);
        imp.banner.set_button_label(button.as_deref());
        imp.banner_action.replace(action);
        imp.banner.set_revealed(!title.is_empty());
    }
}
