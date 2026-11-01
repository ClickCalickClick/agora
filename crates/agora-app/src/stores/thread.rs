use std::cell::{Cell, RefCell};
use std::rc::Rc;

use agora_core::{AgoraError, CommentId, CommentNode, CommentSort, PostId, ThreadRequest};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use super::context::error_message;
use super::{CommentObject, CommentRowKind, Context, LoadState, PostObject};

/// Deepest visual level; deeper replies show "Continue this thread" (SPEC §8.1).
pub const MAX_VISUAL_DEPTH: u32 = 8;

mod imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::ThreadStore)]
    pub struct ThreadStore {
        pub ctx: RefCell<Option<Rc<Context>>>,
        pub post_id: RefCell<Option<PostId>>,
        pub focus: RefCell<Option<CommentId>>,
        pub sort: Cell<CommentSort>,
        pub generation: Cell<u64>,
        /// Absolute depth shown at level 0 (non-zero for a focused branch).
        pub base_depth: Cell<u32>,
        pub rows: RefCell<Option<gio::ListStore>>,
        /// The post, once loaded (or the row it was opened from).
        #[property(get, nullable)]
        pub post: RefCell<Option<PostObject>>,
        #[property(get, builder(LoadState::Idle))]
        pub state: Cell<LoadState>,
        #[property(get)]
        pub error: RefCell<String>,
        /// Showing one branch ("Continue this thread") instead of all comments.
        #[property(get)]
        pub focused: Cell<bool>,
        /// Comment sort key.
        #[property(get)]
        pub sort_key: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ThreadStore {
        const NAME: &'static str = "AgoraThreadStore";
        type Type = super::ThreadStore;
    }

    #[glib::derived_properties]
    impl ObjectImpl for ThreadStore {}
}

glib::wrapper! {
    /// One post and its comment tree, flattened into rows (SPEC §8.1).
    pub struct ThreadStore(ObjectSubclass<imp::ThreadStore>);
}

/// Depth of the shallowest node, which the UI shows at level 0. A focused
/// load ("Continue this thread") starts at the focused comment's depth.
pub fn base_depth(nodes: &[CommentNode]) -> u32 {
    nodes
        .iter()
        .map(|n| match n {
            CommentNode::Comment { comment, .. } => comment.depth,
            CommentNode::More { depth, .. } | CommentNode::Continue { depth, .. } => *depth,
        })
        .min()
        .unwrap_or(0)
}

/// Flattens comment nodes into rows indented relative to `base`, applying the
/// visual depth limit.
pub fn flatten(nodes: Vec<CommentNode>, base: u32, out: &mut Vec<CommentObject>) {
    for node in nodes {
        match node {
            CommentNode::Comment { comment, replies } => {
                let depth = comment.depth.saturating_sub(base);
                if depth >= MAX_VISUAL_DEPTH {
                    if let Some(parent) = comment.parent.clone() {
                        out.push(CommentObject::continue_thread(MAX_VISUAL_DEPTH, parent));
                    }
                    // One "continue" row stands for all siblings at this level.
                    return;
                }
                let row = CommentObject::comment(comment);
                row.set_depth(depth);
                out.push(row);
                flatten(replies, base, out);
            }
            CommentNode::More {
                depth, count, token, ..
            } => {
                out.push(CommentObject::more(
                    depth.saturating_sub(base).min(MAX_VISUAL_DEPTH),
                    count,
                    token,
                ));
            }
            CommentNode::Continue { parent, depth } => {
                out.push(CommentObject::continue_thread(
                    depth.saturating_sub(base).min(MAX_VISUAL_DEPTH),
                    parent,
                ));
            }
        }
    }
}

impl ThreadStore {
    /// Creates an empty store.
    pub fn new(ctx: Rc<Context>) -> Self {
        let obj: Self = glib::Object::new();
        let imp = obj.imp();
        let sort = CommentSort::from_key(&ctx.settings.string("default-comment-sort")).unwrap_or_default();
        imp.sort.set(sort);
        imp.sort_key.replace(sort.key().into());
        imp.rows.replace(Some(gio::ListStore::new::<CommentObject>()));
        imp.ctx.replace(Some(ctx));
        obj
    }

    /// Visible rows, as [`CommentObject`]s.
    pub fn rows(&self) -> gio::ListStore {
        self.imp().rows.borrow().clone().unwrap()
    }

    fn ctx(&self) -> Rc<Context> {
        self.imp().ctx.borrow().clone().unwrap()
    }

    /// Opens a post from a feed row; the row object is shown at once and
    /// updated when the full post arrives.
    pub fn open(&self, post: &PostObject) {
        let imp = self.imp();
        imp.post_id.replace(Some(post.with(|p| p.id.clone())));
        imp.post.replace(Some(post.clone()));
        self.notify_post();
        self.set_focus(None);
        self.reload();
    }

    /// Opens a post by id (links).
    pub fn open_id(&self, id: PostId) {
        let imp = self.imp();
        imp.post_id.replace(Some(id));
        imp.post.replace(None);
        self.notify_post();
        self.set_focus(None);
        self.reload();
    }

    /// Clears the store.
    pub fn close(&self) {
        let imp = self.imp();
        imp.generation.set(imp.generation.get() + 1);
        imp.post_id.replace(None);
        imp.post.replace(None);
        self.notify_post();
        self.rows().remove_all();
        self.set_state(LoadState::Idle);
    }

    /// Changes the comment sort and reloads.
    pub fn set_sort(&self, sort: CommentSort) {
        let imp = self.imp();
        if imp.sort.get() == sort {
            return;
        }
        imp.sort.set(sort);
        imp.sort_key.replace(sort.key().into());
        self.notify_sort_key();
        self.reload();
    }

    /// Shows only the branch under `comment` ("Continue this thread").
    pub fn focus_comment(&self, comment: CommentId) {
        self.set_focus(Some(comment));
        self.reload();
    }

    /// Back from a focused branch to all comments.
    pub fn unfocus(&self) {
        if self.focused() {
            self.set_focus(None);
            self.reload();
        }
    }

    fn set_focus(&self, focus: Option<CommentId>) {
        let imp = self.imp();
        let focused = focus.is_some();
        imp.focus.replace(focus);
        if imp.focused.get() != focused {
            imp.focused.set(focused);
            self.notify_focused();
        }
    }

    /// Loads the post and its comments again.
    pub fn reload(&self) {
        let imp = self.imp();
        let Some(post) = imp.post_id.borrow().clone() else {
            return;
        };
        imp.generation.set(imp.generation.get() + 1);
        let generation = imp.generation.get();
        self.rows().remove_all();
        self.set_state(LoadState::Loading);
        let request = ThreadRequest {
            post,
            sort: imp.sort.get(),
            focus: imp.focus.borrow().clone(),
        };
        let backend = self.ctx().backend.clone();
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            let result = backend.post_with_comments(request).await;
            let Some(obj) = weak.upgrade() else { return };
            if obj.imp().generation.get() != generation {
                return;
            }
            match result {
                Ok((post, nodes)) => {
                    let current = obj.post();
                    match current {
                        Some(row) => row.update(post),
                        None => {
                            obj.imp().post.replace(Some(PostObject::new(post)));
                            obj.notify_post();
                        }
                    }
                    let base = base_depth(&nodes);
                    obj.imp().base_depth.set(base);
                    let mut rows = Vec::new();
                    flatten(nodes, base, &mut rows);
                    obj.rows().extend_from_slice(&rows);
                    obj.set_state(LoadState::Done);
                }
                Err(AgoraError::Cancelled) => obj.set_state(LoadState::Idle),
                Err(e) => {
                    obj.imp().error.replace(error_message(&e));
                    obj.notify_error();
                    obj.set_state(LoadState::Error);
                }
            }
        });
    }

    /// Collapses or expands a comment's branch.
    pub fn toggle_collapsed(&self, row: &CommentObject) {
        if row.kind() != CommentRowKind::Comment {
            return;
        }
        let rows = self.rows();
        let Some(index) = rows.find(row) else { return };
        if row.collapsed() {
            let stash = std::mem::take(&mut *row.stash().borrow_mut());
            for r in &stash {
                r.mark_fade_in();
            }
            row.set_collapsed(false);
            row.set_count(0);
            rows.splice(index + 1, 0, &stash);
        } else {
            let depth = row.depth();
            let mut end = index + 1;
            while let Some(r) = rows.item(end).and_downcast::<CommentObject>() {
                if r.depth() <= depth {
                    break;
                }
                end += 1;
            }
            let hidden: Vec<CommentObject> = (index + 1..end)
                .filter_map(|i| rows.item(i).and_downcast::<CommentObject>())
                .collect();
            let count: u32 = hidden
                .iter()
                .map(|r| match r.kind() {
                    CommentRowKind::Comment => 1 + r.count(),
                    CommentRowKind::More => r.count(),
                    CommentRowKind::Continue => 0,
                })
                .sum();
            for r in &hidden {
                r.set_highlight_line(-1);
            }
            row.stash().replace(hidden);
            row.set_count(count);
            row.set_collapsed(true);
            rows.splice(index + 1, end - index - 1, &[] as &[CommentObject]);
        }
    }

    /// Highlights (or clears) the thread line along `row`'s whole branch.
    pub fn highlight_branch(&self, row: &CommentObject, on: bool) {
        let rows = self.rows();
        let Some(index) = rows.find(row) else { return };
        let depth = row.depth();
        let value = if on { depth as i32 } else { -1 };
        row.set_highlight_line(value);
        let mut i = index + 1;
        while let Some(r) = rows.item(i).and_downcast::<CommentObject>() {
            if r.depth() <= depth {
                break;
            }
            r.set_highlight_line(value);
            i += 1;
        }
    }

    /// Loads the comments behind a "more" row and puts them in its place.
    pub fn load_more(&self, row: &CommentObject) {
        if row.kind() != CommentRowKind::More || row.loading() {
            return;
        }
        let (Some(token), Some(post)) = (row.token(), self.imp().post_id.borrow().clone()) else {
            return;
        };
        row.set_loading(true);
        let sort = self.imp().sort.get();
        let backend = self.ctx().backend.clone();
        let generation = self.imp().generation.get();
        let weak = self.downgrade();
        let row = row.clone();
        glib::spawn_future_local(async move {
            let result = backend.more_comments(post, sort, token).await;
            let Some(obj) = weak.upgrade() else { return };
            row.set_loading(false);
            if obj.imp().generation.get() != generation {
                return;
            }
            match result {
                Ok(nodes) => {
                    let rows = obj.rows();
                    let Some(index) = rows.find(&row) else { return };
                    let mut new_rows = Vec::new();
                    flatten(nodes, obj.imp().base_depth.get(), &mut new_rows);
                    for r in &new_rows {
                        r.mark_fade_in();
                    }
                    rows.splice(index, 1, &new_rows);
                }
                Err(e) => {
                    let retry_obj = obj.downgrade();
                    let retry_row = row.clone();
                    obj.ctx().toast_error(
                        &e,
                        Some(Rc::new(move || {
                            if let Some(o) = retry_obj.upgrade() {
                                o.load_more(&retry_row);
                            }
                        })),
                    );
                }
            }
        });
    }

    fn set_state(&self, state: LoadState) {
        let imp = self.imp();
        if state != LoadState::Error && !imp.error.borrow().is_empty() {
            imp.error.replace(String::new());
            self.notify_error();
        }
        if imp.state.get() != state {
            imp.state.set(state);
            self.notify_state();
        }
    }
}

#[cfg(test)]
mod tests {
    use agora_core::MockBackend;

    use super::*;
    use crate::stores::test_util::{context, run_until, with_main_context};

    fn loaded(post: &str) -> ThreadStore {
        let store = ThreadStore::new(context(MockBackend::new()));
        store.open_id(PostId::new(post));
        run_until(|| store.state() == LoadState::Done);
        store
    }

    fn rows(store: &ThreadStore) -> Vec<CommentObject> {
        let r = store.rows();
        (0..r.n_items()).map(|i| r.item(i).and_downcast().unwrap()).collect()
    }

    #[test]
    fn flatten_caps_visual_depth() {
        with_main_context(|| {
            let store = loaded("t3_showcase7");
            let all = rows(&store);
            assert!(all.iter().all(|r| r.depth() <= MAX_VISUAL_DEPTH));
            assert!(all.iter().any(|r| r.kind() == CommentRowKind::Continue));
            assert!(all.iter().any(|r| r.kind() == CommentRowKind::More));
            // Every comment row follows its parent.
            for (i, r) in all.iter().enumerate() {
                if r.kind() == CommentRowKind::Comment && r.depth() > 0 {
                    assert!(
                        all[..i].iter().any(|p| p.depth() == r.depth() - 1),
                        "row {i} has no parent above it"
                    );
                }
            }
        })
    }

    #[test]
    fn collapse_and_expand_restore_rows() {
        with_main_context(|| {
            let store = loaded("t3_showcase7");
            let before: Vec<_> = rows(&store);
            let parent = before
                .iter()
                .enumerate()
                .find(|(i, r)| {
                    r.kind() == CommentRowKind::Comment && before.get(i + 1).is_some_and(|n| n.depth() > r.depth())
                })
                .map(|(_, r)| r.clone())
                .unwrap();
            store.toggle_collapsed(&parent);
            assert!(parent.collapsed());
            assert!(parent.count() > 0);
            assert!(store.rows().n_items() < before.len() as u32);
            store.toggle_collapsed(&parent);
            assert!(!parent.collapsed());
            assert_eq!(rows(&store), before);
        })
    }

    #[test]
    fn highlight_covers_branch_only() {
        with_main_context(|| {
            let store = loaded("t3_showcase7");
            let all = rows(&store);
            let i = all
                .iter()
                .position(|r| r.kind() == CommentRowKind::Comment && r.depth() == 0)
                .unwrap();
            store.highlight_branch(&all[i], true);
            let end = all[i + 1..]
                .iter()
                .position(|r| r.depth() == 0)
                .map_or(all.len(), |p| p + i + 1);
            for (j, r) in all.iter().enumerate() {
                assert_eq!(r.highlight_line() == 0, (i..end).contains(&j), "row {j}");
            }
            store.highlight_branch(&all[i], false);
            assert!(all.iter().all(|r| r.highlight_line() == -1));
        })
    }

    #[test]
    fn more_row_is_replaced_by_comments() {
        with_main_context(|| {
            let store = loaded("t3_showcase7");
            let more = rows(&store)
                .into_iter()
                .find(|r| r.kind() == CommentRowKind::More)
                .unwrap();
            let n = store.rows().n_items();
            store.load_more(&more);
            run_until(|| store.rows().find(&more).is_none());
            assert!(store.rows().n_items() > n);
        })
    }

    #[test]
    fn focus_shows_one_branch() {
        with_main_context(|| {
            let store = loaded("t3_showcase7");
            let cont = rows(&store)
                .into_iter()
                .find(|r| r.kind() == CommentRowKind::Continue)
                .unwrap();
            store.focus_comment(cont.continue_parent().unwrap());
            run_until(|| store.state() == LoadState::Done && store.rows().n_items() > 0);
            assert!(store.focused());
            assert_eq!(rows(&store)[0].depth(), 0, "focused root sits at level 0");
            assert!(
                rows(&store)
                    .iter()
                    .all(|r| r.continue_parent() != cont.continue_parent()),
                "no row continues the root itself"
            );
            let first = rows(&store)[0].clone();
            assert_eq!(first.with(|c| c.id.clone()), cont.continue_parent());
            store.unfocus();
            assert!(!store.focused());
        })
    }
}
