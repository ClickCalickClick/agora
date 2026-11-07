use std::cell::RefCell;

use adw::subclass::prelude::*;
use agora_core::Post;
use gettextrs::gettext;
use gtk::glib;
use gtk::prelude::*;

use crate::stores::PostObject;
use crate::util;

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/ClickCalickClick/Agora/ui/post_row.ui")]
    pub struct AgoraPostRow {
        #[template_child]
        pub title: TemplateChild<gtk::Label>,
        #[template_child]
        pub meta: TemplateChild<gtk::Label>,
        #[template_child]
        pub badges: TemplateChild<gtk::Box>,
        #[template_child]
        pub score: TemplateChild<gtk::Label>,
        #[template_child]
        pub comments: TemplateChild<gtk::Label>,
        #[template_child]
        pub trailing: TemplateChild<gtk::Box>,
        pub post: RefCell<Option<PostObject>>,
        pub handlers: RefCell<Vec<glib::SignalHandlerId>>,
        pub item: glib::WeakRef<gtk::ListItem>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AgoraPostRow {
        const NAME: &'static str = "AgoraPostRow";
        type Type = super::AgoraPostRow;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AgoraPostRow {
        fn constructed(&self) {
            self.parent_constructed();
            // Score and comment count are in the row's description; the bare
            // numbers would be read without units.
            self.trailing.update_state(&[gtk::accessible::State::Hidden(true)]);
        }
    }
    impl WidgetImpl for AgoraPostRow {}
    impl BoxImpl for AgoraPostRow {}
}

glib::wrapper! {
    /// A quiet, text-first post row (SPEC §2.1, §7.1).
    pub struct AgoraPostRow(ObjectSubclass<imp::AgoraPostRow>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl Default for AgoraPostRow {
    fn default() -> Self {
        glib::Object::new()
    }
}

/// Badge labels for a post, in display order.
pub fn badges(post: &Post) -> Vec<String> {
    let mut out = Vec::new();
    if post.pinned {
        out.push(gettext("Pinned"));
    }
    if post.nsfw {
        out.push(gettext("NSFW"));
    }
    if post.spoiler {
        out.push(gettext("Spoiler"));
    }
    if post.oc {
        out.push(gettext("OC"));
    }
    if post.locked {
        out.push(gettext("Locked"));
    }
    out
}

/// "r/sub · u/author · 3h · flair" (parts left out as asked).
pub fn meta_line(post: &Post, show_subreddit: bool, show_author: bool) -> String {
    let mut parts = Vec::new();
    if show_subreddit {
        parts.push(post.subreddit.prefixed());
    }
    if show_author {
        parts.push(match &post.author {
            Some(a) => a.prefixed(),
            None => gettext("[deleted]"),
        });
    }
    parts.push(util::short_age(post.created));
    if let Some(flair) = &post.flair {
        parts.push(flair.clone());
    }
    parts.join(" · ")
}

impl AgoraPostRow {
    /// Shows `post`. `show_subreddit` is false inside a subreddit feed.
    pub fn bind(&self, item: &gtk::ListItem, post: &PostObject, show_subreddit: bool, show_scores: bool) {
        self.unbind();
        let imp = self.imp();
        imp.item.set(Some(item));
        post.with(|p| {
            imp.title.set_label(&p.title);
            imp.meta.set_label(&meta_line(p, show_subreddit, true));
            while let Some(child) = imp.badges.first_child() {
                imp.badges.remove(&child);
            }
            let list = badges(p);
            imp.badges.set_visible(!list.is_empty());
            for b in list {
                let label = gtk::Label::builder().label(b).css_classes(["badge", "caption"]).build();
                imp.badges.append(&label);
            }
        });
        imp.score.set_visible(show_scores);
        let handlers = vec![
            post.connect_score_notify(glib::clone!(
                #[weak(rename_to = row)]
                self,
                move |p| row.update(p)
            )),
            post.connect_vote_notify(glib::clone!(
                #[weak(rename_to = row)]
                self,
                move |p| row.update(p)
            )),
            post.connect_read_notify(glib::clone!(
                #[weak(rename_to = row)]
                self,
                move |p| row.update(p)
            )),
            post.connect_saved_notify(glib::clone!(
                #[weak(rename_to = row)]
                self,
                move |p| row.update(p)
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
        imp.score.set_label(&util::compact(post.score()));
        imp.comments.set_label(&util::compact(post.comment_count() as i64));
        for (class, on) in [("upvoted", post.vote() > 0), ("downvoted", post.vote() < 0)] {
            if on {
                imp.score.add_css_class(class);
            } else {
                imp.score.remove_css_class(class);
            }
        }
        if post.read() {
            imp.title.add_css_class("dim-label");
        } else {
            imp.title.remove_css_class("dim-label");
        }
        // One sentence for screen readers (SPEC §16).
        let description = post.with(|p| {
            let mut parts = vec![
                p.subreddit.prefixed(),
                p.author
                    .as_ref()
                    .map_or_else(|| gettext("deleted account"), |a| a.prefixed()),
                util::long_age(p.created),
                util::spoken_points(post.score()),
                util::spoken_comments(post.comment_count()),
            ];
            match post.vote() {
                1 => parts.push(gettext("upvoted")),
                -1 => parts.push(gettext("downvoted")),
                _ => {}
            }
            if post.saved() {
                parts.push(gettext("saved"));
            }
            if post.read() {
                parts.push(gettext("read"));
            }
            parts.extend(badges(p));
            parts.join(", ")
        });
        let label = post.with(|p| p.title.clone());
        // Screen readers focus the list item, so the label and description
        // go there. Setting them on the row widget during bind made GTK 4.24
        // log criticals and drop rows from the accessibility tree.
        if let Some(item) = self.imp().item.upgrade() {
            item.set_accessible_label(&label);
            item.set_accessible_description(&description);
        }
    }
}
