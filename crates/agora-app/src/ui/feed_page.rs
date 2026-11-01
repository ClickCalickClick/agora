use std::cell::{OnceCell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use agora_core::{Feed, Sort, TimeRange, VoteState};
use gettextrs::gettext;
use gtk::{gio, glib};

use super::{AgoraPostRow, SubredditHeader};
use crate::i18n::i18n_f;
use crate::stores::{FeedStore, FooterObject, LoadState, PostObject, SubredditObject, Writes};

/// Load the next page when the user scrolls within about this many rows of
/// the end (SPEC §4.6), using a typical row height.
const LOAD_AHEAD_ROWS: f64 = 10.0;
const TYPICAL_ROW_HEIGHT: f64 = 64.0;

type OpenHandler = Box<dyn Fn(&PostObject)>;

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::AgoraFeedPage)]
    #[template(resource = "/io/github/ClickCalickClick/Agora/ui/feed_page.ui")]
    pub struct AgoraFeedPage {
        #[template_child]
        pub sidebar_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub title_widget: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub sort_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub status_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub status_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub scroller: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub list_view: TemplateChild<gtk::ListView>,
        pub store: OnceCell<FeedStore>,
        pub writes: OnceCell<Writes>,
        pub selection: OnceCell<gtk::SingleSelection>,
        pub header: OnceCell<gio::ListStore>,
        pub footer: OnceCell<FooterObject>,
        pub footer_store: OnceCell<gio::ListStore>,
        pub on_open: OnceCell<OpenHandler>,
        /// `feed.sort` state: `best`, `hot`, `new`, `rising` or `top:<range>`.
        #[property(get, set = Self::set_sort_target)]
        pub sort_target: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AgoraFeedPage {
        const NAME: &'static str = "AgoraFeedPage";
        type Type = super::AgoraFeedPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();
            // Widget actions (not an inserted group) so header buttons and
            // menus resolve them from the template.
            klass.install_action("feed.refresh", None, |page, _, _| {
                if let Some(store) = page.imp().store.get() {
                    store.refresh();
                }
            });
            klass.install_property_action("feed.sort", "sort-target");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for AgoraFeedPage {}

    impl AgoraFeedPage {
        fn set_sort_target(&self, target: String) {
            if *self.sort_target.borrow() == target {
                return;
            }
            self.sort_target.replace(target.clone());
            let (key, range) = target.split_once(':').unwrap_or((&target, ""));
            let range = TimeRange::from_key(range).unwrap_or_default();
            if let (Some(sort), Some(store)) = (Sort::from_key(key, range), self.store.get()) {
                store.set_sort(sort);
            }
        }
    }
    impl WidgetImpl for AgoraFeedPage {}
    impl NavigationPageImpl for AgoraFeedPage {}
}

glib::wrapper! {
    /// The post list pane (SPEC §5.3, §7.1).
    pub struct AgoraFeedPage(ObjectSubclass<imp::AgoraFeedPage>)
        @extends gtk::Widget, adw::NavigationPage,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[gtk::template_callbacks]
impl AgoraFeedPage {
    #[template_callback]
    fn on_activate(&self, position: u32) {
        let imp = self.imp();
        let Some(item) = imp.selection.get().and_then(|s| s.item(position)) else {
            return;
        };
        if let Ok(post) = item.downcast::<PostObject>() {
            imp.selection.get().unwrap().set_selected(position);
            if let Some(f) = imp.on_open.get() {
                f(&post);
            }
        }
    }
}

impl AgoraFeedPage {
    /// Connects the page to its store. `on_open` runs when a post is opened.
    pub fn setup(&self, store: &FeedStore, writes: &Writes, on_open: impl Fn(&PostObject) + 'static) {
        let imp = self.imp();
        let _ = imp.on_open.set(Box::new(on_open));
        let _ = imp.writes.set(writes.clone());

        // Model: [subreddit header?] + posts + [footer].
        let header = gio::ListStore::new::<SubredditObject>();
        let footer = FooterObject::default();
        // The footer joins the model only once there are posts: an anchored
        // footer at position 0 would make the list follow it as pages arrive.
        let footer_store = gio::ListStore::new::<FooterObject>();
        let parts = gio::ListStore::new::<gio::ListModel>();
        parts.append(&header);
        parts.append(&store.posts());
        parts.append(&footer_store);
        let flat = gtk::FlattenListModel::new(Some(parts));
        let selection = gtk::SingleSelection::builder()
            .model(&flat)
            .autoselect(false)
            .can_unselect(true)
            .build();
        imp.list_view.set_model(Some(&selection));
        imp.list_view.set_factory(Some(&self.factory(store)));

        // Actions: feed.refresh and feed.sort.

        for prop in ["state", "error", "title", "sort-key", "top-range"] {
            store.connect_notify_local(
                Some(prop),
                glib::clone!(
                    #[weak(rename_to = page)]
                    self,
                    move |_, _| page.update()
                ),
            );
        }
        store.connect_subreddit_notify(glib::clone!(
            #[weak]
            header,
            move |store| {
                match store.subreddit() {
                    Some(sub) => header.splice(0, header.n_items(), &[sub]),
                    None => header.remove_all(),
                }
            }
        ));
        // update() may add or remove the footer, which changes the flattened
        // model; doing that inside the posts' own items-changed emission left
        // the list view's rows inconsistent (GTK criticals, rows missing from
        // the accessibility tree). Run it after the emission instead.
        store.posts().connect_items_changed(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move |_, _, _, _| {
                glib::idle_add_local_once(glib::clone!(
                    #[weak]
                    page,
                    move || page.update()
                ));
            }
        ));
        // Infinite scroll, driven by the user's scrolling only: nothing loads
        // ahead unless the user is near the end (no prefetch).
        let adj = imp.scroller.vadjustment();
        let near_end = glib::clone!(
            #[weak]
            store,
            move |adj: &gtk::Adjustment| {
                let remaining = adj.upper() - adj.value() - adj.page_size();
                if remaining < LOAD_AHEAD_ROWS * TYPICAL_ROW_HEIGHT && store.posts().n_items() > 0 {
                    store.load_more();
                }
            }
        );
        adj.connect_value_changed(near_end.clone());
        // A first page shorter than a tall window cannot be scrolled; it
        // counts as being at the end, which is what the user sees.
        adj.connect_upper_notify(near_end);
        let _ = imp.store.set(store.clone());
        let _ = imp.selection.set(selection);
        let _ = imp.header.set(header);
        let _ = imp.footer.set(footer);
        let _ = imp.footer_store.set(footer_store);

        let window_bound = glib::clone!(
            #[weak(rename_to = page)]
            self,
            move || page.bind_sidebar_button()
        );
        self.connect_realize(move |_| window_bound());
        self.update();
    }

    /// Shows the sidebar toggle only when the sidebar can be hidden.
    fn bind_sidebar_button(&self) {
        let Some(split) = self
            .ancestor(adw::OverlaySplitView::static_type())
            .and_downcast::<adw::OverlaySplitView>()
        else {
            return;
        };
        let button = self.imp().sidebar_button.get();
        let update = glib::clone!(
            #[weak]
            button,
            move |split: &adw::OverlaySplitView| button.set_visible(split.is_collapsed() || !split.shows_sidebar())
        );
        update(&split);
        split.connect_collapsed_notify(update.clone());
        split.connect_show_sidebar_notify(update);
    }

    fn factory(&self, store: &FeedStore) -> gtk::SignalListItemFactory {
        let factory = gtk::SignalListItemFactory::new();
        let settings = gio::Settings::new(crate::config::SCHEMA_ID);
        factory.connect_bind(glib::clone!(
            #[weak]
            store,
            move |_, item| {
                let item = item.downcast_ref::<gtk::ListItem>().unwrap();
                let obj = item.item();
                if let Some(post) = obj.and_downcast_ref::<PostObject>() {
                    let row = item.child().and_downcast::<AgoraPostRow>().unwrap_or_else(|| {
                        let row = AgoraPostRow::default();
                        item.set_child(Some(&row));
                        row
                    });
                    item.set_activatable(true);
                    item.set_selectable(true);
                    let in_sub = matches!(store.feed(), Some(Feed::Subreddit(_)));
                    row.bind(item, post, !in_sub, settings.boolean("show-scores"));
                } else if let Some(sub) = obj.and_downcast_ref::<SubredditObject>() {
                    let header = item.child().and_downcast::<SubredditHeader>().unwrap_or_else(|| {
                        let h = SubredditHeader::default();
                        item.set_child(Some(&h));
                        h
                    });
                    item.set_activatable(false);
                    item.set_selectable(false);
                    header.bind(sub);
                    item.set_accessible_label(&sub.subreddit().name.prefixed());
                } else if let Some(footer) = obj.and_downcast_ref::<FooterObject>() {
                    item.set_activatable(false);
                    item.set_selectable(false);
                    item.set_child(Some(&footer_widget(footer, &store)));
                }
            }
        ));
        factory.connect_unbind(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            if let Some(row) = item.child().and_downcast::<AgoraPostRow>() {
                row.unbind();
            } else if let Some(child) = item.child() {
                super::release_watch(&child);
            }
        });
        factory
    }

    fn update(&self) {
        let imp = self.imp();
        let Some(store) = imp.store.get() else { return };
        imp.title_widget.set_title(&store.title());
        imp.title_widget.set_subtitle(&sort_label(store.sort()));
        let target = sort_target(store.sort());
        if imp.sort_target.replace(target.clone()) != target {
            self.notify_sort_target();
        }
        imp.sort_button.set_visible(store.feed() != Some(Feed::Saved));
        let empty = store.posts().n_items() == 0;
        let state = store.state();
        if let (Some(footer), Some(footer_store)) = (imp.footer.get(), imp.footer_store.get()) {
            footer.set_state(state);
            match (empty, footer_store.n_items()) {
                (true, n) if n > 0 => footer_store.remove_all(),
                (false, 0) => footer_store.append(footer),
                _ => {}
            }
        }
        let page = match (state, empty) {
            (LoadState::Loading | LoadState::Idle, true) => "loading",
            (LoadState::Error, true) => {
                imp.status_page.set_icon_name(Some("network-error-symbolic"));
                imp.status_page.set_title(&gettext("Could Not Load Posts"));
                imp.status_page.set_description(Some(&store.error()));
                imp.status_button.set_visible(true);
                "status"
            }
            (LoadState::Done, true) => {
                imp.status_page.set_icon_name(Some("agora-plaza-symbolic"));
                imp.status_page.set_title(&gettext("No Posts Here Yet"));
                imp.status_page.set_description(
                    match store.feed() {
                        Some(Feed::Saved) => Some(gettext("Posts you save show up here")),
                        Some(Feed::Home) => Some(gettext("Join a few communities to fill your Home feed")),
                        _ => None,
                    }
                    .as_deref(),
                );
                imp.status_button.set_visible(false);
                "status"
            }
            _ => "list",
        };
        imp.stack.set_visible_child_name(page);
    }

    /// Moves the selection by `delta` rows (j/k), skipping header and footer.
    pub fn move_selection(&self, delta: i32) -> bool {
        let imp = self.imp();
        let Some(selection) = imp.selection.get() else {
            return false;
        };
        let n = selection.n_items();
        let current = selection.selected();
        let mut pos = if current == gtk::INVALID_LIST_POSITION {
            if delta > 0 { -1 } else { n as i32 }
        } else {
            current as i32
        };
        loop {
            pos += delta;
            if pos < 0 || pos >= n as i32 {
                return true;
            }
            if selection.item(pos as u32).is_some_and(|i| i.is::<PostObject>()) {
                break;
            }
        }
        imp.list_view.scroll_to(
            pos as u32,
            gtk::ListScrollFlags::SELECT | gtk::ListScrollFlags::FOCUS,
            None,
        );
        true
    }

    /// Opens the selected post (o).
    pub fn open_selected(&self) -> bool {
        let Some(selection) = self.imp().selection.get() else {
            return false;
        };
        let pos = selection.selected();
        if pos == gtk::INVALID_LIST_POSITION {
            return false;
        }
        self.on_activate(pos);
        true
    }

    /// Votes or saves the selected post (a, z, s).
    pub fn act_on_selected(&self, action: &str) -> bool {
        let imp = self.imp();
        let Some(post) = imp
            .selection
            .get()
            .and_then(|s| s.selected_item())
            .and_downcast::<PostObject>()
        else {
            return false;
        };
        let writes = imp.writes.get().unwrap();
        match action {
            "up" => writes.vote_post(&post, VoteState::Up),
            "down" => writes.vote_post(&post, VoteState::Down),
            "save" => writes.toggle_save_post(&post),
            _ => return false,
        }
        true
    }

    /// Moves keyboard focus into the list.
    pub fn focus_list(&self) {
        self.imp().list_view.grab_focus();
    }
}

fn footer_widget(footer: &FooterObject, store: &FeedStore) -> gtk::Widget {
    let stack = gtk::Stack::builder().margin_top(12).margin_bottom(12).build();
    stack.add_named(&gtk::Box::new(gtk::Orientation::Vertical, 0), Some("none"));
    let spinner = adw::Spinner::builder().width_request(24).height_request(24).build();
    stack.add_named(&spinner, Some("loading"));
    let retry = gtk::Button::builder()
        .label(gettext("_Retry"))
        .use_underline(true)
        .halign(gtk::Align::Center)
        .build();
    retry.add_css_class("pill");
    retry.connect_clicked(glib::clone!(
        #[weak]
        store,
        move |_| store.load_more()
    ));
    let error_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .build();
    let error_label = gtk::Label::builder().wrap(true).css_classes(["dim-label"]).build();
    error_box.append(&error_label);
    error_box.append(&retry);
    stack.add_named(&error_box, Some("error"));
    let end = gtk::Label::builder()
        .label(gettext("You're all caught up"))
        .css_classes(["dim-label", "caption"])
        .build();
    stack.add_named(&end, Some("done"));
    let update = glib::clone!(
        #[weak]
        stack,
        #[weak]
        store,
        #[weak]
        error_label,
        move |f: &FooterObject| {
            stack.set_visible_child_name(match f.state() {
                LoadState::Idle => "none",
                LoadState::Loading => "loading",
                LoadState::Error => {
                    error_label.set_label(&store.error());
                    "error"
                }
                LoadState::Done => "done",
            });
        }
    );
    update(footer);
    let handler = footer.connect_state_notify(update);
    super::attach_watch(&stack, footer, handler);
    stack.upcast()
}

fn sort_target(sort: Sort) -> String {
    match sort {
        Sort::Top(range) => format!("top:{}", range.key()),
        other => other.key().to_string(),
    }
}

fn sort_label(sort: Sort) -> String {
    match sort {
        Sort::Best => gettext("Best"),
        Sort::Hot => gettext("Hot"),
        Sort::New => gettext("New"),
        Sort::Rising => gettext("Rising"),
        Sort::Top(range) => {
            let range = match range {
                TimeRange::Hour => gettext("Past Hour"),
                TimeRange::Day => gettext("Past Day"),
                TimeRange::Week => gettext("Past Week"),
                TimeRange::Month => gettext("Past Month"),
                TimeRange::Year => gettext("Past Year"),
                TimeRange::All => gettext("All Time"),
            };
            i18n_f("Top · {}", &[&range])
        }
    }
}
