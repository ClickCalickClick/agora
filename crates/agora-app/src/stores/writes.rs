use std::rc::Rc;

use agora_core::{Thing, VoteState};
use gtk::glib;

use super::{CommentObject, Context, PostObject, vote_to_int};

/// Optimistic writes: the object changes at once and reverts, with an error
/// toast, if Reddit's response is an error (SPEC §4.5).
#[derive(Clone)]
pub struct Writes {
    ctx: Rc<Context>,
}

impl Writes {
    /// Creates the helper.
    pub fn new(ctx: Rc<Context>) -> Self {
        Self { ctx }
    }

    /// Votes on a post; voting the current vote again clears it.
    pub fn vote_post(&self, post: &PostObject, vote: VoteState) {
        let old = post.vote_state();
        let new = if old == vote { VoteState::None } else { vote };
        let old_score = post.score();
        post.set_vote(vote_to_int(new));
        post.set_score(old_score - old.delta() + new.delta());
        let id = post.with(|p| p.id.clone());
        let fut = self.ctx.backend.vote(Thing::Post(id), new);
        let (ctx, post, this) = (self.ctx.clone(), post.clone(), self.clone());
        glib::spawn_future_local(async move {
            if let Err(e) = fut.await {
                post.set_vote(vote_to_int(old));
                post.set_score(old_score);
                let retry_post = post.clone();
                ctx.toast_error(&e, Some(Rc::new(move || this.vote_post(&retry_post, vote))));
            }
        });
    }

    /// Votes on a comment; voting the current vote again clears it.
    pub fn vote_comment(&self, row: &CommentObject, vote: VoteState) {
        let Some(id) = row.with(|c| c.id.clone()) else { return };
        let old = row.vote_state();
        let new = if old == vote { VoteState::None } else { vote };
        let old_score = row.score();
        row.set_vote(vote_to_int(new));
        row.set_score(old_score - old.delta() + new.delta());
        let fut = self.ctx.backend.vote(Thing::Comment(id), new);
        let (ctx, row, this) = (self.ctx.clone(), row.clone(), self.clone());
        glib::spawn_future_local(async move {
            if let Err(e) = fut.await {
                row.set_vote(vote_to_int(old));
                row.set_score(old_score);
                let retry_row = row.clone();
                ctx.toast_error(&e, Some(Rc::new(move || this.vote_comment(&retry_row, vote))));
            }
        });
    }

    /// Saves or unsaves a post.
    pub fn toggle_save_post(&self, post: &PostObject) {
        let saved = !post.saved();
        post.set_saved(saved);
        let fut = self.ctx.backend.save(Thing::Post(post.with(|p| p.id.clone())), saved);
        let (ctx, post) = (self.ctx.clone(), post.clone());
        glib::spawn_future_local(async move {
            if let Err(e) = fut.await {
                post.set_saved(!saved);
                ctx.toast_error(&e, None);
            }
        });
    }

    /// Saves or unsaves a comment.
    pub fn toggle_save_comment(&self, row: &CommentObject) {
        let Some(id) = row.with(|c| c.id.clone()) else { return };
        let saved = !row.saved();
        row.set_saved(saved);
        let fut = self.ctx.backend.save(Thing::Comment(id), saved);
        let (ctx, row) = (self.ctx.clone(), row.clone());
        glib::spawn_future_local(async move {
            if let Err(e) = fut.await {
                row.set_saved(!saved);
                ctx.toast_error(&e, None);
            }
        });
    }
}
