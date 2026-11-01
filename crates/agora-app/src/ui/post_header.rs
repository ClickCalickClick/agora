use std::cell::RefCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use agora_core::{PostKind, VoteState};
use gettextrs::gettext;
use gtk::glib;

use super::post_row::badges;
use crate::render::{self, LinkHandler};
use crate::stores::{PostObject, Writes};
use crate::util;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct PostHeader {
        pub content: gtk::Box,
        pub up: gtk::ToggleButton,
        pub down: gtk::ToggleButton,
        pub score: gtk::Label,
        pub comments: gtk::Label,
        pub save: gtk::ToggleButton,
        pub post: RefCell<Option<PostObject>>,
        pub handlers: RefCell<Vec<glib::SignalHandlerId>>,
        pub writes: RefCell<Option<Writes>>,
        pub updating: std::cell::Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PostHeader {
        const NAME: &'static str = "AgoraPostHeader";
        type Type = super::PostHeader;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for PostHeader {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            let root = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(12)
                .margin_top(18)
                .margin_bottom(6)
                .margin_start(12)
                .margin_end(12)
                .build();
            self.content.set_orientation(gtk::Orientation::Vertical);
            self.content.set_spacing(12);
            root.append(&self.content);

            // Action bar: vote, score, comments, save, copy link (SPEC §7.2).
            let bar = gtk::Box::builder().spacing(4).css_classes(["post-actions"]).build();
            self.up.set_icon_name("agora-upvote-symbolic");
            self.up.set_tooltip_text(Some(&gettext("Upvote")));
            self.up.add_css_class("flat");
            self.up.add_css_class("upvote");
            self.down.set_icon_name("agora-downvote-symbolic");
            self.down.set_tooltip_text(Some(&gettext("Downvote")));
            self.down.add_css_class("flat");
            self.down.add_css_class("downvote");
            self.score.add_css_class("numeric");
            self.score.add_css_class("heading");
            self.score.add_css_class("score");
            self.score.set_width_chars(3);
            bar.append(&self.up);
            bar.append(&self.score);
            bar.append(&self.down);
            let comments = gtk::Box::builder().spacing(6).margin_start(12).build();
            comments.append(&gtk::Image::from_icon_name("agora-comments-symbolic"));
            self.comments.add_css_class("numeric");
            comments.append(&self.comments);
            bar.append(&comments);
            let spacer = gtk::Box::builder().hexpand(true).build();
            bar.append(&spacer);
            self.save.set_icon_name("bookmark-new-symbolic");
            self.save.set_tooltip_text(Some(&gettext("Save")));
            self.save.add_css_class("flat");
            bar.append(&self.save);
            let copy = gtk::Button::builder()
                .icon_name("edit-copy-symbolic")
                .tooltip_text(gettext("Copy Link"))
                .action_name("post.copy-link")
                .css_classes(["flat"])
                .build();
            bar.append(&copy);
            root.append(&bar);
            root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            obj.set_child(Some(&root));

            let weak = obj.downgrade();
            let vote = move |state: VoteState| {
                let weak = weak.clone();
                move |_: &gtk::ToggleButton| {
                    let Some(obj) = weak.upgrade() else { return };
                    let imp = obj.imp();
                    if imp.updating.get() {
                        return;
                    }
                    if let (Some(post), Some(writes)) = (imp.post.borrow().clone(), imp.writes.borrow().clone()) {
                        writes.vote_post(&post, state);
                    }
                }
            };
            self.up.connect_toggled(vote(VoteState::Up));
            self.down.connect_toggled(vote(VoteState::Down));
            let weak = obj.downgrade();
            self.save.connect_toggled(move |_| {
                let Some(obj) = weak.upgrade() else { return };
                let imp = obj.imp();
                if imp.updating.get() {
                    return;
                }
                if let (Some(post), Some(writes)) = (imp.post.borrow().clone(), imp.writes.borrow().clone()) {
                    writes.toggle_save_post(&post);
                }
            });
        }
    }

    impl WidgetImpl for PostHeader {}
    impl BinImpl for PostHeader {}
}

glib::wrapper! {
    /// The post itself at the top of the post view (SPEC §7.2).
    pub struct PostHeader(ObjectSubclass<imp::PostHeader>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for PostHeader {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl PostHeader {
    /// Shows `post`.
    pub fn bind(&self, post: &PostObject, writes: &Writes, on_link: &LinkHandler) {
        self.unbind();
        let imp = self.imp();
        imp.writes.replace(Some(writes.clone()));
        while let Some(child) = imp.content.first_child() {
            imp.content.remove(&child);
        }
        post.with(|p| {
            // Meta line with links to the subreddit and author.
            let mut meta = format!(
                "<a href=\"agora:r/{0}\">r/{0}</a>",
                glib::markup_escape_text(p.subreddit.as_str())
            );
            meta.push_str(" · ");
            match &p.author {
                Some(a) => meta.push_str(&format!(
                    "<a href=\"agora:u/{0}\">u/{0}</a>",
                    glib::markup_escape_text(a.as_str())
                )),
                None => meta.push_str(&glib::markup_escape_text(&gettext("[deleted]"))),
            }
            meta.push_str(" · ");
            meta.push_str(&glib::markup_escape_text(&util::short_age(p.created)));
            if p.edited {
                meta.push_str(" · ");
                meta.push_str(&glib::markup_escape_text(&gettext("edited")));
            }
            let meta_label = gtk::Label::builder()
                .use_markup(true)
                .label(meta)
                .xalign(0.0)
                .wrap(true)
                .build();
            meta_label.add_css_class("dim-label");
            meta_label.add_css_class("caption");
            let on = on_link.clone();
            meta_label.connect_activate_link(move |_, uri| {
                on(uri);
                glib::Propagation::Stop
            });
            imp.content.append(&meta_label);

            let title = gtk::Label::builder()
                .label(&p.title)
                .xalign(0.0)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .selectable(true)
                .css_classes(["title-2", "content-title"])
                .build();
            title.set_accessible_role(gtk::AccessibleRole::Heading);
            imp.content.append(&title);

            let mut tags = badges(p);
            if let Some(flair) = &p.flair {
                tags.insert(0, flair.clone());
            }
            if !tags.is_empty() {
                let row = gtk::Box::builder().spacing(4).build();
                for t in tags {
                    row.append(&gtk::Label::builder().label(t).css_classes(["badge", "caption"]).build());
                }
                imp.content.append(&row);
            }

            match &p.kind {
                PostKind::Text => {}
                PostKind::Link { url, domain } => imp.content.append(&link_card(domain, url, on_link)),
                PostKind::Image { url } | PostKind::Video { url } => {
                    imp.content.append(&link_card(&gettext("Media"), url, on_link));
                }
                PostKind::Gallery { urls } => {
                    if let Some(url) = urls.first() {
                        imp.content.append(&link_card(&gettext("Gallery"), url, on_link));
                    }
                }
            }
            if let Some(body) = &p.body {
                let doc = render::parse_body(body);
                if !doc.is_empty() {
                    imp.content.append(&render::render(&doc, on_link));
                }
            }
        });

        let handlers = vec![
            post.connect_vote_notify(glib::clone!(
                #[weak(rename_to = h)]
                self,
                move |p| h.update(p)
            )),
            post.connect_score_notify(glib::clone!(
                #[weak(rename_to = h)]
                self,
                move |p| h.update(p)
            )),
            post.connect_saved_notify(glib::clone!(
                #[weak(rename_to = h)]
                self,
                move |p| h.update(p)
            )),
            post.connect_comment_count_notify(glib::clone!(
                #[weak(rename_to = h)]
                self,
                move |p| h.update(p)
            )),
        ];
        imp.handlers.replace(handlers);
        imp.post.replace(Some(post.clone()));
        self.update(post);
    }

    /// Disconnects from the bound post.
    pub fn unbind(&self) {
        let imp = self.imp();
        if let Some(post) = imp.post.take() {
            for h in imp.handlers.take() {
                post.disconnect(h);
            }
        }
    }

    fn update(&self, post: &PostObject) {
        let imp = self.imp();
        imp.updating.set(true);
        imp.up.set_active(post.vote() > 0);
        imp.down.set_active(post.vote() < 0);
        imp.save.set_active(post.saved());
        imp.save.set_tooltip_text(Some(&if post.saved() {
            gettext("Unsave")
        } else {
            gettext("Save")
        }));
        imp.updating.set(false);
        imp.score.set_label(&util::compact(post.score()));
        imp.score
            .update_property(&[gtk::accessible::Property::Label(&util::spoken_points(post.score()))]);
        for (class, on) in [("upvoted", post.vote() > 0), ("downvoted", post.vote() < 0)] {
            if on {
                imp.score.add_css_class(class);
            } else {
                imp.score.remove_css_class(class);
            }
        }
        imp.comments.set_label(&util::compact(post.comment_count() as i64));
        imp.comments
            .update_property(&[gtk::accessible::Property::Label(&util::spoken_comments(
                post.comment_count(),
            ))]);
    }
}

/// A card for a link post: domain and URL, opens in the browser.
fn link_card(domain: &str, url: &str, on_link: &LinkHandler) -> gtk::Widget {
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(domain))
        .subtitle(glib::markup_escape_text(url))
        .subtitle_lines(1)
        .activatable(true)
        .build();
    row.add_prefix(&gtk::Image::from_icon_name("web-browser-symbolic"));
    row.add_suffix(&gtk::Image::from_icon_name("adw-external-link-symbolic"));
    row.set_tooltip_text(Some(&gettext("Opens in your web browser")));
    let (on, url) = (on_link.clone(), url.to_string());
    row.connect_activated(move |_| on(&url));
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    list.append(&row);
    list.upcast()
}
