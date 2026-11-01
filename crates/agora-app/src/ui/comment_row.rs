use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use agora_core::{CommentState, VoteState};
use gettextrs::gettext;
use gtk::glib;

use crate::i18n::{i18n_f, ni18n_f};
use crate::render::{self, LinkHandler};
use crate::stores::{CommentObject, CommentRowKind, ThreadStore, Writes};
use crate::util;

/// Width of one indentation level, in pixels.
const LEVEL_WIDTH: i32 = 12;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct CommentRow {
        pub lines: gtk::Box,
        pub content: gtk::Box,
        pub row: RefCell<Option<CommentObject>>,
        pub handlers: RefCell<Vec<glib::SignalHandlerId>>,
        pub thread: RefCell<Option<ThreadStore>>,
        /// Up, score, down and save controls of the current action bar.
        pub actions: RefCell<Option<(gtk::ToggleButton, gtk::Label, gtk::ToggleButton, gtk::ToggleButton)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CommentRow {
        const NAME: &'static str = "AgoraCommentRow";
        type Type = super::CommentRow;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for CommentRow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            let root = gtk::Box::builder().margin_start(6).margin_end(6).build();
            self.lines.add_css_class("thread-lines");
            self.lines.set_hexpand(false);
            root.append(&self.lines);
            self.content.set_orientation(gtk::Orientation::Vertical);
            self.content.set_spacing(4);
            self.content.set_hexpand(true);
            self.content.set_margin_top(8);
            self.content.set_margin_bottom(8);
            root.append(&self.content);
            obj.set_child(Some(&root));

            // Hovering a comment highlights its whole branch (SPEC §2.1, §8.1).
            let motion = gtk::EventControllerMotion::new();
            motion.connect_enter(glib::clone!(
                #[weak]
                obj,
                move |_, _, _| obj.highlight(true)
            ));
            motion.connect_leave(glib::clone!(
                #[weak]
                obj,
                move |_| obj.highlight(false)
            ));
            obj.add_controller(motion);
        }
    }

    impl WidgetImpl for CommentRow {}
    impl BinImpl for CommentRow {}
}

glib::wrapper! {
    /// One row of the flattened comment tree: a comment, "N more replies" or
    /// "Continue this thread" (SPEC §8).
    pub struct CommentRow(ObjectSubclass<imp::CommentRow>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for CommentRow {
    fn default() -> Self {
        glib::Object::new()
    }
}

/// Spoken description of a row, for the list item (SPEC §16).
pub fn accessible_text(row: &CommentObject) -> String {
    match row.kind() {
        CommentRowKind::More => ni18n_f(
            "{} more reply",
            "{} more replies",
            row.count(),
            &[&row.count().to_string()],
        ),
        CommentRowKind::Continue => gettext("Continue this thread"),
        CommentRowKind::Comment => row
            .with(|c| {
                let author = c
                    .author
                    .as_ref()
                    .map_or_else(|| gettext("deleted account"), |a| a.prefixed());
                let mut parts = vec![if c.depth == 0 {
                    i18n_f("Comment by {}", &[&author])
                } else {
                    i18n_f("Level {} reply by {}", &[&(c.depth + 1).to_string(), &author])
                }];
                if !row.score_hidden() {
                    parts.push(util::spoken_points(row.score()));
                }
                parts.push(util::long_age(c.created));
                match row.vote() {
                    1 => parts.push(gettext("upvoted")),
                    -1 => parts.push(gettext("downvoted")),
                    _ => {}
                }
                if row.collapsed() {
                    parts.push(ni18n_f(
                        "collapsed, {} reply",
                        "collapsed, {} replies",
                        row.count(),
                        &[&row.count().to_string()],
                    ));
                }
                parts.join(", ")
            })
            .unwrap_or_default(),
    }
}

impl CommentRow {
    /// Shows `row`.
    pub fn bind(
        &self,
        row: &CommentObject,
        thread: &ThreadStore,
        writes: &Writes,
        on_link: &LinkHandler,
        op: Option<&str>,
        me: &str,
    ) {
        self.unbind();
        let imp = self.imp();
        imp.thread.replace(Some(thread.clone()));
        imp.row.replace(Some(row.clone()));
        self.build_lines(row);
        self.build_content(row, writes, on_link, op, me);

        let mut handlers = vec![row.connect_highlight_line_notify(glib::clone!(
            #[weak(rename_to = this)]
            self,
            move |r| this.update_lines(r)
        ))];
        for prop in ["vote", "score", "saved"] {
            handlers.push(row.connect_notify_local(
                Some(prop),
                glib::clone!(
                    #[weak(rename_to = this)]
                    self,
                    move |r, _| this.update_actions(r)
                ),
            ));
        }
        for prop in ["collapsed", "loading"] {
            let (writes, on_link, op, me) = (writes.clone(), on_link.clone(), op.map(str::to_owned), me.to_owned());
            handlers.push(row.connect_notify_local(
                Some(prop),
                glib::clone!(
                    #[weak(rename_to = this)]
                    self,
                    move |r, _| this.build_content(r, &writes, &on_link, op.as_deref(), &me)
                ),
            ));
        }
        imp.handlers.replace(handlers);

        if row.take_fade_in() {
            // Fade in rows that appear after expanding or loading (SPEC §8.1).
            self.set_opacity(0.0);
            let target = adw::PropertyAnimationTarget::new(self, "opacity");
            adw::TimedAnimation::builder()
                .widget(self)
                .value_from(0.0)
                .value_to(1.0)
                .duration(200)
                .target(&target)
                .build()
                .play();
        } else {
            self.set_opacity(1.0);
        }
    }

    /// Disconnects from the bound row.
    pub fn unbind(&self) {
        let imp = self.imp();
        let lit = imp.row.borrow().as_ref().is_some_and(|r| r.highlight_line() >= 0);
        if lit {
            self.highlight(false);
        }
        if let Some(row) = imp.row.take() {
            for h in imp.handlers.take() {
                row.disconnect(h);
            }
        }
    }

    fn highlight(&self, on: bool) {
        let imp = self.imp();
        let (Some(row), Some(thread)) = (imp.row.borrow().clone(), imp.thread.borrow().clone()) else {
            return;
        };
        if row.kind() == CommentRowKind::Comment {
            thread.highlight_branch(&row, on);
        }
    }

    /// One line per level, including the row's own (SPEC §8.1).
    fn build_lines(&self, row: &CommentObject) {
        let lines = &self.imp().lines;
        while let Some(child) = lines.first_child() {
            lines.remove(&child);
        }
        for level in 0..=row.depth() {
            let slot = gtk::Box::builder()
                .width_request(LEVEL_WIDTH)
                .css_classes(["thread-slot"])
                .build();
            let line = gtk::Box::builder()
                .width_request(2)
                .halign(gtk::Align::Center)
                .css_classes(["thread-line"])
                .build();
            slot.append(&line);
            // Clicking a line collapses the comment that line belongs to.
            let click = gtk::GestureClick::new();
            click.connect_released(glib::clone!(
                #[weak(rename_to = this)]
                self,
                move |g, _, _, _| {
                    g.set_state(gtk::EventSequenceState::Claimed);
                    this.collapse_level(level as i32);
                }
            ));
            slot.add_controller(click);
            slot.set_cursor_from_name(Some("pointer"));
            lines.append(&slot);
        }
        self.update_lines(row);
    }

    fn update_lines(&self, row: &CommentObject) {
        let mut child = self.imp().lines.first_child();
        let mut level = 0;
        while let Some(slot) = child {
            if let Some(line) = slot.first_child() {
                if row.highlight_line() == level {
                    line.add_css_class("highlight");
                } else {
                    line.remove_css_class("highlight");
                }
            }
            level += 1;
            child = slot.next_sibling();
        }
    }

    /// Collapses the comment whose branch the line at `level` belongs to: the
    /// row itself at its own level, otherwise the ancestor at that depth.
    fn collapse_level(&self, level: i32) {
        let imp = self.imp();
        let (Some(row), Some(thread)) = (imp.row.borrow().clone(), imp.thread.borrow().clone()) else {
            return;
        };
        let rows = thread.rows();
        let Some(mut i) = rows.find(&row) else { return };
        loop {
            let Some(r) = rows.item(i).and_downcast::<CommentObject>() else {
                return;
            };
            if r.depth() as i32 == level && r.kind() == CommentRowKind::Comment {
                thread.toggle_collapsed(&r);
                return;
            }
            if i == 0 {
                return;
            }
            i -= 1;
        }
    }

    fn build_content(&self, row: &CommentObject, writes: &Writes, on_link: &LinkHandler, op: Option<&str>, me: &str) {
        let content = &self.imp().content;
        self.imp().actions.replace(None);
        while let Some(child) = content.first_child() {
            content.remove(&child);
        }
        match row.kind() {
            CommentRowKind::More => {
                let label = ni18n_f(
                    "{} more reply",
                    "{} more replies",
                    row.count().max(1),
                    &[&row.count().max(1).to_string()],
                );
                let label = if row.count() == 0 {
                    gettext("More replies")
                } else {
                    label
                };
                let button = gtk::Button::builder()
                    .halign(gtk::Align::Start)
                    .css_classes(["flat", "more-button"])
                    .build();
                if row.loading() {
                    button.set_child(Some(&adw::Spinner::new()));
                } else {
                    button.set_child(Some(&gtk::Label::builder().label(&label).xalign(0.0).build()));
                }
                let (thread, r) = (self.imp().thread.borrow().clone(), row.clone());
                button.connect_clicked(move |_| {
                    if let Some(t) = &thread {
                        t.load_more(&r);
                    }
                });
                content.append(&button);
            }
            CommentRowKind::Continue => {
                let button = gtk::Button::builder()
                    .label(gettext("Continue This Thread →"))
                    .halign(gtk::Align::Start)
                    .css_classes(["flat", "more-button"])
                    .build();
                let (thread, r) = (self.imp().thread.borrow().clone(), row.clone());
                button.connect_clicked(move |_| {
                    if let (Some(t), Some(id)) = (&thread, r.continue_parent()) {
                        t.focus_comment(id);
                    }
                });
                content.append(&button);
            }
            CommentRowKind::Comment => {
                let Some(c) = row.data() else { return };
                // Header: author, badges, score, age. Clicking it collapses.
                let header = gtk::Box::builder().spacing(6).build();
                let author = gtk::Label::builder()
                    .label(c.author.as_ref().map_or_else(|| gettext("[deleted]"), |a| a.prefixed()))
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .css_classes(["caption-heading"])
                    .build();
                header.append(&author);
                let name = c.author.as_ref().map(|a| a.as_str());
                if c.is_submitter || (name.is_some() && name == op) {
                    header.append(
                        &gtk::Label::builder()
                            .label(gettext("OP"))
                            .css_classes(["badge", "caption", "accent"])
                            .build(),
                    );
                }
                if name.is_some() && name == Some(me) {
                    header.append(
                        &gtk::Label::builder()
                            .label(gettext("You"))
                            .css_classes(["badge", "caption"])
                            .build(),
                    );
                }
                let mut meta = vec![util::short_age(c.created)];
                if c.edited {
                    meta.push(gettext("edited"));
                }
                if row.collapsed() {
                    meta.push(ni18n_f(
                        "{} reply",
                        "{} replies",
                        row.count(),
                        &[&row.count().to_string()],
                    ));
                }
                header.append(
                    &gtk::Label::builder()
                        .label(meta.join(" · "))
                        .css_classes(["caption", "dim-label"])
                        .build(),
                );
                let click = gtk::GestureClick::new();
                let (thread, r) = (self.imp().thread.borrow().clone(), row.clone());
                click.connect_released(move |_, _, _, _| {
                    if let Some(t) = &thread {
                        t.toggle_collapsed(&r);
                    }
                });
                header.add_controller(click);
                content.append(&header);
                if row.collapsed() {
                    content.add_css_class("collapsed");
                    return;
                }
                content.remove_css_class("collapsed");

                match c.state {
                    CommentState::Deleted => content.append(&placeholder(&gettext("[deleted]"))),
                    CommentState::Removed => content.append(&placeholder(&gettext("[removed]"))),
                    CommentState::Normal => {
                        let doc = render::parse_body(&c.body);
                        content.append(&render::render(&doc, on_link));
                    }
                }
                content.append(&self.actions(row, writes));
            }
        }
    }

    fn actions(&self, row: &CommentObject, writes: &Writes) -> gtk::Widget {
        let bar = gtk::Box::builder().spacing(2).css_classes(["comment-actions"]).build();
        let make = |icon: &str, class: &str| {
            gtk::ToggleButton::builder()
                .icon_name(icon)
                .css_classes(["flat", "circular", class])
                .build()
        };
        let up = make("agora-upvote-symbolic", "upvote");
        up.set_tooltip_text(Some(&gettext("Upvote")));
        let down = make("agora-downvote-symbolic", "downvote");
        down.set_tooltip_text(Some(&gettext("Downvote")));
        let score = gtk::Label::builder()
            .css_classes(["caption", "numeric", "score"])
            .build();
        let save = make("bookmark-new-symbolic", "save");
        let w: Rc<Writes> = Rc::new(writes.clone());
        for (button, action) in [(&up, "up"), (&down, "down"), (&save, "save")] {
            let (w, r) = (w.clone(), row.clone());
            button.connect_clicked(move |_| match action {
                "up" => w.vote_comment(&r, VoteState::Up),
                "down" => w.vote_comment(&r, VoteState::Down),
                _ => w.toggle_save_comment(&r),
            });
        }
        bar.append(&up);
        bar.append(&score);
        bar.append(&down);
        bar.append(&save);
        self.imp().actions.replace(Some((up, score, down, save)));
        self.update_actions(row);
        bar.upcast()
    }

    /// Updates the action bar in place, so a pressed button keeps focus.
    fn update_actions(&self, row: &CommentObject) {
        let Some((up, score, down, save)) = self.imp().actions.borrow().clone() else {
            return;
        };
        up.set_active(row.vote() > 0);
        down.set_active(row.vote() < 0);
        save.set_active(row.saved());
        save.set_tooltip_text(Some(&if row.saved() {
            gettext("Unsave")
        } else {
            gettext("Save")
        }));
        score.set_label(&if row.score_hidden() {
            "•".to_string()
        } else {
            util::compact(row.score())
        });
        score.remove_css_class("upvoted");
        score.remove_css_class("downvoted");
        if row.vote() > 0 {
            score.add_css_class("upvoted");
        } else if row.vote() < 0 {
            score.add_css_class("downvoted");
        }
        score.update_property(&[gtk::accessible::Property::Label(&if row.score_hidden() {
            gettext("Score hidden")
        } else {
            util::spoken_points(row.score())
        })]);
    }
}

fn placeholder(text: &str) -> gtk::Widget {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build()
        .upcast()
}
