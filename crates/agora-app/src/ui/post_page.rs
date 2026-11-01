use std::cell::OnceCell;
use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use agora_core::{CommentSort, VoteState};
use gettextrs::gettext;
use gtk::{gio, glib};

use super::PostHeader;
use super::comment_row::{self, CommentRow};
use crate::render::LinkHandler;
use crate::stores::{CommentObject, CommentRowKind, Context, FooterObject, LoadState, PostObject, ThreadStore, Writes};
use crate::util;

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::AgoraPostPage)]
    #[template(resource = "/io/github/ClickCalickClick/Agora/ui/post_page.ui")]
    pub struct AgoraPostPage {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub sort_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub overflow_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub list_view: TemplateChild<gtk::ListView>,
        pub ctx: OnceCell<Rc<Context>>,
        pub store: OnceCell<ThreadStore>,
        pub writes: OnceCell<Writes>,
        pub selection: OnceCell<gtk::SingleSelection>,
        pub header: OnceCell<gio::ListStore>,
        pub focus_note: OnceCell<gio::ListStore>,
        pub footer: OnceCell<FooterObject>,
        /// `post.sort` state: a comment sort key.
        #[property(get, set = Self::set_sort_target)]
        pub sort_target: std::cell::RefCell<String>,
        pub last_state: std::cell::Cell<LoadState>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AgoraPostPage {
        const NAME: &'static str = "AgoraPostPage";
        type Type = super::AgoraPostPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();
            klass.install_property_action("post.sort", "sort-target");
            klass.install_action("post.copy-link", None, |page, _, _| page.copy_link());
            klass.install_action("post.open-in-browser", None, |page, _, _| page.open_in_browser());
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for AgoraPostPage {}

    impl AgoraPostPage {
        fn set_sort_target(&self, key: String) {
            if *self.sort_target.borrow() == key {
                return;
            }
            self.sort_target.replace(key.clone());
            if let (Some(sort), Some(store)) = (CommentSort::from_key(&key), self.store.get()) {
                store.set_sort(sort);
            }
        }
    }
    impl WidgetImpl for AgoraPostPage {}
    impl NavigationPageImpl for AgoraPostPage {}
}

glib::wrapper! {
    /// The post view: the post, then its comment tree (SPEC §7.2, §8).
    pub struct AgoraPostPage(ObjectSubclass<imp::AgoraPostPage>)
        @extends gtk::Widget, adw::NavigationPage,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[gtk::template_callbacks]
impl AgoraPostPage {
    #[template_callback]
    fn on_activate(&self, position: u32) {
        let Some(item) = self.imp().selection.get().and_then(|s| s.item(position)) else {
            return;
        };
        if let Ok(row) = item.downcast::<CommentObject>() {
            self.activate_row(&row);
        }
    }
}

impl AgoraPostPage {
    /// Connects the page to its store.
    pub fn setup(&self, ctx: &Rc<Context>, store: &ThreadStore, writes: &Writes, on_link: LinkHandler) {
        let imp = self.imp();
        let header = gio::ListStore::new::<PostObject>();
        let focus_note = gio::ListStore::new::<gtk::StringObject>();
        let footer = FooterObject::default();
        let footer_store = gio::ListStore::new::<FooterObject>();
        footer_store.append(&footer);
        let parts = gio::ListStore::new::<gio::ListModel>();
        parts.append(&header);
        parts.append(&focus_note);
        parts.append(&store.rows());
        parts.append(&footer_store);
        let selection = gtk::SingleSelection::builder()
            .model(&gtk::FlattenListModel::new(Some(parts)))
            .autoselect(false)
            .can_unselect(true)
            .build();
        imp.list_view.set_model(Some(&selection));
        imp.list_view
            .set_factory(Some(&self.factory(ctx, store, writes, on_link)));

        for prop in ["state", "error", "post", "focused", "sort-key"] {
            store.connect_notify_local(
                Some(prop),
                glib::clone!(
                    #[weak(rename_to = page)]
                    self,
                    move |_, _| page.update()
                ),
            );
        }
        let _ = imp.ctx.set(ctx.clone());
        let _ = imp.store.set(store.clone());
        let _ = imp.writes.set(writes.clone());
        let _ = imp.selection.set(selection);
        let _ = imp.header.set(header);
        let _ = imp.focus_note.set(focus_note);
        let _ = imp.footer.set(footer);
        self.update();
    }

    fn factory(
        &self,
        ctx: &Rc<Context>,
        store: &ThreadStore,
        writes: &Writes,
        on_link: LinkHandler,
    ) -> gtk::SignalListItemFactory {
        let factory = gtk::SignalListItemFactory::new();
        let (ctx, writes) = (ctx.clone(), writes.clone());
        factory.connect_bind(glib::clone!(
            #[weak]
            store,
            move |_, item| {
                let item = item.downcast_ref::<gtk::ListItem>().unwrap();
                let obj = item.item();
                if let Some(row) = obj.and_downcast_ref::<CommentObject>() {
                    let widget = item.child().and_downcast::<CommentRow>().unwrap_or_else(|| {
                        let w = CommentRow::default();
                        item.set_child(Some(&w));
                        w
                    });
                    let op = store.post().and_then(|p| p.with(|p| p.author.clone()));
                    let me = ctx.session.username();
                    widget.bind(row, &store, &writes, &on_link, op.as_ref().map(|a| a.as_str()), &me);
                    item.set_activatable(row.kind() != CommentRowKind::Comment);
                    item.set_selectable(true);
                    item.set_accessible_label(&comment_row::accessible_text(row));
                } else if let Some(post) = obj.and_downcast_ref::<PostObject>() {
                    let widget = item.child().and_downcast::<PostHeader>().unwrap_or_else(|| {
                        let w = PostHeader::default();
                        item.set_child(Some(&w));
                        w
                    });
                    widget.bind(post, &writes, &on_link);
                    item.set_activatable(false);
                    item.set_selectable(false);
                    item.set_focusable(false);
                    post.with(|p| item.set_accessible_label(&p.title));
                } else if obj.and_downcast_ref::<gtk::StringObject>().is_some() {
                    item.set_activatable(false);
                    item.set_selectable(false);
                    item.set_child(Some(&focus_note_widget(&store)));
                } else if let Some(footer) = obj.and_downcast_ref::<FooterObject>() {
                    item.set_activatable(false);
                    item.set_selectable(false);
                    item.set_focusable(false);
                    item.set_child(Some(&comments_footer(footer, &store)));
                }
            }
        ));
        factory.connect_unbind(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            if let Some(w) = item.child().and_downcast::<CommentRow>() {
                w.unbind();
            } else if let Some(w) = item.child().and_downcast::<PostHeader>() {
                w.unbind();
            } else if let Some(child) = item.child() {
                super::release_watch(&child);
            }
        });
        factory
    }

    fn update(&self) {
        let imp = self.imp();
        let Some(store) = imp.store.get() else { return };
        let header = imp.header.get().unwrap();
        let post = store.post();
        let current = header.item(0).and_downcast::<PostObject>();
        if current != post {
            match &post {
                Some(p) => header.splice(0, header.n_items(), std::slice::from_ref(p)),
                None => header.remove_all(),
            }
        } else if let Some(p) = &post {
            // Same object, fresh data: rebind the header row.
            header.splice(0, 1, std::slice::from_ref(p));
        }
        let note = imp.focus_note.get().unwrap();
        match (store.focused(), note.n_items()) {
            (true, 0) => note.append(&gtk::StringObject::new("focus")),
            (false, n) if n > 0 => note.remove_all(),
            _ => {}
        }
        imp.footer.get().unwrap().set_state(store.state());
        // A freshly loaded thread starts at the top, wherever the list was.
        if imp.last_state.replace(store.state()) == LoadState::Loading && store.state() == LoadState::Done {
            let list = imp.list_view.get();
            glib::idle_add_local_once(move || list.scroll_to(0, gtk::ListScrollFlags::NONE, None));
        }
        let key = store.sort_key();
        if imp.sort_target.replace(key.clone()) != key {
            self.notify_sort_target();
        }
        let has_post = post.is_some();
        imp.sort_button.set_sensitive(has_post);
        imp.overflow_button.set_sensitive(has_post);
        if let Some(p) = &post {
            p.with(|p| self.set_title(&p.title));
        } else {
            self.set_title(&gettext("Post"));
        }
        imp.stack
            .set_visible_child_name(if has_post || store.state() != LoadState::Idle {
                "content"
            } else {
                "empty"
            });
    }

    fn activate_row(&self, row: &CommentObject) {
        let Some(store) = self.imp().store.get() else { return };
        match row.kind() {
            CommentRowKind::Comment => store.toggle_collapsed(row),
            CommentRowKind::More => store.load_more(row),
            CommentRowKind::Continue => {
                if let Some(id) = row.continue_parent() {
                    store.focus_comment(id);
                }
            }
        }
    }

    fn open_in_browser(&self) {
        if let Some(post) = self.imp().store.get().and_then(|s| s.post()) {
            let url = post.with(|p| util::reddit_url(&p.permalink));
            let root = self.root().and_downcast::<gtk::Window>();
            gtk::UriLauncher::new(&url).launch(root.as_ref(), gio::Cancellable::NONE, |_| {});
        }
    }

    fn copy_link(&self) {
        let Some(post) = self.imp().store.get().and_then(|s| s.post()) else {
            return;
        };
        let url = post.with(|p| util::reddit_url(&p.permalink));
        self.clipboard().set_text(&url);
        if let Some(ctx) = self.imp().ctx.get() {
            ctx.toast(gettext("Link copied"), None);
        }
    }

    fn selected_row(&self) -> Option<CommentObject> {
        self.imp()
            .selection
            .get()?
            .selected_item()
            .and_downcast::<CommentObject>()
    }

    /// Moves the selection to the next or previous comment row (j/k).
    pub fn move_selection(&self, delta: i32) -> bool {
        let imp = self.imp();
        let Some(selection) = imp.selection.get() else {
            return false;
        };
        let n = selection.n_items() as i32;
        let current = selection.selected();
        let mut pos = if current == gtk::INVALID_LIST_POSITION {
            if delta > 0 { -1 } else { n }
        } else {
            current as i32
        };
        loop {
            pos += delta;
            if pos < 0 || pos >= n {
                return true;
            }
            if selection.item(pos as u32).is_some_and(|i| i.is::<CommentObject>()) {
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

    /// Collapses or expands the selected comment (Space).
    pub fn toggle_selected(&self) -> bool {
        match self.selected_row() {
            Some(row) => {
                self.activate_row(&row);
                if row.kind() == CommentRowKind::Comment {
                    // Rebinding replaced the label; keep screen readers in sync.
                    if let Some(sel) = self.imp().selection.get() {
                        let pos = sel.selected();
                        self.imp().list_view.scroll_to(pos, gtk::ListScrollFlags::FOCUS, None);
                    }
                }
                true
            }
            None => false,
        }
    }

    /// Votes or saves the selected comment, or the post if none is selected.
    pub fn act_on_selected(&self, action: &str) -> bool {
        let imp = self.imp();
        let writes = imp.writes.get().unwrap();
        if let Some(row) = self.selected_row().filter(|r| r.kind() == CommentRowKind::Comment) {
            match action {
                "up" => writes.vote_comment(&row, VoteState::Up),
                "down" => writes.vote_comment(&row, VoteState::Down),
                "save" => writes.toggle_save_comment(&row),
                _ => return false,
            }
            return true;
        }
        let Some(post) = imp.store.get().and_then(|s| s.post()) else {
            return false;
        };
        match action {
            "up" => writes.vote_post(&post, VoteState::Up),
            "down" => writes.vote_post(&post, VoteState::Down),
            "save" => writes.toggle_save_post(&post),
            _ => return false,
        }
        true
    }
}

fn focus_note_widget(store: &ThreadStore) -> gtk::Widget {
    let bar = gtk::Box::builder()
        .spacing(12)
        .margin_start(12)
        .margin_end(12)
        .margin_top(6)
        .margin_bottom(6)
        .css_classes(["card", "focus-note"])
        .build();
    let label = gtk::Label::builder()
        .label(gettext("Showing one branch of the thread"))
        .hexpand(true)
        .xalign(0.0)
        .wrap(true)
        .build();
    let button = gtk::Button::builder()
        .label(gettext("_View All Comments"))
        .use_underline(true)
        .css_classes(["pill", "small"])
        .build();
    button.connect_clicked(glib::clone!(
        #[weak]
        store,
        move |_| store.unfocus()
    ));
    bar.append(&label);
    bar.append(&button);
    bar.upcast()
}

fn comments_footer(footer: &FooterObject, store: &ThreadStore) -> gtk::Widget {
    let stack = gtk::Stack::builder().margin_top(24).margin_bottom(24).build();
    stack.add_named(&gtk::Box::new(gtk::Orientation::Vertical, 0), Some("none"));
    stack.add_named(
        &adw::Spinner::builder().width_request(32).height_request(32).build(),
        Some("loading"),
    );
    let error = adw::StatusPage::builder()
        .icon_name("network-error-symbolic")
        .title(gettext("Could Not Load Comments"))
        .css_classes(["compact"])
        .build();
    let retry = gtk::Button::builder()
        .label(gettext("_Retry"))
        .use_underline(true)
        .halign(gtk::Align::Center)
        .css_classes(["pill"])
        .build();
    retry.connect_clicked(glib::clone!(
        #[weak]
        store,
        move |_| store.reload()
    ));
    error.set_child(Some(&retry));
    stack.add_named(&error, Some("error"));
    let empty = adw::StatusPage::builder()
        .icon_name("agora-comments-symbolic")
        .title(gettext("No Comments Yet"))
        .css_classes(["compact"])
        .build();
    stack.add_named(&empty, Some("empty"));
    let update = glib::clone!(
        #[weak]
        stack,
        #[weak]
        store,
        #[weak]
        error,
        move |f: &FooterObject| {
            stack.set_visible_child_name(match f.state() {
                LoadState::Loading => "loading",
                LoadState::Error => {
                    error.set_description(Some(&store.error()));
                    "error"
                }
                LoadState::Done if store.rows().n_items() == 0 => "empty",
                _ => "none",
            });
        }
    );
    update(footer);
    let handler = footer.connect_state_notify(update);
    super::attach_watch(&stack, footer, handler);
    stack.upcast()
}
