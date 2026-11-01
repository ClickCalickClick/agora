use std::cell::RefCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib;

use crate::stores::SubredditObject;
use crate::util;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct SubredditHeader {
        pub avatar: adw::Avatar,
        pub title: gtk::Label,
        pub name: gtk::Label,
        pub description: gtk::Label,
        pub join: gtk::Button,
        pub sub: RefCell<Option<SubredditObject>>,
        pub handler: RefCell<Option<glib::SignalHandlerId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SubredditHeader {
        const NAME: &'static str = "AgoraSubredditHeader";
        type Type = super::SubredditHeader;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for SubredditHeader {
        fn constructed(&self) {
            self.parent_constructed();
            let card = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(10)
                .margin_top(6)
                .margin_bottom(6)
                .css_classes(["card", "community-header"])
                .build();
            let top = gtk::Box::builder().spacing(12).build();
            self.avatar.set_size(48);
            self.avatar.set_show_initials(true);
            top.append(&self.avatar);
            let names = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .valign(gtk::Align::Center)
                .hexpand(true)
                .build();
            self.title.set_xalign(0.0);
            self.title.set_wrap(true);
            self.title.add_css_class("title-3");
            self.name.set_xalign(0.0);
            self.name.set_wrap(true);
            self.name.add_css_class("dim-label");
            self.name.add_css_class("caption");
            names.append(&self.title);
            names.append(&self.name);
            top.append(&names);
            self.join.set_valign(gtk::Align::Center);
            self.join.add_css_class("pill");
            self.join.set_action_name(Some("win.toggle-subscribe"));
            top.append(&self.join);
            card.append(&top);
            self.description.set_xalign(0.0);
            self.description.set_wrap(true);
            card.append(&self.description);
            self.obj().set_child(Some(&card));
        }
    }

    impl WidgetImpl for SubredditHeader {}
    impl BinImpl for SubredditHeader {}
}

glib::wrapper! {
    /// The compact community header above a subreddit's posts (SPEC §2.1).
    pub struct SubredditHeader(ObjectSubclass<imp::SubredditHeader>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for SubredditHeader {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl SubredditHeader {
    /// Shows `obj`.
    pub fn bind(&self, obj: &SubredditObject) {
        let imp = self.imp();
        if let (Some(old), Some(h)) = (imp.sub.take(), imp.handler.take()) {
            old.disconnect(h);
        }
        let sub = obj.subreddit();
        let title = sub
            .title
            .clone()
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| sub.name.to_string());
        imp.avatar.set_text(Some(&title));
        imp.title.set_label(&title);
        let mut line = vec![sub.name.prefixed()];
        if let Some(m) = sub.members {
            line.push(util::members(m));
        }
        imp.name.set_label(&line.join(" · "));
        imp.description.set_label(sub.description.as_deref().unwrap_or(""));
        imp.description
            .set_visible(sub.description.as_ref().is_some_and(|d| !d.is_empty()));
        imp.join.set_action_target_value(Some(&sub.name.as_str().to_variant()));
        let handler = obj.connect_subscribed_notify(glib::clone!(
            #[weak(rename_to = header)]
            self,
            move |o| header.update_join(o)
        ));
        imp.handler.replace(Some(handler));
        imp.sub.replace(Some(obj.clone()));
        self.update_join(obj);
    }

    fn update_join(&self, obj: &SubredditObject) {
        let join = &self.imp().join;
        if obj.subscribed() {
            join.set_label(&gettext("Joined"));
            join.remove_css_class("suggested-action");
        } else {
            join.set_label(&gettext("Join"));
            join.add_css_class("suggested-action");
        }
    }
}
