# Slice 1 review (Fable, independent)

**Reviewed:** commit `843dc12` ("Slice 1: reading on sample data") against `a65c8eb`, plus the
spec-only `b202b7f`. Ground truth: `docs/SPEC.md` (§3, §4.3, §4.5, §5, §7, §8, §11, §13, §16,
§19, §20), `docs/SPIKE-0.md`.
**Checks run:** `cargo build -p agora-app`, `cargo test --workspace` (72 tests, all pass),
`cargo clippy --workspace --all-targets -- -D warnings` (clean), `cargo fmt --check` (clean),
`build-aux/gen-potfiles.py --check` (clean). Headless screenshots at 1280×800 and 360×720 in
mock mode (feed, post `t3_showcase7`, Markdown showcase `t3_showcase1`, `listing=network`,
`post_with_comments=network`, `listing=expired`). Two probe tests were run on a scratch copy of
the workspace (not committed) to confirm findings 2 and 3 below.

**Working tree note:** the tree I reviewed has an uncommitted change to
`crates/agora-app/src/ui/window.rs` (the `focus_is_button` guard for Space) and three
untracked files the exit criteria depend on: `docs/STORES.md`,
`docs/CONTRACTS-CHANGELOG.md`, `docs/slices/SLICE-1.md`. None of these are in `843dc12`.
They need to be committed before the PR.

Overall: the slice is solid. The crates are cleanly separated, the stores have generation
guards for every async reply, every factory unbind disconnects what bind connected on the
row objects, `agora-markdown` is defensive (scheme allow-list, depth caps, every text escaped
in the renderer), and the scaffold matches §19. The items below are what I could verify.

---

## A. Must fix before the PR

### A1. Thread lines are spread across the row (visible in every thread)

`crates/agora-app/src/ui/comment_row.rs:217-222`

```rust
let line = gtk::Box::builder().width_request(2).halign(gtk::Align::Center).hexpand(true) ...
```

`hexpand` propagates upward in GTK 4: the 2 px line makes its 12 px slot expand, the slots
make the `lines` box expand, and the `lines` box then competes with `content` (also
`hexpand`) for the row's spare width. Result (screenshot
`post-1280.png`, crop below the post header): each row's indentation depends on how long its
text is, the lines sit tens of pixels apart instead of 12 px, and the first top-level comment
appears indented deeper than its replies. At 360 px it happens to look right only because
there is no spare width.

Fix: drop `hexpand(true)` from the line (the `halign: Center` already centers it in the
12 px slot), and set `self.lines.set_hexpand(false)` explicitly in `constructed` so the box
never inherits expansion from a child. Add `self.lines.set_valign(Fill)` if the lines should
run the row's full height (they do today only because the Box is vertical-fill by default).

### A2. "Continue this thread" shows a branch that points at itself

`crates/agora-app/src/stores/thread.rs:58-82` (`flatten`) and `:175-220` (`reload`).

`flatten` caps on the comment's **absolute** `depth`, but a focused load
(`ThreadRequest.focus = Some(id)`) returns the branch rooted at a comment that is already at
depth 7 or more. Probe test output on `t3_showcase7` after focusing the first Continue row:

```
kind=Comment  depth=7 id=t1_showcase7c364
kind=Continue depth=8 cont_parent=t1_showcase7c364
```

The root comment is shown indented seven levels, its replies (depth 8) are replaced by a
Continue row whose parent is the root itself, and pressing it reloads the same two rows
forever. The existing test `focus_shows_one_branch` passes because it only checks the first
row's id.

Fix: in `reload`, when `focus` is set, take `base = first comment's depth` and flatten with
`depth - base` (pass `base` into `flatten`, apply it to the cap, the row depth, and the
`More`/`Continue` depths). Better still, settle it in the contract now (see C3): define
`Comment.depth` as relative to the response root so the real parser normalises and the
store needs no special case. Extend `focus_shows_one_branch` to assert `rows[0].depth() == 0`
and that no Continue row's parent equals the focus id.

### A3. MockBackend `more_comments` ignores the sort, so "load more" duplicates comments

`crates/agora-core/src/mock.rs:306-331` (`cut` writes the sibling index `i` of the **sorted**
clone into the `MoreToken`) vs `:448-467` (`more_comments` slices the **unsorted** stored
tree at that index).

Probe test (load the thread, then load every More row, count comment ids already shown):

```
sort Best: 1 more rows, 92 duplicate comments
sort New:  1 more rows, 29 duplicate comments
sort Old:  1 more rows, 98 duplicate comments
```

Even the default sort duplicates, because the sample trees are not stored in Best order. In
the app this shows as the same comments appearing twice after "N more replies". The test
`more_row_is_replaced_by_comments` only asserts the row count grew.

Fix: sort the siblings in `more_comments` the same way `post_with_comments` does, which means
`more_comments` needs the sort. The simplest honest shape is to add it to the operation:
`fn more_comments(&self, post: PostId, sort: CommentSort, token: MoreToken)`. Reddit's own
more-comments partial takes a `sort` parameter too, so the real backend will want it
(see C2). Then make the mock test assert no duplicate ids.

### A4. A thread that fails to load by id shows "No Post Selected" and no Retry

`crates/agora-app/src/ui/post_page.rs:265-270`

```rust
imp.stack.set_visible_child_name(if has_post || store.state() == LoadState::Loading { "content" } else { "empty" });
```

With `open_id` (links, `AGORA_DEV_OPEN`, slice 5's URL router) there is no `PostObject`
until the reply arrives, so on `LoadState::Error` the page flips to the empty placeholder and
the error footer with its Retry button is never visible (screenshot `fail-thread-1280.png`,
`AGORA_MOCK_FAIL=post_with_comments=network AGORA_DEV_OPEN=t3_showcase7`). Opening from a feed
row is fine because the row object is shown at once.

Fix: show `"content"` whenever `store.state() != LoadState::Idle` (the comments footer already
renders Loading, Error+Retry and Empty), or add a third stack page for the error.

---

## B. Should fix before the PR (small, contained)

### B1. Optimistic vote revert races with a second vote on the same object

`crates/agora-app/src/stores/writes.rs:22-39` (`vote_post`), `:43-60` (`vote_comment`).

Each write captures `old`/`old_score` at press time and restores them absolutely on failure.
Two quick presses on one item (mock latency is 350 ms, so this is easy to hit):

- Up then Down, first request fails, second succeeds: UI reverts to None/S while Reddit holds
  Down.
- Up then Up (to clear), both fail: the second revert restores the first press's optimistic
  Up/S+1, so the UI shows an upvote that was never accepted.

Fix: keep a per-object write sequence (`Cell<u64>` on `PostObject`/`CommentObject`); a failed
write reverts only if it is still the latest write for that object, otherwise it just toasts.
The Retry closure is fine as written (it re-runs the toggle from the reverted state).

### B2. Voting or saving a comment destroys the button that was just pressed

`crates/agora-app/src/ui/comment_row.rs:153-163` rebuilds the whole row content (including
re-parsing and re-rendering the Markdown) on every `vote`, `score` and `saved` notify, which
fires twice per vote. The `ToggleButton` the user clicked (or pressed Space on) is removed
inside its own `clicked` handler, so keyboard focus is lost after every vote, which is the
§16 "nothing is reachable by hover only" promise failing for the most common action.

Fix: keep the action-bar widgets in the `imp` struct like `PostHeader` does and update them in
place with an `updating` guard; rebuild content only on `collapsed`/`loading`.

### B3. Footer and focus-note widgets are recreated and re-connected on every bind

`crates/agora-app/src/ui/feed_page.rs:258-262` + `:440`,
`crates/agora-app/src/ui/post_page.rs:203-212` + `:460`.

Each bind calls `footer_widget`/`comments_footer`/`focus_note_widget`, which creates a new
widget tree and does `footer.connect_state_notify(update)` without ever disconnecting. The
`FooterObject` lives as long as the page, so every scroll past the end adds a handler (they
hold weak refs, so nothing crashes; they just accumulate and run against dead stacks). Create
the three widgets once in `setup` (OnceCell) and hand the same widget to `set_child` in bind,
or disconnect in `unbind` the way the row widgets do.

### B4. `CommentRow::unbind` never clears the branch highlight

`crates/agora-app/src/ui/comment_row.rs:186-192`: `imp.row.take()` runs first, then
`self.highlight(false)` reads `imp.row` and finds `None`, so the call is dead. A hovered row
that is recycled leaves its whole branch lit until the pointer enters another comment. Call
`thread.highlight_branch(&row, false)` directly with the row you just took.

### B5. Space is swallowed on focused widgets that are not `gtk::Button`

`crates/agora-app/src/ui/window.rs:322` and `:332-334` (uncommitted). `focus_is_button`
checks `Button`/`ToggleButton` (the latter is redundant; it is a `Button`), but
`gtk::MenuButton` is not a Button subclass: with a comment selected, Space on the focused
Sort or More menu button in the post header collapses the comment instead of opening the
menu. Same for the link card's `AdwActionRow`, and for any future `CheckButton`/`Switch`.
Invert the test: handle Space only when the focus widget is the comment `ListView` itself
(`focus.is::<gtk::ListView>()` or `focus == list_view`), and let everything else through.
While there, `w.is::<gtk::Text>() || w.is::<gtk::TextView>()` could be
`w.is::<gtk::Editable>() || w.is::<gtk::TextView>()` so it also covers `EditableLabel`.

### B6. Collapsed state is never announced

`crates/agora-app/src/ui/post_page.rs:191` sets the list item's accessible label once at
bind. `toggle_collapsed` does not rebind the row (it only splices rows after it), so Orca
never hears "collapsed, N replies"; the comment at `post_page.rs:341` ("Rebinding replaced
the label") describes something that does not happen. In `CommentRow::bind`, connect
`collapsed`/`count` notify and update the parent `ListItem`'s label (store a weak ref to the
item, or do it in the factory and disconnect in unbind).

### B7. `Backend::events()` hands out competing receivers

`crates/agora-core/src/backend.rs:77-79`: "Every call returns a receiver of the same channel".
`async_channel` is MPMC: each event is delivered to **one** receiver. Today only
`SessionStore` subscribes, but the doc invites a second listener (slice 5's inbox store, a
Blocked countdown) that would silently steal `SessionExpired` from the session store. Decide
before freezing: either document single-consumer (`fn events(&self) -> Option<Receiver>` that
returns `Some` once), or make it a broadcast (`async_broadcast`, or the backend fans out to
each subscriber).

---

## C. Contract notes for the end-of-slice freeze (`agora-core`)

Things slice 2–4 will otherwise have to change under the sign-off rule.

- **C1. Session surface.** §4.4 needs sign-out (clear `NetworkSession` data, drafts,
  subscriptions) and the login dialog. The trait has no `sign_out`. Add
  `fn sign_out(&self) -> BackendFuture<()>` now. The login `WebView` can be built by
  `agora-reddit-web` directly (the app depends on it), so no GTK type leaks into core.
- **C2. `more_comments` needs `CommentSort`** (A3). The partial Reddit's front end calls for
  "more replies" carries the thread sort; without it the backend cannot reproduce the order
  it was cut from.
- **C3. Define `Comment.depth`** as "relative to the first node of the response" (A2), so a
  focused/permalink load and a `more_comments` load both start where the UI expects.
- **C4. Media models are too thin for slice 3** (§7.3, §7.4, §16): `PostKind::Video { url }`
  has one URL but the spec needs the HLS playlist, the MP4 fallback, and whether it is a
  short silent GIF (no audio, ≤ 30 s); `Gallery { urls }` has no captions (alt text is
  required); `Image { url }` has no size or alt. Suggest struct payloads now:
  `Video { hls: Option<String>, fallback: String, duration_secs: Option<u32>, has_audio: Option<bool> }`,
  `Gallery { items: Vec<GalleryItem { url, caption: Option<String> }> }`,
  `Image { url, width: Option<u32>, height: Option<u32>, alt: Option<String> }`.
- **C5. `Subreddit` lacks posting requirements and flairs.** §4.5 says `subreddit_info`
  "includes posting requirements"; `NewPost.flair_id` has nothing to choose from. Add
  `flairs: Vec<Flair>` and a `PostRequirements` struct (title length, allowed kinds,
  flair required), even if the mock fills defaults.
- **C6. Smaller model gaps:** `Comment` has no `permalink` (§8.2 Copy Link/Share), no author
  flair, no `distinguished` (mod/admin) marker, no `locked`; `Post` has no author flair,
  `distinguished`, or `archived` (no voting/commenting); `Post.score` is `i64` while
  `Comment.score` is `Option<i64>` although Reddit hides new post scores too.
  `Subreddit.id: String`, `Notification.id: String` and `mark_read(Vec<String>)` break the
  name-type pattern; `SubredditId`/`NotificationId` are cheap now.
- **C7. Operation table gap (spec, not code):** the profile screen's Comments tab (§5.3) has
  no operation (`listing` only returns posts). Either add `user_comments` or drop the tab from
  §5.3 before the table is frozen.
- **C8. Slice 2 sequencing trap:** `FeedStore::load_more` returns early when
  `!session.can_request() && !ctx.mock` (`feed.rs:132`). `AgoraWindow::setup` calls
  `show_feed` before `session.check()` has resolved, so with a real backend the first feed
  will never load until the user refreshes. The feed store should re-run `load_more` when the
  session becomes `SignedIn`. Not a slice 1 bug, but worth a TODO so it is not rediscovered.

Checked and fine: `AgoraError` matches §4.7 plus `Cancelled` and `ChallengeRequired`
(posting rule 3); `Cursor`/`MoreToken` are opaque strings, which is right for partial URLs;
`Body::Html`/`Body::Markdown` as in §4.5; the mock's `FailureScript` covers every operation
name; `ThreadRequest.focus` is the right shape for permalinks.

---

## D. `agora-markdown` and the renderer (safety)

No findings that need action.

- URLs: `inline.rs:16-46` `safe_url` allows only `http`, `https`, `mailto` (links only),
  rejects control characters and relative paths, resolves `/r/x` and `//host` against
  `https://www.reddit.com`. `javascript:`, `data:`, `file:` and `agora:` are rejected, so
  content cannot forge the in-app `agora:r/…` / `agora:u/…` links; those only come from the
  mention pass. Tests cover `javascript:` in both parsers.
- Pango markup: every text segment in `render/mod.rs:197-260` goes through
  `glib::markup_escape_text`, including link `href`s, image alt, mention names and the
  spoiler title. Code blocks and the post title use `.label()` without markup. `PostHeader`
  meta (`post_header.rs:359-376`) and the link card (`:504-506`) escape too.
- Recursion: `MAX_DEPTH = 32` in both parsers (`markdown.rs:260`, `inline.rs:225,277`),
  `MAX_DOM_DEPTH = 128` for html5ever subtrees read as text beyond it (`html.rs:15`),
  and the renderer flattens below `MAX_NESTING = 12` (`render/mod.rs:18`). `script`,
  `style`, `template`, `svg` and form controls are dropped entirely.
- One behaviour to keep in mind for slice 2: `html_to_ast` drops `<template>` content, which
  is correct for a body. §4.5's "parsers descend into template content" is about the comment
  tree partial, which belongs to the `agora-reddit-web` parser, not this crate.

---

## E. Spec drift, accessibility, HIG, scaffold

Can be follow-ups unless noted.

- **E1 (§8.1) "Continue thread → pushes a new page rooted at that comment."** Implemented as
  an in-place reload with a "Showing one branch" bar. Workable, but the NavigationView back
  button, Alt+←, the mouse back button and the edge swipe do not leave the branch; only `h`
  and `win.back` do. Either push an `AdwNavigationPage` inside the post pane or update §8.1
  and §11 to say so.
- **E2 (§8.1) collapse animation.** Only expand fades rows in; collapse is instant. Minor.
- **E3 (§16) per-widget accessible-label tests** are absent ("a unit test per custom widget
  asserts that its accessible label is non-empty"). CI already runs tests under `xvfb-run`,
  so a `gtk::init` test for `AgoraPostRow`, `CommentRow`, `PostHeader` and `SubredditHeader`
  is feasible.
- **E4 (§16) TREE_ITEM semantics** on comment rows are not set; "Level N reply by" is. Fine
  for now, note it in the Orca walkthrough.
- **E5 (§7.1) `single-click-activate: true`** (`feed_page.blp:73`) also selects on hover, so
  a pointer resting over the list moves the selection that `j`/`k` and `a`/`z`/`s` act on.
  Standard GTK trade-off for a sidebar-style list; mention it in the keyboard walkthrough so
  it is not reported as a bug.
- **E6 Infinite scroll** (`feed_page.rs:184-193`) only fires on `value-changed` and only when
  `upper > page_size`. A viewport taller than the first page (25 rows × ~64 px) will never
  request page 2. Also listen to `upper` notify, or check once after each page arrives.
- **E7 `collapse_level` on More/Continue rows** (`comment_row.rs:259-279`) walks up to the
  nearest **comment** at that depth, which for a More row at its own depth is the previous
  sibling, not the parent whose line it is. Off by one level for non-comment rows.
- **E8 `bind_sidebar_button`** (`feed_page.rs:205`) connects two handlers to the split view on
  every `realize`; harmless today (realized once), leaks if the page is ever reparented.
- **E9 Spec wording vs scaffold.** §3.3/§19 describe `config.rs.in` and
  `build-aux/gen-resources.py` driven by Meson; the code compiles Blueprint, GResource and the
  dev schema in `crates/agora-app/build.rs` and passes configuration through `AGORA_*`
  environment variables, with `gen-potfiles.py` for POTFILES. This is a cleaner setup
  (plain `cargo run` works) and `meson.build` documents it; update the two spec paragraphs
  at the end of the slice rather than the code.
- **E10 CI.** The list in §19 is complete (fmt, clippy, tests under xvfb via `meson test`,
  cargo-deny, gitleaks, Flatpak Devel build, appstreamcli/desktop-file-validate/
  glib-compile-schemas). Two things to watch on the first green run: (a) `xvfb-run -a
  build-aux/sdk-run.sh meson test` relies on `flatpak run --socket=x11` seeing the Xvfb
  socket and xauth cookie from inside the container; (b) the Flatpak job generates
  `build-aux/cargo-sources.json` on the fly and the README tells a fresh clone to do the same,
  which matches the slice 6 exit criterion only if that step stays documented. Nothing wrong
  in the YAML itself; `permissions: contents: read` and `upload-artifact: false` are right.
- **E11 metainfo** `<release date="2026-11-01">` is one day ahead of the review date; if
  `appstreamcli validate` warns about a future release date, move it or drop the `<releases>`
  block until the first tag.
- **E12 HIG/screenshots.** Wide, medium and 360 px layouts, the empty, loading, error and
  expired states, badges, sort subtitle, Markdown (lists, nested quotes, code, table,
  superscript, spoiler, mentions) all render as the spec describes. Two visual checks after
  A1 lands: the Copy Link button in the post action bar renders dimmer than Save in every
  screenshot (confirm it is not insensitive because the `post.*` action group cannot be
  resolved from inside the list item), and the comment action bar at 55% opacity reads as
  disabled at a glance; consider 0.7.

---

## F. What I checked and did not find

- RefCell double borrows: every `borrow()` of store/row state is cloned out before a call
  that could re-enter (`toggle_collapsed`, `highlight_branch`, `build_content`, `reload`,
  `load_more`). None found.
- Stale async results: `FeedStore` and `ThreadStore` bump `generation` on every reset and
  compare after `await`; `load_subreddit_info` compares the `SubredditObject` identity;
  `SessionStore`, `SubscriptionsStore` and `Writes` use weak upgrades. Correct.
- Factory recycling: `AgoraPostRow`, `PostHeader`, `CommentRow`, `SubredditHeader` all
  disconnect their object handlers in `unbind` (or on the next `bind`) and the factories call
  `unbind`. Only the footer/focus-note widgets are not covered (B3).
- `toggle_collapsed`/stash logic: nested collapsed branches survive an outer collapse and
  expand; counts include nested hidden counts and More counts; hidden rows get their
  highlight cleared; a More row stashed under a collapsed parent that finishes loading is
  dropped safely because `rows.find` returns `None`. Correct.
- Keyboard: Ctrl/Alt combinations and text entries pass through; `j`/`k` skip header,
  footer and focus-note rows; Space on a Button passes through (B5 covers the rest).
- `close_request` clamps saved window size to ≥ 360 and the schema ranges match §13; all
  25 keys from §13 exist with the specified defaults.
- `cargo deny`, `gitleaks` config, `rustfmt.toml`, `edition = 2024`, `rust-version = 1.92`
  present.

---

## Resolution (slice 1 lead, 2026-11-01)

Fixed before the PR:

- **A1** thread lines: the line no longer expands; the lines box is fixed width.
- **A2** focused branches: rows are indented relative to the shallowest comment received
  (`base_depth`), so a focused root sits at level 0 and never continues itself. Test added.
- **A3** `more_comments` now takes the `CommentSort`; the mock sorts before slicing. Test
  `load_more_never_repeats_comments` covers every sort.
- **A4** the post pane shows content (with Retry) whenever the thread is not idle.
- **B1** one write per item and kind in flight; a second press while pending is ignored.
- **B2** votes and saves update the comment action bar in place; only collapse and loading
  rebuild the row.
- **B3** footer handlers are disconnected on unbind (`attach_watch`/`release_watch`).
- **B4** highlight is cleared before the row is taken.
- **B5** Space belongs to buttons, menu buttons, list box rows and anything outside the post
  pane.
- **B7** `Backend::events()` documents the single-listener contract.
- **C** `Backend::sign_out` added; first feed load waits for the session to be signed in
  (slice 2 trap); a short first page counts as "at the end".
- **E** spec updated for build.rs resources (§19, §3.3) and the in-place "Continue this
  thread" (§8.1); metainfo release date set to 2026-10-31.
- Found while fixing: `feed.*` and `post.*` were inserted action groups that the template's
  buttons never resolved (Refresh and Copy Link were insensitive). They are now widget class
  actions (`install_action`, `install_property_action`).

Deferred (recorded for later slices): B6 (row accessible label refresh on collapse; Orca
walkthrough will confirm), richer media `PostKind` payloads (slice 3), posting requirements
and flair source (slice 4), `Comment` permalink/distinguished/locked, typed ids for
subreddits and notifications, a `user_comments` operation for the profile Comments tab,
per-widget accessible-label tests, `single-click-activate` hover selection.
