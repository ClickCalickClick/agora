use std::cell::{OnceCell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use agora_core::Feed;
use gettextrs::gettext;
use gtk::glib;

use crate::stores::{SubredditObject, SubscriptionsStore};

type FeedHandler = Box<dyn Fn(Feed)>;

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/ClickCalickClick/Agora/ui/sidebar.ui")]
    pub struct AgoraSidebar {
        #[template_child]
        pub sidebar: TemplateChild<adw::Sidebar>,
        pub on_feed: OnceCell<FeedHandler>,
        pub subscriptions: RefCell<Option<SubscriptionsStore>>,
        /// Fixed rows in order, matching the first sidebar items.
        pub fixed: RefCell<Vec<Feed>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AgoraSidebar {
        const NAME: &'static str = "AgoraSidebar";
        type Type = super::AgoraSidebar;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AgoraSidebar {}
    impl WidgetImpl for AgoraSidebar {}
    impl BinImpl for AgoraSidebar {}
}

glib::wrapper! {
    /// Feeds, the user's places and subscriptions (SPEC §5.1).
    pub struct AgoraSidebar(ObjectSubclass<imp::AgoraSidebar>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[gtk::template_callbacks]
impl AgoraSidebar {
    #[template_callback]
    fn on_activated(&self, index: u32) {
        if let Some(feed) = self.feed_at(index)
            && let Some(f) = self.imp().on_feed.get()
        {
            f(feed);
        }
    }
}

impl AgoraSidebar {
    /// Fills the sidebar; `on_feed` runs when the user picks a feed.
    pub fn setup(&self, subscriptions: &SubscriptionsStore, on_feed: impl Fn(Feed) + 'static) {
        let imp = self.imp();
        let _ = imp.on_feed.set(Box::new(on_feed));

        let feeds = adw::SidebarSection::new();
        feeds.append(item(&gettext("Home"), "go-home-symbolic"));
        feeds.append(item(&gettext("Popular"), "agora-plaza-symbolic"));
        imp.sidebar.append(feeds);

        let you = adw::SidebarSection::new();
        you.set_title(Some(&gettext("You")));
        you.append(item(&gettext("Saved"), "user-bookmarks-symbolic"));
        imp.sidebar.append(you);
        imp.fixed.replace(vec![Feed::Home, Feed::Popular, Feed::Saved]);

        let subs = adw::SidebarSection::new();
        subs.set_title(Some(&gettext("Subscriptions")));
        subs.bind_model(Some(&subscriptions.list()), |obj| {
            let sub = obj.downcast_ref::<SubredditObject>().unwrap().subreddit();
            let item = adw::SidebarItem::builder().title(sub.name.prefixed()).build();
            if let Some(title) = sub.title.filter(|t| !t.is_empty()) {
                item.set_tooltip(Some(&title));
            }
            item
        });
        imp.sidebar.append(subs);
        imp.subscriptions.replace(Some(subscriptions.clone()));
    }

    fn feed_at(&self, index: u32) -> Option<Feed> {
        let imp = self.imp();
        let fixed = imp.fixed.borrow();
        if let Some(feed) = fixed.get(index as usize) {
            return Some(feed.clone());
        }
        let list = imp.subscriptions.borrow().as_ref()?.list();
        let sub = list
            .item(index - fixed.len() as u32)?
            .downcast::<SubredditObject>()
            .ok()?;
        Some(Feed::Subreddit(sub.subreddit().name))
    }

    /// Highlights the row for `feed`, or nothing if it has no row.
    pub fn select_feed(&self, feed: &Feed) {
        let imp = self.imp();
        let fixed_len = imp.fixed.borrow().len() as u32;
        let index = if let Some(i) = imp.fixed.borrow().iter().position(|f| f == feed) {
            Some(i as u32)
        } else if let (Feed::Subreddit(name), Some(store)) = (feed, imp.subscriptions.borrow().as_ref()) {
            let list = store.list();
            (0..list.n_items())
                .find(|&i| {
                    list.item(i)
                        .and_downcast::<SubredditObject>()
                        .is_some_and(|s| s.subreddit().name.eq_ignore_case(name))
                })
                .map(|i| i + fixed_len)
        } else {
            None
        };
        imp.sidebar.set_selected(index.unwrap_or(gtk::INVALID_LIST_POSITION));
    }
}

fn item(title: &str, icon: &str) -> adw::SidebarItem {
    adw::SidebarItem::builder().title(title).icon_name(icon).build()
}
