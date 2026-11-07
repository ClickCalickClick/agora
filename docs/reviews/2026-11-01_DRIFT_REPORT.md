# Drift report: slice 1 (EGM)

**Date:** 2026-11-01 · **Code:** `main` at `a3a6947` (PR #4 merged) · **Method:** EGM
`egm-drift-report` (SPEC §20.1 rule 6). Every design doc was compared with the code it names.
Only differences are listed. Where the code and the doc agree, nothing is said.

**Design docs checked:** `docs/SPEC.md`, `docs/SPIKE-0.md`, `docs/slices/SLICE-1.md`,
`docs/STORES.md`, `docs/CONTRACTS-CHANGELOG.md`, `crates/agora-reddit-web/STRATEGIES.md`
(only where it constrains slice 1 contracts), and the EGM plan
(`EGM-PLAN.md`, project files). Earlier EGM reports in `docs/reviews/egm-2026-10-31/` and the
slice 1 Fable review (`docs/reviews/slice-1-fable-review.md`) were used to tell new drift from
items already deferred on purpose.

**CI on the merge:** all ten check runs on PR #4 passed (fmt, clippy, tests, Meson data
checks, cargo-deny, gitleaks, cargo-sources, Flatpak build against GNOME 51).

Each item is marked:
- **[spec]**: the code is the better choice; change the doc.
- **[code]**: the doc is right; change the code.
- **[decide]**: Jared decides.
- **[later]**: planned for a later slice; listed so it is not forgotten.

---

## Summary: what slice 2 must fix first

These either break with a real backend or change a frozen contract. Do them before writing
`agora-reddit-web`, because each one is cheaper now than after the extractors exist.

1. **`ThreadRequest` has no subreddit, but every comment endpoint needs one.**
   `post_with_comments` and `more_comments` are `/svc/shreddit/comments/r/{sub}/{id}` and
   `/svc/shreddit/more-comments/r/{sub}/{id}` (STRATEGIES.md). `ThreadRequest` carries only
   `PostId`, and `ThreadStore::open_id(PostId)` (links, permalinks) has no subreddit at all.
   Add `subreddit: SubredditName` to `ThreadRequest` (and to `open_id`), or document that the
   backend resolves it with an extra request. Contract change: needs a changelog entry. **[decide]**
2. **Join can send a subscribe with an empty subreddit id.** `FeedStore::set_feed` shows a
   `Subreddit::from_name` placeholder (`id: ""`, `subscribed: false`) until `subreddit_info`
   returns, and the Join button is live the whole time. Reddit's `UpdateSubredditSubscriptions`
   needs `subredditId`. A subscribed user also sees "Join" for that moment. Make Join
   insensitive until the info has loaded. **[code]**
3. **Only the feed waits for sign-in.** `FeedStore::load_more` checks
   `session.can_request()`, but `SubscriptionsStore::refresh`, `FeedStore::load_subreddit_info`,
   `ThreadStore::reload`, `ThreadStore::load_more` and every `Writes` method do not. With a real
   backend, `AgoraWindow::setup` fires `whoami` and `subscriptions` back to back at start-up, and
   a failed subscriptions load stays in `Error` with no retry when the session signs in. Gate
   every store on `can_request()` and refresh subscriptions on `SignedIn`. **[code]**
4. **The window calls the backend directly.** `AgoraWindow::toggle_subscribe`
   (`ui/window.rs`) calls `ctx.backend.subscribe(...)`. `STORES.md` and SPEC §3.4 say views never
   call the backend. Move it into `Writes` (with the one-write-in-flight guard the other writes
   have) or `SubscriptionsStore`. **[code]**
5. **Session `Unknown` can last forever.** `SessionStore::check` logs and keeps `Unknown` when
   `whoami` fails with `Network`, `Parse` or `ExtractorBroken`. The feed then never loads and
   nothing tells the user. `Blocked` also has no timer to leave the state after `retry_after`.
   Show an error with Retry for a failed check, and leave `Blocked` when the backoff ends. **[code]**
6. **Contract freeze is not recorded.** PR #4 merged with "Contracts frozen: Jared signs off"
   unchecked, and `CONTRACTS-CHANGELOG.md` still reads "pending (slice 1 PR)". Either record the
   sign-off (with today's date) or say the freeze waits for items 1 and 7. **[decide]**
7. **Fold the known contract gaps in before the freeze, not after.** Each is a changelog entry
   later: `NewPostContent` has no image/video upload (§9), `PostKind` media payloads are too thin
   for §7.3/§7.4/§16 (Fable C4), `Subreddit` has no posting requirements or flairs (§4.5, §9),
   `Comment` has no `permalink` (§8.2 Copy Link), there is no `user_comments` operation for the
   profile Comments tab (§5.3), and `subscribe` takes a whole `Subreddit` where Reddit needs only
   the id. **[decide]**

---

## docs/SPEC.md

### §3 Platform, stack, layout

- §3.1 names `AdwCssClassBinding` for read and vote styling and `GtkEnumList` for sort
  dropdowns. Neither is used: `AgoraPostRow::update` adds and removes CSS classes by hand, and
  sort is a `GtkMenuButton` with a menu (`feed_page.blp`, `post_page.blp`). The menu is a fine
  GNOME pattern; update §3.1 and §5.1 ("sort dropdown") to say menu. **[spec]**
- §3.2 says UI definitions are Blueprint composite templates. Five widgets are
  (`window`, `sidebar`, `feed_page`, `post_page`, `post_row`). `CommentRow`, `PostHeader`,
  `SubredditHeader`, the Markdown renderer, the list footers and the link card are built in Rust
  with `gtk::*::builder()`, and the shortcuts dialog is loaded with `gtk::Builder::from_resource`
  rather than as a template. Building data-shaped rows in code is reasonable. Suggest §3.2 says
  "Blueprint for windows, pages and fixed widgets; rows whose structure depends on content are
  built in code". Moving `PostHeader` and `SubredditHeader` to Blueprint is optional. **[spec]**
- §3.2 lists `ashpd`, `zbus`, `rusqlite`, `glycin`, `soup3`, `webkit6`. None is a dependency yet,
  which matches their slices. **[later]**
- §3.3 shows `agora-reddit-web` as a workspace crate and `agora-app/src/services/`. The crate
  directory holds only `STRATEGIES.md` and fixtures and is not a workspace member, and there is
  no `services/` module. Expected for slice 1. **[later]**
- §3.3 shows `data/resources/ui/<module>/*.blp` and `style/<module>.css`. The code has a flat
  `ui/*.blp` and one `style/style.css`. **[spec]**
- §3.4 diagram lists Inbox, Search, Profile, Subreddit, Drafts, Filters and Prefs stores.
  `STORES.md` has no `SubredditStore` or `PrefsStore` (subreddit info lives in `FeedStore`,
  prefs are read straight from `gio::Settings`). Update the diagram to the STORES.md list. **[spec]**

### §4 Backend

- §4.3 lists events `SessionExpired`, `Blocked`, `SignedIn`. The code adds `SignedOut` and
  documents `events()` as single-listener (each event goes to one receiver). Add both to §4.3,
  since slice 5's notifications store must not call `events()` a second time. **[spec]**
- §4.3/§4.5 operation table: `more_comments` is "(post id, child ids)"; the trait is
  `more_comments(post, sort: CommentSort, token: MoreToken)`. `sign_out` is in the trait but not
  in the table. Update the table. **[spec]**
- §4.5 says `subreddit_info` "includes posting requirements". `Subreddit` has no such field
  (see summary item 7). **[decide]**
- §4.7 lists eight `AgoraError` variants. The code adds `Cancelled` (dropped interactive
  request, never shown) and `ChallengeRequired { fallback_url }` (posting rule 3). Add both to
  §4.7. **[spec]**
- §4.7 "Network state comes from `gio::NetworkMonitor`... banner says You're offline". Not
  implemented, and §5.4 lists offline as one of every list's states. No slice owns it in §20.
  Assign it to slice 2. **[decide]**
- `Comment.depth` is documented in `model.rs` as absolute, while `STORES.md` says rows are
  indented relative to the shallowest comment. The spec does not define it. The real parser
  will read `depth` from `<shreddit-comment>` and `startingDepth` from more-comments, so state
  in §4.5 that depth is absolute and the store rebases it. **[spec]**

### §5 Layout and navigation

- §5.1 narrow layout is "single pane, navigation stack: Sidebar → List → Post". The code keeps
  the sidebar as an `AdwOverlaySplitView` overlay at every width below 1100 sp, and only the
  list and post form a stack. `SLICE-1.md` already describes the overlay. The overlay is the
  common GNOME pattern for this layout; update §5.1. **[spec]**
- §5.1 sidebar sections: You should hold Saved, Inbox, Drafts and Profile. Only Saved exists;
  the others belong to slices 4 and 5. **[later]**
- §5.1 "Back navigation: `AdwNavigationView` provides the back button, Alt+←, mouse back
  button, edge swipe". This works only when the split view is collapsed. In the wide layout, and
  when leaving a focused branch, only `h` and `win.back` go back. `win.back` has no accelerator,
  yet the shortcuts dialog lists `h <Alt>Left` for Back. Bind Alt+← and the mouse back button to
  `win.back`. **[code]**
- §5.2 main menu: Preferences and the account popover are missing (slices 2 and 5). **[later]**
- §5.3 About: "links to repo, CoC, translators". No translator credits are set
  (`translator_credits`). Add `gettext("translator-credits")`. §18.2 asks for
  `AdwAboutDialog::from_appdata` with release notes; the code builds the dialog by hand. **[code]**
- §5.4 signed-out and offline status pages do not exist yet. Signed-out belongs to slice 2;
  offline see §4.7 above. **[later]**

### §7 Reading

- §7.1 context menu (right-click and long-press: vote, save, hide, share, copy link, open in
  browser, open subreddit, view author, mute, report) is not implemented, and no slice lists
  it. Slice 4 is the natural owner. **[decide]**
- §7.1 pull to refresh on touch is not implemented; no slice lists it. **[decide]**
- §7.1 thumbnails and large cards: not implemented (slice 3). `show-thumbnails` and
  `large-cards` are read nowhere. **[later]**
- §7.2 overflow menu should hold Hide, Report, Edit and Delete; it has Copy Link and Open in
  Browser (later actions arrive in slice 4). Send by Email (§12.3) is missing. **[later]**
- §7.6 says links "activate through the URL router (§12)". `AgoraWindow::open_link` handles
  only `agora:r/` and `agora:u/`; every reddit.com URL opens in the browser. Expected until slice
  5. `agora:u/` opens the user's post feed, not a profile page. **[later]**

### §8 Comments

- §8.1 "Clicking a thread line or the comment header collapses the branch". Matches. Collapse
  fades only on expand; collapse is instant (Fable E2, accepted). **[spec]**
- §8.1 "Collapsed state shows author · N replies" and §16 "announce collapsed state": the
  accessible label is set once at bind and not refreshed on collapse (Fable B6, deferred). Still
  open; the Orca walkthrough has not been done. **[code]**
- §16 "Level N reply by": `comment_row::accessible_text` uses the absolute `Comment.depth`, not
  the row's rebased depth, so a focused branch announces "Level 9 reply" for a comment shown at
  level 0. Use `row.depth()`. **[code]**
- §8.2 Reply, Share/Copy Link, Edit and Delete for comments are missing (slice 4), and Copy Link
  needs `Comment.permalink` (summary item 7). **[later]**

### §11 Keyboard

- Missing keys: `r`, `c`, Ctrl+F, Ctrl+K, Ctrl+L, Ctrl+, (their features arrive in slices 4
  and 5), and Alt+← in the wide layout (see §5.1). **[later]** / **[code]**
- The single-letter guard checks only `gtk::Text` and `gtk::TextView`; `gtk::Editable` would
  also cover `EditableLabel` and spin buttons (Fable B5). **[code]**

### §13 Preferences

- All 25 gschema keys exist with the specified defaults. Read so far: `default-sort`,
  `default-top-range`, `default-comment-sort`, `default-feed`, `mark-read-on-open`,
  `content-zoom`, `show-scores`, `favorite-subreddits`, window size keys. The rest have no
  reader yet, which matches their slices. **[later]**
- `FeedStore::set_sort` and `ThreadStore::set_sort` do not write the default back. The spec does
  not say whether a sort change is remembered. **[decide]**

### §16 Accessibility

- "A unit test per custom widget asserts that its accessible label is non-empty." There are no
  GTK widget tests; CI runs `cargo test` without a display (`ci.yml`: "no display"). §19 also
  says GTK-dependent tests run under `xvfb-run`, which CI does not do. Add `xvfb-run` to the Arch
  job and one test per row widget. **[code]**
- `TREE_ITEM` semantics on comment rows: not set (Fable E4, noted). **[later]**
- Keyboard-only and Orca walkthroughs (§16, slice 1 exit): not done before merge. **[decide]**

### §17 Internationalization

- "Dates via GLib (`GDateTime` formatting)". Ages are computed in `util::short_age` and
  `util::long_age` from Unix seconds with `ngettext`; only `now()` uses GLib. The short forms
  (`{}m`, `{}h`) are translatable strings without translator comments explaining them. Add
  translator comments; GLib formatting is not needed for relative ages. **[spec]**

### §18 Disclaimers and HIG

- §18.1 items 1 to 4 and 8 hold: README, About (`add_legal_section`) and metainfo carry the
  disclaimer text; `LICENSE` exists. The Welcome page is slice 2. **[later]**
- §18.3 "CI checks that `pkg-config --modversion webkitgtk-6.0` succeeds **in the SDK**". CI
  checks it in the Arch container instead. The Flatpak build would catch a missing
  `webkitgtk-6.0` only once `agora-reddit-web` links it in slice 2. Add the check to the Flatpak
  job, or reword §18.3. **[code]**

### §19 Testing and CI

- CI moved the Rust and data checks to `archlinux:latest`; §19 already says so. Two things the
  spec does not cover: (a) Arch is rolling, so GTK or libadwaita can move past the GNOME 51
  runtime and code can start using an API the runtime lacks (the `v4_24` / `v1_10` feature
  pins limit this for gtk-rs bindings only); the Flatpak job remains the real gate. (b) The
  `cargo-sources` job downloads `flatpak-cargo-generator.py` from the `master` branch, unpinned,
  on every run. Pin it to a commit. **[code]**
- "Bridge integration" tests and "pacing scheduler" tests are slice 2. **[later]**
- `AGORA_MOCK=1` is documented as the switch for mock mode, but slice 1 always runs on sample
  data (`application.rs` ignores the variable). `SLICE-1.md` says so. Slice 2 must make the
  variable real. **[later]**

### §20 Build plan

- §20 slice 1 exit criteria "reading works by keyboard and with Orca" and "contracts frozen"
  were not met when PR #4 merged (unchecked in the PR). See summary item 6. **[decide]**
- No slice owns: the offline banner (§4.7), the post-row context menu (§7.1), pull to refresh
  (§7.1), Send by Email (§12.3), RTL testing (§17; slice 6 says "RTL pass", fine). Add the first
  four to slices 2, 4, 4 and 5. **[decide]**

---

## docs/SPIKE-0.md

- §6 change 1 ("parsers descend into `<template>`") is for `agora-reddit-web`; `agora-markdown`
  drops `<template>` content on purpose (Fable D). No drift.
- §6 change 2 (quiet home page via `UserContentFilter` or periodic reload) is slice 2 work and
  is in SPEC §4.2. **[later]**
- §7 "Not covered": Home, Popular, user and saved listings, a non-empty subscriptions list and
  structured profile fields are still "not tried" in STRATEGIES.md. `Feed::Home`, `Popular`,
  `User` and `Saved` are in the frozen `Feed` enum, so slice 2 must try them before the
  extractors are written. **[later]**
- `comment` sends Reddit rich-text JSON (`mode=richText`, `content`), but `Backend::comment`
  and `edit` take Markdown. The backend will need a Markdown → rich-text converter, or Reddit's
  `markdown` mode if it still exists. Neither the spike nor the spec says which. Find out in
  slice 4's first live check. **[decide]**

## docs/slices/SLICE-1.md

- Exit checklist items still open: keyboard walkthrough, Orca walkthrough, contract sign-off.
  "CI green on GitHub" is now true (PR #4) but still unchecked. **[spec]**
- "Narrow the window to phone width: the sidebar becomes an overlay" matches the code but not
  SPEC §5.1 (see above). **[spec]**

## docs/STORES.md

- "Views never call the backend": broken by `AgoraWindow::toggle_subscribe` (summary item 4).
  **[code]**
- `win.back` is listed with no accelerator; SPEC §11 and the shortcuts dialog give it Alt+←.
  **[code]**
- `win.toggle-subscribe(s)` takes the subreddit name, but the handler ignores the parameter and
  uses the feed's current subreddit. Either drop the parameter or use it. **[code]**
- "Errors become... a toast with Retry for row-level actions". `FeedStore::load_subreddit_info`
  and `SubscriptionsStore::refresh` only log failures; the user sees nothing. **[code]**
- The `Writes` row lists vote and save only. Subscribe is described as "through
  `win.toggle-subscribe`" but has no in-flight guard, so a double press can revert the wrong
  state (the race Fable B1 fixed for votes). **[code]**
- `ThreadStore` property `post` is documented; `focused` and `sort-key` match. No other drift.

## docs/CONTRACTS-CHANGELOG.md

- Sign-off column for the freeze is "pending (slice 1 PR)" after the PR merged (summary
  item 6). **[decide]**

## crates/agora-reddit-web/STRATEGIES.md

- `subscribe` needs `subredditId`; `post_with_comments` and `more_comments` need the subreddit
  name. The frozen trait does not guarantee either (summary items 1 and 2). **[decide]**

## EGM-PLAN.md (project files)

- Step 3 (split SPEC.md into `docs/designs/` with a SITEMAP) has not been done; SPEC.md is now
  810 lines. It was optional. With slices 2 to 6 each needing only a few sections, a split
  before slice 2 would let each slice lead load less. **[decide]**
- Step 5 asks for a drift report "after each wave"; this is the first. No drift otherwise.
