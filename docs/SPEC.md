# Agora: Design Specification

**Status:** Revised after the EGM Goldfish review (2026-10-31): vertical-slice build plan, reduced 1.0 scope
**Owner:** Jared (ClickCalickClick)
**Inspiration:** [Giara](https://gitlab.gnome.org/World/giara) by Gabriele Musco

Agora is a native GNOME client for Reddit, written in Rust with GTK 4 and libadwaita. It is a new project, not a fork: it takes Giara's idea (a Reddit app that belongs on GNOME) and rebuilds it for current GNOME, adaptive layouts, and touch.

**Public source, self-built, not distributed.** Agora reads and writes Reddit through the user's own logged-in browser session in an embedded web engine (§4). The source is public on GitHub with the disclaimers in §18; there are no binaries, store listings or Flathub releases. Each user builds and runs it themselves, on their own account, for personal use.

This document describes what Agora does and how it is built. Decisions are recorded in §21. Review reports live in `docs/reviews/`.

---

## 0. Problem statement

People who use GNOME and read Reddit have two options today, and neither fits the desktop:

- **reddit.com in a browser tab.** It works, but it is a web app inside a browser: it does not follow the GNOME look, adaptive layout, keyboard conventions or accessibility stack, it autoplays media, and it is built around engagement prompts.
- **Third-party native clients.** Giara and similar apps used Reddit's public Data API. That API is closing to apps without a written agreement (sources and dates in §0.1), so a new native client cannot rely on it.

Agora's goal is a calm, native GNOME reader and writer for one person's own Reddit account that keeps working after the API closes, without pretending to be anything other than that person browsing reddit.com.

### 0.1 Facts this design depends on

| Fact | Source | Checked |
|---|---|---|
| Self-service registration of new OAuth apps ended under the Responsible Builder Policy (November 2025) | r/redditdev announcements; reddithelp.com | 2026-10-31 |
| Reddit stops accepting new public API access requests on 2026-10-31; unregistered apps lose access from 2027-01-12; the public Data API closes to everyone without an agreement in March 2027 | r/redditdev announcements; press coverage (TechRepublic, Business Insider) | 2026-10-31 |
| Reddit announced on 2026-10-29 that the site is becoming logged-in-only | Reddit announcement; press coverage | 2026-10-31 |
| r/all was removed in April 2025 | Reddit announcement | 2026-10-31 |

These are recorded with sources in `docs/reviews/SPEC-REVIEW.md` §1. If any of them changes, revisit §4 and the alternatives below.

### 0.2 Alternatives considered

| Option | Cost | Breakage risk | Account risk | Why not chosen |
|---|---|---|---|---|
| Reddit OAuth API with a user-registered client ID | Low | Certain: closes March 2027, and new apps can no longer register | Low | Not available to new apps |
| Plain HTTP (libsoup) to `.json` endpoints with cookies copied out of a browser | Low | High: unauthenticated `.json` is gone, and copied cookies expire | High: a non-browser client presenting browser cookies looks like session hijacking to anti-abuse systems | Copying cookies out of a browser is the kind of circumvention §18 forbids |
| Browser extension or user script that talks to a native app | Medium: two codebases and native messaging | Medium | Low | The user must keep a browser running with Reddit open, and the native app only mirrors a browser tab |
| RSS/Atom feeds | Low | Medium | Low | Read-only, no comments tree, no inbox, no voting or posting |
| A visible embedded browser tab (a "Reddit webapp" wrapper) | Low | Low | Low | Not a native GNOME app; no adaptive layout, keyboard model or accessibility beyond what the site provides |
| **Hidden embedded WebKit session with a JS bridge (chosen)** | High: extractors per operation | High: reddit.com changes without notice (§4.5) | Medium: automated use of the website may violate the User Agreement; kept small by pacing (§4.6) | Chosen: the only option that gives a native UI on top of the user's own real browser session |
| Do not build Agora | None | None | None | Rejected by the owner; the project accepts the risks in §0.3 |

### 0.3 Risks and mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Reddit changes an endpoint or page and an operation breaks | High | One feature stops working | One extractor per operation in one crate (§4.5); `--self-test` reports which operation broke; fixtures make the fix testable |
| Reddit restricts or bans the account | Medium | User loses access | Human-paced traffic with exact limits (§4.6); stop on any captcha, 429 or block; never circumvent; first-run acknowledgement (§6) |
| JavaScript injection through the bridge | Low | Script runs in the logged-in session | Arguments passed as typed values with `call_async_javascript_function`, never interpolated into script text (§4.2) |
| Page scripts forging bridge replies | Low | Wrong data shown | Bridge script and message handler live in the isolated world `agora`; page scripts cannot reach that world's handler (§4.2) |
| Login cookies on disk | Medium | Someone with access to the user's files can use the session | Cookies stay in WebKit's own profile under `$XDG_DATA_HOME/agora/web/`, the same exposure as any browser profile; Agora never copies or logs them |
| Very large threads at a paced request rate | Medium | Slow loading of "more replies" | Load more only when the user asks; interactive lane jumps ahead of background work (§4.6) |
| The hidden view is throttled by WebKit because it is not visible | Unknown | Bridge replies stall | Measured in the spike (§20, slice 0); if it happens, keep the view mapped off-screen or adjust WebKit's page visibility settings |
| Legal exposure for the author | Low | Claims against the author | Disclaimers in README, About and onboarding; no Reddit marks; no credentials; GPL warranty disclaimer (§18) |
| Name clash with "Agora Linux App Store" and Agora.io | Low | Confusion if ever distributed | None needed while Agora is not listed anywhere; revisit if distribution plans change |

---

## 1. Goals and non-goals

### Goals

1. Follow the GNOME HIG: libadwaita widgets, adaptive layout from 360 px to desktop widths, accessible, translatable.
2. Meet the [GNOME Circle](https://circle.gnome.org/) checklist for HIG, accessibility and translatability, even though Agora is not submitted anywhere.
3. Cover everyday Reddit use: browsing, reading, voting, posting, replying, and the inbox.
4. Every action works with mouse and keyboard, touch, and the Orca screen reader (§16).
5. Public source on GitHub, self-built by each user, with the disclaimers in §18.
6. Generate no more traffic than a person browsing reddit.com would, under the exact limits in §4.6.
7. Keep every Reddit-specific detail in the `agora-reddit-web` crate, behind the `Backend` trait (§4.3), so the UI never depends on how Reddit is reached.

### Non-goals

- Moderation tools (mod queue, removals, flair management, mod mail). Out of scope permanently, not just deferred.
- Chat (Reddit's chat is not reachable this way).
- Swipe-to-vote or swipe-to-save gestures. Swipes are for navigation only.
- Support for anything older than the current stable GNOME runtime (§3.1).
- Multiple accounts. A future version would need one WebKit `NetworkSession` (one data directory) per account.
- Reddit's OAuth API, API keys, or client IDs (§0.2).
- Distribution: no Flathub, no app stores, no prebuilt binaries or bundles on GitHub Releases, no GNOME Circle application. Users build from source.
- Other networks (Lemmy, PieFed, …).
- Not in 1.0 (see "Later" in §20): an offline cache, background inbox checking after the window closes, polls, an offline action queue.

---

## 2. Identity and discoverability

| Item | Value |
|---|---|
| Name | Agora |
| App ID | `io.github.ClickCalickClick.Agora` (matches the GitHub repo); development builds use `io.github.ClickCalickClick.Agora.Devel` |
| GenericName | Reddit Client |
| Summary (AppStream) | "Browse and discuss Reddit" |
| Desktop `Keywords` | `Reddit;Subreddit;Forum;Social;News;Discussion;` (translatable) |
| Categories | `Network;News;GTK;GNOME;` |
| License | GPL-3.0-or-later (same family as Giara) |
| Metadata license | CC0-1.0 |
| Code of Conduct | GNOME Code of Conduct (`CODE_OF_CONDUCT.md` links to it) |

**Discoverability:** searching "Reddit" in the Activities overview (and GNOME Software, for a local install) must find Agora. This is done through `GenericName`, `Keywords`, and the AppStream `<keywords>` and `<summary>`. The name "Agora" itself must not contain "Reddit" (trademark).

**Trademark note:** the app must not use the Reddit logo or Snoo in its icon, and AppStream must state that Agora is an unofficial client not affiliated with Reddit, Inc.

### 2.1 Visual identity

Agora is its own app built entirely from GNOME parts. The rule: **everything structural comes from libadwaita; everything that makes it "Agora" is a small number of signature touches.** No custom theming, no custom window chrome, no fixed brand color in the UI.

**Relationship to Giara.** Giara is inspiration for the *idea* only. Agora is written from scratch in Rust, and reuses none of Giara's code, UI files, CSS, icons, artwork, or screenshots. Where a design choice would make Agora look like Giara, pick the alternative. Agora differs in:
- **Layout:** three panes (sidebar, list, post) instead of a feed-plus-detail flow, with phone layouts built on `AdwNavigationView`.
- **Feed style:** text-first rows with a small trailing thumbnail by default (large cards are opt-in), rather than an image-led feed.
- **Icon and name:** a new name and an original icon concept (below), with no reference to Giara's icon shape or palette.

**Design intent.** An agora was the open square where people gathered to talk. The identity leans on that: a quiet place for conversation rather than a feed that competes for attention. The concrete rules below carry this out.

**App icon (designed in slice 6):**
- A small open plaza seen from the GNOME icon perspective: a row of simple columns (a portico) whose negative space forms a speech bubble.
- Palette: a warm stone/sand base with one muted teal accent. Not Reddit orange-red, and no Snoo, alien, or Reddit mark.
- Follows the [GNOME app icon guidelines](https://developer.gnome.org/hig/guidelines/app-icons.html) (made in App Icon Preview, tested at 16–128 px and on light and dark backgrounds), plus a matching symbolic icon.
- AppStream `<branding>` uses the sand (light) and deep teal (dark) colors. The in-app UI uses the **system accent color**.

**Signature touches (all built from stock libadwaita widgets and style classes):**
1. **Conversation lines.** The comment thread lines (§8.1): 2 px neutral lines that turn the accent color along the whole branch on hover or keyboard focus, with an animated collapse.
2. **Quiet rows.** Post rows are typographic: `.heading` title, one `.dim-label` metadata line, score and comment count as small trailing figures. Read posts use `.dim-label` on the title.
3. **Community headers.** Each subreddit page opens with a compact header built from its icon, name, and description in a rounded `.card`, with Join as a `.pill` button. Subreddit banner images are shown at most 96 px tall with reduced opacity.
4. **Calm by default.** No autoplay, no counters in the sidebar except Inbox, no red notification dots. Motion uses libadwaita's standard springs and respects reduced animation.
5. **Illustrated empty states.** Custom `AdwStatusPage` icons in the same style as the app icon (an empty plaza for "No posts", a closed gate for "Offline", a mailbox for "Inbox zero").

**Writing voice.** Short, plain UI text ("Nothing new in your inbox", not "0 unread messages found"). Follows the GNOME HIG writing style (header capitalization for buttons and titles, sentence case for descriptions).

**What stays strictly stock:** header bars, sidebars, split views, dialogs, preferences, toasts, banners, buttons, typography, and color, so that Agora follows future GNOME visual changes without edits.

---

## 3. Platform and stack

### 3.1 Platform target

- **Target: the current stable GNOME runtime.** As of this spec that is **GNOME 51** (`org.gnome.Platform//51`), **GTK 4.24**, **libadwaita 1.10**, **WebKitGTK 6.0**. This line is the only place the versions are stated; everything else says "the target runtime".
- No compatibility shims for older versions. Bump to each new GNOME runtime within one cycle.
- Newer libadwaita/GTK features the spec relies on: `AdwSidebar` with sections (§5.1), `AdwCssClassBinding` (read-state and vote-state styling), `GtkEnumList` for sort dropdowns, and GTK's built-in reduced-motion handling in Spinner, Stack and Revealer.

### 3.2 Language and core crates

| Concern | Crate | Notes |
|---|---|---|
| UI | `gtk4`, `libadwaita` (gtk-rs) | Feature flags pinned to the target runtime (`gtk4/v4_24`, `libadwaita/v1_10`) |
| UI definitions | Blueprint (`.blp`) compiled to `.ui` | Composite templates via `#[derive(CompositeTemplate)]` |
| GObject / async | `glib`, `gio` | All async work runs on the GLib main context; there is no second async runtime |
| Reddit traffic | `webkit6` (WebKitGTK 6.0) | Hidden logged-in WebView, JS bridge via script message handlers (§4.2) |
| HTTP (media) | `soup3` (libsoup 3) | Media from Reddit's CDNs only (§4.6), on the GLib main context; same library GStreamer's `souphttpsrc` uses |
| JSON | `serde`, `serde_json` | Bridge messages and Reddit JSON parsed into `agora-core` models |
| Portals | `ashpd` | OpenURI, Settings, FileChooser |
| D-Bus | `zbus` | GNOME Shell search provider |
| Local data | `rusqlite` (bundled SQLite) | Drafts and the saved subscription list (§14) |
| Image decoding | `glycin` | Same sandboxed loader stack as Loupe; supports animated formats |
| Markdown | `pulldown-cmark` plus a Reddit extension layer | Spoilers (`>!text!<`, pre-pass), superscript (`^`), `r/` and `u/` autolinks (post-pass). An HTML path is added only if the spike shows some operation returns HTML bodies (§4.5) |
| Video | `GtkVideo` / `GtkMediaFile` (GStreamer backend) | Reddit-hosted video is DASH/HLS; see §7.4 |
| i18n | `gettext-rs` | |
| Logging | `tracing` | Never logs cookies, CSRF tokens or post content at info level |
| Build | Meson + Cargo | Standard GNOME Rust template layout |

Tests that need an async executor use `glib::MainContext` directly (or `futures::executor` for display-free crates).

### 3.3 Repository layout (planned)

The code is a **Cargo workspace of small crates** so that display-free logic is testable with plain `cargo test`. Only `agora-app` and `agora-reddit-web` depend on GTK/WebKit; `agora-reddit-web` keeps its parsers in a display-free module.

```
agora/
├── Cargo.toml                 # workspace
├── crates/
│   ├── agora-core/            # shared types: IDs, models, errors, Backend trait, MockBackend
│   ├── agora-reddit-web/      # Backend impl: hidden WebView session, JS bridge, ops/*.js, parsers, pacing
│   ├── agora-markdown/        # Reddit Markdown → document AST (no GTK)
│   └── agora-app/
│       └── src/
│           ├── main.rs, application.rs, config.rs.in
│           ├── stores/     # GObject stores (§3.4)
│           ├── services/   # notifications, search provider, url routing, media loader, local data
│           ├── render/     # markdown AST → GTK widgets
│           └── ui/         # one module per page/widget (window, sidebar, post_row, …)
├── meson.build
├── build-aux/
│   ├── io.github.ClickCalickClick.Agora.Devel.json   # Flatpak manifest
│   └── gen-resources.py      # writes the gresource XML and po/POTFILES (§19)
├── data/
│   ├── *.desktop.in.in, *.metainfo.xml.in.in, *.gschema.xml.in
│   ├── *.search-provider.ini
│   ├── icons/ (scalable + symbolic)
│   └── resources/
│       ├── ui/<module>/*.blp
│       └── style/<module>.css
├── po/  (LINGUAS; POTFILES generated)
└── docs/
    ├── SPEC.md
    ├── STORES.md            # store and action contract, frozen at the end of slice 1
    ├── reviews/             # design reviews (SPEC-REVIEW.md, EGM reports)
    └── slices/              # one brief per slice (§20), written from this spec
```

### 3.4 Architecture overview

```
 ┌──────────────── UI (GTK main thread) ─────────────────┐
 │  Window → Pages → Rows/Widgets      (GObject props,   │
 │                                      list models)     │
 └───────────────▲──────────────────────────┬────────────┘
                 │ signals / list models    │ commands
 ┌───────────────┴──────────────────────────▼────────────┐
 │  Stores (GObject): Session, Feed, Thread, Inbox,      │
 │  Search, Profile, Subreddit, Subscriptions, Drafts,   │
 │  Filters, Prefs                                       │
 └───────────────▲──────────────────────────┬────────────┘
                 │ BackendEvent             │ async calls
 ┌───────────────┴──────────────────────────▼────────────┐
 │  Backend trait                                        │
 │  RedditWebBackend: request lanes · pacing · JS bridge │
 │  · parsers                       (MockBackend in tests)│
 └───────────────────────────────────────────┬───────────┘
                                             ▼
                          hidden WebKit view on www.reddit.com
```

- **Stores** are GObjects that own state and expose `gio::ListModel`s, so views bind to them directly (`GtkListView` + `GtkSignalListItemFactory`). Views never call the backend.
- **Backend** is reached only through the `Backend` trait (§4.3). There is no persistent cache between stores and backend in 1.0: stores keep what the current session has loaded in memory, and WebKit's and libsoup's own HTTP caches handle repeat loads.

---

## 4. Data access: the browser-session backend

### 4.1 Why a browser session

See §0.2 for the alternatives. Agora does **not** use the OAuth API or a client ID. It drives the user's own logged-in reddit.com session through a hidden web engine, and re-presents the results natively.

Agora is **public source, self-built, for personal use** (§1, §18). It works the way a user script or browser extension does, and it carries the same risks (§0.3):
- **Fragility:** reddit.com's private endpoints and page markup change without notice. Expect to update extractors (§4.5) regularly. All Reddit-specific knowledge lives in one crate and a set of versioned scripts so that a breakage is contained to one operation.
- **Account risk:** automated use of the website may violate Reddit's User Agreement. Pacing traffic like a person (§4.6) keeps the footprint small, but the risk to the account is the user's, and the user acknowledges it on first run (§6).

### 4.2 Architecture

```
 Agora UI (GTK)                                   hidden WebKitGTK WebView
 ───────────────                                  ─────────────────────────
 Stores ──► Backend trait ──► RedditWebBackend ──► call_async_javascript_function(
                                   ▲                  "return agora.run(op, args)",
                                   │                  {op, args}, world = "agora")
                                   │                          │  same-origin fetch
                                   │                          ▼
                                   └── parse JSON ◄── promise result (small)
                                       into models     or script message "agora"
                                                       (large payloads)
```

- **Engine:** WebKitGTK 6.0 via the `webkit6` crate (GTK 4 API), which the GNOME runtime ships.
- **Session:** one `webkit6::NetworkSession` with persistent data and cache directories (`$XDG_DATA_HOME/agora/web/` and `$XDG_CACHE_HOME/agora/web/`). Cookies, local storage and the login live there, as in a browser profile. Agora never reads or stores the password and never copies cookies out of WebKit.
- **Hidden view:** a `WebView` that is created but not shown, kept on a lightweight same-origin page on `https://www.reddit.com/` chosen during the spike, so all calls are same-origin and Reddit's full front-end app is not left running. It is a normal desktop WebKit; Agora does not override the User-Agent.
- **Isolated world:** the bridge runs in a named script world, **`agora`**. In the UI process this means passing the world name to three calls: `UserScript::for_world(..., "agora", ...)` for `agora-bridge.js`, `UserContentManager::register_script_message_handler("agora", Some("agora"))`, and every `call_async_javascript_function(..., Some("agora"), ...)`. Page scripts cannot call the bridge or post to its handler. Requests the bridge makes still travel over the page's network stack, so Reddit's servers see them like any other request from that page.
- **Calls:** an operation starts with `WebView::call_async_javascript_function` with body `return agora.run(op, args)` and a typed `arguments` dictionary (`op: String`, `args: JSON`). Arguments are never interpolated into script text. Results up to 64 KiB return as the promise value; larger results are posted to the `agora` message handler in chunks keyed by request id.
- **Bridge loading:** `agora-bridge.js` and `ops/*.js` are compiled into `agora-reddit-web` with `include_str!`, so the crate loads its own scripts under plain `cargo test` and in the app.
- **CSRF token:** where a write needs one, the operation reads it from where the spike finds it (a cookie or a DOM attribute) and records the source in `STRATEGIES.md`.
- **Threading:** WebKit lives on the GTK main thread. `RedditWebBackend` holds the view and exposes futures that are **not `Send`**, polled on the GLib main context.

### 4.3 Backend trait

The UI never talks to WebKit or parser types. `agora-core` defines:

- `trait Backend` — `#[async_trait(?Send)]` methods (or equivalent `LocalBoxFuture` returns) for the operations in §4.5's operation table. Used as `Rc<dyn Backend>`.
- `fn events(&self) -> async_channel::Receiver<BackendEvent>` — the backend reports `SessionExpired`, `Blocked { retry_after }` and `SignedIn(Username)` through this channel; the Session store listens.
- Reddit names throughout: `Subreddit`, `SubredditName`, `Username`, `PostId`, `CommentId`.
- `MockBackend` — an in-memory implementation with sample data and scripted failures, used by UI work and store tests.

`agora-reddit-web` is the only real implementation. The trait exists so the UI can be built and tested against `MockBackend`; it carries no capability flags.

Exact signatures are frozen at the end of slice 1 (§20) from what the spike and the first end-to-end slice show; until then they may change.

### 4.4 Sign-in and session

- **Sign in** opens a visible `WebView` in an `AdwDialog` on `https://www.reddit.com/login` using the same `NetworkSession`. The user logs in with Reddit's own page (password, 2FA, SSO and captchas are all handled by Reddit). Agora detects success when the `whoami` operation returns a username, then closes the dialog.
- **Session expiry:** if any operation returns "logged out", the backend emits `SessionExpired`; the UI shows an `AdwBanner` ("Signed out of Reddit · Sign In") and stops issuing requests.
- **Sign out** clears the `NetworkSession` website data, drafts, and the saved subscription list.
- No logged-out browsing: Reddit is moving to logged-in-only, and signing in is the first-run step (§6).

### 4.5 Operations and extraction

Every operation is a small, versioned JavaScript function in `crates/agora-reddit-web/js/ops/<operation>.js`, paired with a Rust parser in `src/parse/<operation>.rs`.

**One extraction method per operation.** The slice 0 spike (§20) tries each operation against a live session and picks the method that works, in this order of preference:
1. **Same-origin JSON** — `fetch()` with `credentials: "include"` against JSON endpoints the logged-in site serves (for example `/r/{sub}/hot.json`, `/comments/{id}.json`, `/api/me.json`). Gives Markdown bodies and full metadata.
2. **Site-internal endpoints** — endpoints reddit.com's own front end calls (for example partials under `/svc/shreddit/…`), parsed in the page with `DOMParser`.

The chosen method for each operation, and the date it was checked, are recorded in `crates/agora-reddit-web/STRATEGIES.md`. A second method is added to an operation only after the first one actually breaks. Agora does not navigate the hidden view to scrape full pages, and **never operates the page's own buttons or forms**.

**Writes** (vote, save, comment, submit, message) send the same request the site's own front end sends, and are confirmed from that request's response. Stores apply votes and saves optimistically and revert them, with an error toast, if the response is an error. There is no separate read-back request.

**Operation table** (frozen with the trait at the end of slice 1):

| Operation | Kind | Returns |
|---|---|---|
| `whoami` | read | `Option<Username>` |
| `listing` (feed, subreddit, user posts, saved; sort, time range, `after` cursor) | read | `Listing<Post>` |
| `post_with_comments` (post id, comment sort) | read | `(Post, Vec<CommentNode>)` |
| `more_comments` (post id, child ids) | read | `Vec<CommentNode>` |
| `subreddit_info` | read | `Subreddit` (includes posting requirements) |
| `subscriptions` | read | `Vec<Subreddit>` |
| `user_profile` | read | `User` |
| `inbox` (tab, `after` cursor) | read | `Listing<Message>` |
| `search` (query, scope, sort, time range, `after`) | read | `SearchResults` |
| `vote`, `save`, `hide`, `subscribe`, `mark_read` | write | `()` |
| `submit`, `comment`, `edit` | write | the created or edited item |
| `delete`, `report`, `send_message` | write | `()` |

"Load more" in comment trees uses `more_comments` like any other operation, with whatever method the spike selects.

**Self-test:** a "Check Reddit connection" action in Preferences (and `agora --self-test`) runs every read operation once, inside the pacing limits of §4.6 (so it takes about 15 seconds), and reports which ones broke.

**Body format:** if every read operation returns Markdown, `agora-markdown` has only a Markdown parser. If the spike shows an operation returns HTML bodies, an `html_to_ast` path is added for that operation then, with an allow-list of tags (p, a, em, strong, del, code, pre, blockquote, ul/ol/li, table, sup, spoiler spans).

### 4.6 Human-paced traffic

Agora generates no more traffic than a person browsing reddit.com would. The scheduler enforces these limits, and its time-mocked unit tests assert them.

**Request lanes:**
- **Interactive lane:** requests caused by a user action (open a post, scroll to the end of a feed, vote, search). A new interactive request for a different view drops any queued interactive requests for the view the user left; they are never sent.
- **Background lane:** inbox checks and the self-test. Background requests wait while any interactive request is queued.
- One request is in flight at a time across both lanes.

**Pacing constants:**

| Constant | Value |
|---|---|
| Minimum gap between requests | 1.0 s |
| Random jitter added to each gap | 0–500 ms, uniform |
| Burst after a user action | up to 2 requests with a 250 ms gap (for example post, then its comments) |
| Feed page size | 25 posts |
| Next feed page | requested when the user scrolls within 10 rows of the end |
| Inbox check interval | 15 min, only while the window is open |
| Backoff after pushback | 5 min, doubling each time, capped at 60 min |

- **No prefetch:** never load comments for posts the user has not opened; never pre-open links.
- **Pushback:** on HTTP 429, a captcha or interstitial page, or a redirect to the login page, stop all traffic, show a banner, and back off as above. Never retry automatically in a tight loop.
- **Media:** images and video load from Reddit's public media CDNs (`i.redd.it`, `preview.redd.it`, `v.redd.it`, `external-preview.redd.it`) only when on screen. They are fetched with libsoup using Agora's own honest User-Agent (`Agora/<version> (GNOME; +https://github.com/ClickCalickClick/agora)`) and no cookies. If the spike shows the CDN refuses that, media is fetched through the WebKit `NetworkSession` instead, so it is real browser traffic. Agora never copies the WebView's User-Agent into another client.

### 4.7 Networking and errors

- Network state comes from `gio::NetworkMonitor`. When offline, a banner says "You're offline" and actions are disabled; nothing is queued.
- Errors surface as `AdwToast`s with a Retry action, never modal dialogs.
- `AgoraError` variants: `Network`, `SessionExpired`, `Blocked { retry_after }`, `NotFound`, `Forbidden`, `ExtractorBroken { operation }`, `Parse`, `Io`.

---

## 5. Window layout and navigation

### 5.1 Three-pane adaptive layout

```
Wide (≥ 1100 sp)
┌──────────────┬──────────────────────┬───────────────────────────────┐
│ Sidebar      │ Post list            │ Post view                     │
│ ⌕ Search     │ [Hot ▾]   r/gnome    │ r/gnome · u/someone · 3h      │
│ Home         │ ┌──────────────────┐ │ Title of the post             │
│ Popular      │ │▢ Title…       ▲ 42│ │ [media]                       │
│ Saved        │ │  r/gnome · 3h  💬9│ │ body text…                    │
│ Inbox (3)    │ └──────────────────┘ │ ▲ 42 ▼   💬 9   ⋯              │
│ ── Subs ──   │ ┌──────────────────┐ │ ── Comments ──                │
│ r/gnome      │ │…                 │ │ │ comment                     │
│ r/rust       │ └──────────────────┘ │ │  │ reply                    │
└──────────────┴──────────────────────┴───────────────────────────────┘

Medium (700–1100 sp): sidebar collapses into an overlay; list + post view stay side by side.
Narrow (< 700 sp, phones): single pane, navigation stack: Sidebar → List → Post.
```

- Implemented with an outer `AdwOverlaySplitView` (sidebar) wrapping an `AdwNavigationSplitView` (list / content), driven by `AdwBreakpoint`s at the widths above.
- Sidebar is an `AdwSidebar` with sections: Feeds (Home, Popular), You (Saved, Inbox, Drafts, Profile), Subscriptions (alphabetical, favorites pinned on top). There is no "All" feed (r/all was removed; §0.1).
- Each pane has its own `AdwHeaderBar`. The sort dropdown (Best/Hot/New/Top/Rising, Top with time range) lives in the list pane's header bar.
- Back navigation: `AdwNavigationView` provides the back button, Alt+←, mouse back button, and the edge-swipe gesture on touch.

### 5.2 Main menu

Primary menu (`open-menu-symbolic`) in the sidebar header: Preferences, Keyboard Shortcuts, About Agora. Account avatar button opens a popover: profile, Sign Out.

### 5.3 Screens

| Screen | Widget base | Contents |
|---|---|---|
| Welcome | `AdwDialog` with `AdwCarousel` | §6 |
| Feed / post list | `GtkListView` in `GtkScrolledWindow` | Post rows (§7.1), pull-to-refresh, infinite scroll |
| Post view | `AdwNavigationPage` | Header, media, body, action bar, comment tree |
| Subreddit header | Area above the list | Icon, name, description, member count, Join/Leave, rules (opens dialog) |
| User profile | `AdwNavigationPage` | Avatar, karma, cake day; tabs: Posts, Comments, Saved (own profile only) via `AdwViewSwitcher` |
| Inbox | `AdwNavigationPage` | Tabs: All, Replies, Mentions, Messages; unread rows bold; Mark All Read |
| Drafts | `AdwNavigationPage` | Saved drafts by context; open one to resume it; each has a delete button |
| Composer | `AdwDialog` (sheet on narrow) | §9 |
| Search | `GtkSearchBar` in list pane | §10 |
| Image viewer | Fullscreen overlay page | §7.3 |
| Preferences | `AdwPreferencesDialog` | §13 |
| About | `AdwAboutDialog` | Credits Giara, links to repo, CoC, translators, disclaimer |
| Shortcuts | `AdwShortcutsDialog` | §11 |

### 5.4 Empty, loading, and error states

Every list has an `AdwStatusPage` for empty ("No posts here yet"), signed-out ("Sign in to see your inbox"), offline, and error states. Loading uses `AdwSpinner` placeholders, never a blocking dialog.

---

## 6. Onboarding (first launch)

A short welcome in an `AdwDialog` with an `AdwCarousel`:

1. **Welcome:** app icon, one-sentence description, the short disclaimer from §18 (unofficial, not affiliated with Reddit, personal use, may break, use at your own risk) with a link to the full text in About, and a required check button: "I understand that using Agora may put my Reddit account at risk". Continue stays insensitive until it is checked; the acknowledgement is stored in GSettings (`risk-acknowledged`).
2. **Sign in to Reddit:** a "Sign In" button opens Reddit's real login page in an embedded web view (§4.4). Nothing to register, no client ID.
3. **Done:** quick tips (keyboard shortcuts, Preferences).

If the dialog is closed before signing in, the main window shows an `AdwStatusPage` "Sign in to Reddit" with a Sign In button. Sign In / Sign Out also live in the account popover and Preferences.

---

## 7. Reading experience

### 7.1 Post rows (list)

Text-first rows in a `GtkListView`:

- Title (wraps to 3 lines, `.heading` weight), then a subtitle line: `r/sub · u/author · 3h · flair`.
- Small rounded thumbnail on the trailing edge when the post has media.
- Trailing column: score and comment count. Vote state shown by the score's color (accent for up, a muted color for down) and in the accessible description (§16).
- Badges as small pills: NSFW, Spoiler, Pinned, OC, Locked.
- Read posts dim their title (`.dim-label`).
- **Large card previews** (Preferences toggle): rows become cards with full-width media above the title.

Interactions:
- Click or Enter opens the post.
- Right-click or long-press opens a context menu: Upvote, Downvote, Save, Hide, Share, Copy Link, Open in Browser, Open Subreddit, View Author, Mute Subreddit, Report.
- Pull to refresh on touch (`GtkScrolledWindow` edge overshoot + spinner); F5 / Ctrl+R on keyboard.
- Infinite scroll: next page as in §4.6.

### 7.2 Post view

- Header: subreddit (link), author (link), age, flair, edited marker.
- Media (§7.3–7.4), a link card for link posts (domain and title; opens in browser), or body rendered from Markdown (§7.6).
- Action bar: Upvote / score / Downvote, Comments count, Save, Share, overflow menu (Copy Link, Open in Browser, Hide, Report, Edit/Delete for own posts).
- Comment sort dropdown (Best, Top, New, Controversial, Old, Q&A).
- Comment tree (§8).
- Content zoom: Ctrl++ / Ctrl+- / Ctrl+0 scale post and comment text in 10% steps (80%–200%, persisted in GSettings). UI chrome is not scaled; it follows the system text size.

### 7.3 Images and galleries

- Inline in post view at fitting width; galleries use an `AdwCarousel` with dots and arrow buttons.
- Tap or click opens a **fullscreen viewer** with: black background; pinch, Ctrl+scroll and double-tap zoom; pan; swipe left/right or arrow keys between gallery items; swipe down or Esc to close; toolbar with Save As (file chooser portal), Copy, Share, Open in Browser.
- Images decoded with `glycin` (sandboxed, supports animated WebP/GIF).
- **Image hosts:** only Reddit's media CDNs (§4.6) load automatically. Images on any other host show a placeholder with the host name and load only when the user clicks it.

### 7.4 Video

- Inline `GtkVideo` in post view. Tap to play; **no autoplay**. Controls: play/pause, seek, volume, fullscreen.
- Reddit-hosted video (`v.redd.it`) is delivered as DASH/HLS with separate audio. Play the HLS playlist through GStreamer (`hlsdemux2` in gst-plugins-good and DASH support in gst-plugins-bad, both in the target runtime). `souphttpsrc` uses Agora's honest User-Agent (§4.6). **Fallback:** play the highest `fallback_url` MP4 (video only) with a "No audio" note, plus Open in Browser.
- External video (YouTube etc.) shows a link card that opens in the browser.
- **Short silent GIFs/GIFVs** (no audio track, 30 s or shorter) loop automatically in the post view, **unless** the system "reduce animation" setting is on (`gtk-enable-animations` false), in which case they show a play button. They never autoplay in the list.

### 7.5 NSFW and spoilers

- NSFW media is blurred (CSS `filter: blur()` on a `GtkPicture`) with an overlay "NSFW · Tap to Show" button. Reveal is per item, not sticky.
- Preference: "Show NSFW content" (Off hides NSFW posts entirely; On shows blurred; separate toggle "Blur NSFW media", default on).
- Spoiler posts and `>!spoiler!<` text are hidden the same way.

### 7.6 Markdown rendering

Rendering target is native widgets, not a WebView:
- Paragraphs, headings, emphasis, links → `GtkLabel` with Pango markup (selectable, `wrap`, links activate through the URL router §12).
- Code blocks → monospace `GtkLabel` in a styled frame with a horizontal scroller.
- Block quotes → indented box with a left border.
- Lists → nested boxes with bullet/number labels.
- Tables → `GtkGrid` inside a horizontal scroller.
- Spoilers → hidden spans revealed on click.
- Images embedded in text → inline pictures for Reddit CDN hosts; click-to-load placeholders for other hosts (§7.3).

The AST (`agora-markdown`) has these node types: blocks `Paragraph`, `Heading(level)`, `CodeBlock(lang)`, `BlockQuote`, `List(ordered)`, `ListItem`, `Table`, `TableRow`, `TableCell`, `ThematicBreak`; inlines `Text`, `Emphasis`, `Strong`, `Strikethrough`, `Code`, `Link(url)`, `Image(url, alt)`, `Superscript`, `Spoiler`, `SubredditLink`, `UserLink`, `LineBreak`.

---

## 8. Comments

### 8.1 Tree

- Comments render as a flattened `GtkListView` of rows with a depth value, which keeps virtualization for threads with thousands of comments.
- Each depth level is indented and draws a **thread line**: 2 px, neutral gray, which turns the **accent color on hover or keyboard focus** along that comment's whole branch.
- Clicking a thread line or the comment header collapses the branch. The collapse is animated by fading the hidden rows (opacity), never by animating row height, because rows are recycled; it respects reduced animation. Collapsed state shows "author · N replies".
- Max visual depth 8; deeper replies show "Continue thread →" which pushes a new page rooted at that comment.
- **Load more**: "N more replies" rows load inline through the `more_comments` operation (§4.5) and replace themselves with the results.
- OP's comments get an "OP" badge; the user's own comments get a "You" badge.

### 8.2 Comment actions

Upvote, Downvote, Reply, Save, Share/Copy Link, and for own comments Edit and Delete. Available through a hover/focus action row on desktop and a long-press menu on touch.

---

## 9. Composer

One composer used for new posts, replies, edits, and messages; opened as an `AdwDialog` (fills the screen on narrow).

- **Post types:** Text, Link, and Image/Video (upload via file chooser portal). Polls are not in 1.0.
- Subreddit picker with search; shows the subreddit's posting requirements and rules, required flair picker, NSFW/Spoiler toggles.
- **Editor:** `GtkSourceView` 5 with Markdown syntax highlighting, spell check (`libspelling`), and a **formatting toolbar**: Bold, Italic, Strikethrough, Link, Quote, Code, Spoiler, Bulleted list, Numbered list, Heading.
- **Live preview toggle** (`AdwToggleGroup`: Write / Preview) using the same renderer as §7.6. On wide windows, side-by-side preview is offered.
- **Auto-saved drafts:** saved 2 s after the last keystroke and on close, keyed by context (new post in r/x, reply to comment y, message to u/z). Reopening the same context restores the draft. Drafts are listed under "You → Drafts" in the sidebar.
- Closing with unsent text asks: Discard / Save Draft (`AdwAlertDialog`).
- Character counter for titles (300 max).

---

## 10. Search

- `Ctrl+F` or the search button opens a `GtkSearchBar` in the post list pane.
- **Scope:** when inside a subreddit, search is scoped to it by default, with a toggle "Search all of Reddit". On Home/Popular it searches everything.
- Results tabs: Posts, Subreddits, Users. Sort and time-range options in a dropdown.
- The sidebar search entry filters subscriptions instantly and offers "Go to r/…" for any typed name (Ctrl+K focuses it).
- **GNOME Shell search provider:** registered via `*.search-provider.ini` and the `org.gnome.Shell.SearchProvider2` D-Bus interface (zbus). Matches typed text against the saved subscription list (§14) only; never makes network requests. Activating a result opens Agora on that subreddit.

---

## 11. Keyboard

Full keyboard operation; every action reachable without a pointer. Shortcuts listed in an `AdwShortcutsDialog` (Ctrl+?). The accelerator table lives in `application.rs`.

| Key | Action |
|---|---|
| j / k | Next / previous post (list) or comment (post view) |
| Enter / o | Open selected post |
| a / z | Upvote / downvote |
| s | Save |
| r | Reply (post view) |
| c | New post (compose) |
| Space | Collapse/expand selected comment |
| h / Alt+← | Back |
| Ctrl+F | Search |
| Ctrl+K | Go to subreddit |
| Ctrl+L | Open Reddit link from clipboard |
| F5 / Ctrl+R | Refresh |
| Ctrl++ / Ctrl+- / Ctrl+0 | Content zoom |
| Ctrl+, | Preferences |
| Ctrl+? | Keyboard shortcuts |
| Ctrl+W / Ctrl+Q | Close window / Quit |
| F9 | Toggle sidebar |

Single-letter shortcuts are disabled while a text entry has focus.

---

## 12. Links, sharing, and the desktop

### 12.1 In-app link routing

A `UrlRouter` service parses every activated link:
- `reddit.com`, `old.reddit.com`, `www.reddit.com`, `np.reddit.com`, `redd.it`, `/r/…`, `/u/…` → open in Agora (post, comment permalink, subreddit, user).
- `i.redd.it`, `preview.redd.it` → image viewer.
- Everything else → default browser through the OpenURI portal.

### 12.2 Opening Reddit links from outside

**Decided.** On Linux, an app cannot claim `https://` links for one domain; registering for `x-scheme-handler/https` would make Agora the handler for all web links. So Agora does not register a global handler. Instead it:
1. accepts `reddit.com` URLs as command-line arguments and through `GApplication` open (desktop file `%U`), so "Open With → Agora" and `agora https://reddit.com/...` work;
2. offers "Open Link from Clipboard" (Ctrl+L), which detects a Reddit URL on the clipboard;
3. accepts URLs dropped on the window.

### 12.3 Sharing

**Decided.** GNOME has no xdg-desktop-portal Share portal in the target runtime. The Share menu offers Copy Link (with a toast), Open in Browser, and Send by Email (`mailto:` via OpenURI). Adopt a share portal if one is added to GNOME later.

### 12.4 Notifications

- Desktop notifications for new inbox replies, username mentions, and messages, sent with `GNotification` (portal-backed in Flatpak), found by the inbox check that runs every 15 minutes **while the window is open** (§4.6). Clicking a notification opens the item. Several arrivals are grouped into one notification.
- If a later inbox check shows an item was deleted or removed, its notification is withdrawn (`gio::Application::withdraw_notification`). Notifications never quote text the inbox check already reports as deleted.
- Background checking after the window closes is not in 1.0 (§20, "Later").
- Notifications are off for signed-out users.

### 12.5 Other integrations

- Follows system color scheme (`AdwStyleManager`) and **system accent color** (no Reddit orange anywhere).
- Follows system font and text size.
- Uses the file chooser portal for uploads and Save As.
- Respects `gtk-enable-animations` (reduced motion) everywhere.

---

## 13. Preferences (`AdwPreferencesDialog`)

**General:** Default feed (Home/Popular), default sort, open links in Agora vs browser, mark posts read on open.
**Appearance:** Large card previews, content text size (zoom slider, mirrors Ctrl+/-), show thumbnails, show scores.
**Content:** Show NSFW content, blur NSFW media, blur spoilers, autoplay silent GIFs, muted subreddits (list editor), muted keywords (list editor; matched case-insensitively against titles).
**Notifications:** Enable notifications; which types (replies, mentions, messages).
**Account:** signed-in user, Sign Out, "Check Reddit connection" self-test (§4.5).

All persisted in GSettings, schema `io.github.ClickCalickClick.Agora` (`data/io.github.ClickCalickClick.Agora.gschema.xml.in`; the Devel build installs the same schema ID). Keys:

| Key | Type | Default | Range / values |
|---|---|---|---|
| `window-width` | `i` | 1100 | ≥ 360 |
| `window-height` | `i` | 720 | ≥ 360 |
| `window-maximized` | `b` | false | |
| `risk-acknowledged` | `b` | false | set on the Welcome page (§6) |
| `default-feed` | `s` (enum) | `home` | `home`, `popular` |
| `default-sort` | `s` (enum) | `best` | `best`, `hot`, `new`, `top`, `rising` |
| `default-top-range` | `s` (enum) | `day` | `hour`, `day`, `week`, `month`, `year`, `all` |
| `default-comment-sort` | `s` (enum) | `best` | `best`, `top`, `new`, `controversial`, `old`, `qa` |
| `open-links-in-agora` | `b` | true | |
| `mark-read-on-open` | `b` | true | |
| `large-cards` | `b` | false | |
| `content-zoom` | `d` | 1.0 | 0.8–2.0, step 0.1 |
| `show-thumbnails` | `b` | true | |
| `show-scores` | `b` | true | |
| `show-nsfw` | `b` | false | |
| `blur-nsfw` | `b` | true | |
| `blur-spoilers` | `b` | true | |
| `autoplay-gifs` | `b` | true | |
| `muted-subreddits` | `as` | `[]` | lowercase names without `r/` |
| `muted-keywords` | `as` | `[]` | |
| `notifications-enabled` | `b` | true | |
| `notify-replies` | `b` | true | |
| `notify-mentions` | `b` | true | |
| `notify-messages` | `b` | true | |
| `favorite-subreddits` | `as` | `[]` | pinned at the top of Subscriptions |

Read state of posts is kept in memory for the session only in 1.0.

---

## 14. Local data

There is **no offline cache in 1.0** (§20, "Later"). Agora keeps only:

- **Drafts** and the **saved subscription list** in `$XDG_DATA_HOME/agora/agora.db` (SQLite, one file, small writes on the main context). The subscription list is refreshed from the `subscriptions` operation at sign-in and when the user subscribes or unsubscribes; the Shell search provider (§10) reads it.
- **WebKit's profile** in `$XDG_DATA_HOME/agora/web/` and its HTTP cache in `$XDG_CACHE_HOME/agora/web/`, managed by WebKit.
- **libsoup's HTTP cache** for media in `$XDG_CACHE_HOME/agora/media/`, limited to 200 MB, managed by `soup::Cache`.
- In-memory store contents for the current session.

**Deleted content:** Agora never stores post or comment bodies on disk, so it cannot show content after it is gone from Reddit. When a refresh shows a post or comment as deleted or removed, Agora shows the same placeholder reddit.com shows. There is no "show deleted" feature.

**Sign out** deletes `agora.db` contents (drafts, subscriptions) and the WebKit website data.

---

## 15. Filters

- **NSFW:** §7.5.
- **Muted subreddits:** their posts are removed from Home and Popular; visiting the subreddit directly still works with a banner "You muted this subreddit · Unmute".
- **Muted keywords:** posts whose titles match are hidden from feeds. A collapsed "N posts hidden by filters" row appears at the end of a page so that filtering is never silent.
- Filtering is local only (not synced to Reddit). It is implemented in the Filters store, which the Feed store calls on every page it receives.

---

## 16. Accessibility (from day one)

- Every custom widget sets an accessible role, label, and description (`gtk::Accessible`). Post rows announce: title, subreddit, author, age, score, comment count, vote state, badges.
- Comment rows announce depth ("Level 3 reply by …") and collapsed state; expose `GTK_ACCESSIBLE_ROLE_TREE_ITEM` semantics where possible.
- Vote buttons are toggle buttons with pressed state, not color only.
- Focus order follows reading order; focus is visible; nothing is reachable by hover only (thread-line highlight and comment actions also appear on focus).
- All images have alt text: Reddit captions when present, otherwise "Image from post: <title>".
- Contrast: rely on libadwaita colors; no custom colors below WCAG AA.
- Testing: an Orca walkthrough and a keyboard-only walkthrough of every screen at the end of each slice, done by hand. Automated checks: a unit test per custom widget asserts that its accessible label is non-empty.

---

## 17. Internationalization

- All user-facing strings through gettext; plurals via `ngettext`.
- Dates and numbers via GLib (`GDateTime` formatting, locale-aware "3 hours ago").
- RTL layouts tested (Hebrew/Arabic pseudolocale).
- **Translation platform:** GNOME's Damned Lies requires hosting on GNOME GitLab, and the repo is on GitHub. 1.0 ships English with full gettext plumbing. If contributors ask to translate, set up Weblate (hosted.weblate.org libre plan) then.
- Spelling: American English, as in GNOME.

---

## 18. Disclaimers, legal hygiene and HIG checklist

Agora is public source on GitHub but is **not distributed**: no Flathub, no app stores, no prebuilt binaries. Flathub and GNOME Circle are not goals.

### 18.1 Disclaimer requirements

`DISCLAIMER.md` and the README text exist from the start; slice 1 wires them into the app; every later slice keeps them intact.

1. **Disclaimer text.** One canonical text, `DISCLAIMER.md` at the repo root, reused verbatim (translatable) in the app:
   > Agora is an unofficial, independent project. It is not affiliated with, endorsed by, or sponsored by Reddit, Inc. "Reddit" is a trademark of Reddit, Inc. and is used only to describe what this app works with.
   > Agora is for personal use. It shows Reddit through your own reddit.com login, inside an embedded web browser. Automated use of reddit.com may violate Reddit's User Agreement; you are responsible for how you use it and for your account.
   > Agora depends on reddit.com's website and may stop working at any time.
   > The software is provided "as is", without warranty of any kind, as stated in the GPL-3.0 license. Use it at your own risk.
2. **README:** the disclaimer appears near the top, before any build instructions.
3. **About dialog:** the full disclaimer in a "Disclaimer" section of `AdwAboutDialog` (via `add_legal_section`), plus the GPL license. The Welcome page shows the short form and the risk acknowledgement (§6).
4. **No Reddit marks.** No Reddit logo, Snoo, wordmark, colors or screenshots of Reddit's own UI in the icon, README, metainfo or repo artwork. "Reddit" appears only in plain text, descriptively ("a client for Reddit"), never as part of the app name.
5. **No Reddit app credentials, ever.** The repo and the app contain no Reddit OAuth client IDs or secrets, no API keys, and no credentials extracted from Reddit's own web or mobile apps. The app only uses the cookies WebKit holds for the user's own login. CI runs a secrets scan (`gitleaks`) on every push.
6. **No user data in the repo.** Fixtures are scrubbed of usernames, IDs that identify the user, tokens and cookies before they are committed (§20.1 rule 5). Fixture content written by other Reddit users is kept to the minimum needed and anonymized.
7. **No circumvention.** Agora never bypasses captchas, rate limits, logins, paywalls, bans or blocks, and never disguises its own HTTP client as a browser. When Reddit pushes back, it stops (§4.6).
8. **License:** GPL-3.0-or-later, whose warranty disclaimer and limitation of liability (sections 15–16) apply. `LICENSE` is added in slice 1.

Disclaimers lower the risk but do not guarantee that no one is held responsible. If that matters, get advice from a lawyer before inviting others to use the app.

### 18.2 HIG checklist

| Requirement | How Agora meets it |
|---|---|
| Follows the GNOME HIG | libadwaita widgets throughout, adaptive layout, `AdwToolbarView` (raised top bar on scrolled pages), no custom window chrome |
| Window state | Size and maximized state persisted in GSettings |
| About | `AdwAboutDialog::from_appdata` with release notes, Giara credit, and the disclaimer section |
| App icon | Original icon per §2.1, symbolic icon designed first; no Reddit marks |
| AppStream metadata | Valid (`appstreamcli validate`) and states the disclaimer in its description |
| Translatable | gettext throughout |
| Accessible | §16 |
| Adaptive / mobile | §5, tested at 360×640; desktop file has `X-Purism-FormFactor=Workstation;Mobile;` (kept so the app works on GNOME phones the user builds it for) |
| Privacy | No telemetry. Automatic network traffic goes only to reddit.com (through the hidden web view) and Reddit's media CDNs. Other hosts are contacted only when the user clicks a link or a click-to-load image |

### 18.3 Flatpak manifest

`build-aux/io.github.ClickCalickClick.Agora.Devel.json`, built by each user with `flatpak-builder --user --install` (no bundles are published):
- Runtime/SDK `org.gnome.Platform` / `org.gnome.Sdk` at the target version (§3.1), plus `org.freedesktop.Sdk.Extension.rust-stable`.
- **WebKitGTK 6.0** (`webkitgtk-6.0`) and libsoup 3 come from the GNOME runtime; no extra module. CI checks that `pkg-config --modversion webkitgtk-6.0` succeeds in the SDK, and adds `libspelling` as a module only if the SDK lacks it.
- Finish args: `--share=network`, `--share=ipc`, `--socket=wayland`, `--socket=fallback-x11`, `--device=dri`, `--socket=pulseaudio` (video audio). No filesystem access (portals for files). OpenURI, FileChooser and Notification go through portals. No `org.freedesktop.secrets` talk-name (no secrets are stored outside WebKit's own profile).
- WebKit's web process sandbox runs inside the Flatpak sandbox as in GNOME Web; no extra permissions needed.

---

## 19. Testing and quality

- **Unit tests:** every `agora-reddit-web` parser against recorded fixtures (never live Reddit in CI); Markdown → AST; URL router; filter matching; pacing scheduler and request lanes (time-mocked, asserting every value in §4.6); store logic against `MockBackend`.
- **Bridge integration:** a local test server serves saved reddit.com responses to a real hidden WebView under `xvfb-run`, so `ops/*.js` and parsers are tested end to end without touching Reddit.
- **Live check:** `agora --self-test` (§4.5) against the user's own session, run by hand, never in CI.
- **Mock mode:** `AGORA_MOCK=1` starts the app on `MockBackend`, for UI work, screenshots and manual tests without a Reddit account.
- **UI:** manual test script per slice (desktop, narrow 360 px, touch, keyboard-only, Orca).
- **CI (GitHub Actions)**, running in the `ghcr.io/flathub-infra/flatpak-github-actions:gnome-51` container (GNOME 51 SDK), is the single authoritative list:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test --workspace` (GTK-dependent tests under `xvfb-run`)
  - `cargo deny check`
  - `gitleaks` secrets scan
  - Flatpak build of the Devel manifest
  - `appstreamcli validate`, `desktop-file-validate`, `glib-compile-schemas --strict`
- **Generated lists:** Meson has no glob function, so `build-aux/gen-resources.py` scans `data/resources/` and `crates/agora-app/src/` and writes the gresource XML and `po/POTFILES`. It runs from a Meson `custom_target` whose `depend_files` include the scanned directories; Blueprint files are compiled with `blueprint-compiler batch-compile` into a target that `gnome.compile_resources` depends on. Adding a new file needs a `meson setup --reconfigure`, which CI always does from scratch.
- Rust edition 2024; MSRV pinned to the Rust SDK extension of the target runtime.

---

## 20. Build plan: vertical slices

Agora is built in **slices**. Each slice delivers a working app that does more than the previous one, end to end, and ends with the accessibility and keyboard walkthrough (§16). Slices run in order. A slice is owned by one lead agent (one project thread), which may hand self-contained modules inside the slice to sub-agents, but integrates and tests the slice itself.

| Slice | Name | Delivers | Exit criteria |
|---|---|---|---|
| 0 | Spike (live, with Jared) | A throwaway prototype: hidden WebView, isolated world `agora`, `call_async_javascript_function`, and each read operation in §4.5 plus `vote` and `comment` tried against a live session. Measures whether the hidden view is throttled, where the CSRF token lives, whether media CDNs accept an honest User-Agent, and which pages work as the lightweight home page. Records scrubbed fixtures | `STRATEGIES.md` lists a working method for every operation in §4.5 (or marks it missing); fixtures committed. **Go/no-go:** if `listing`, `post_with_comments` or `more_comments` has no working same-origin method, stop and revisit §0.2 with Jared before slice 1 |
| 1 | Reading on mock data | Scaffold (Meson, Cargo workspace, Flatpak manifest, CI, gschema, i18n, `DISCLAIMER.md`, `LICENSE`, About); `agora-core` (models, `Backend`, `MockBackend`, errors); `agora-markdown`; stores; window and three-pane layout; sidebar; post list and rows; post view with text/link posts; comment tree with collapse; Markdown renderer; empty/error states; j/k | Under `AGORA_MOCK=1`, reading works on desktop and at 360 px, by keyboard and with Orca. **Contracts frozen:** `Backend` signatures and operation table (§4.3, §4.5), `docs/STORES.md` (stores, properties, signals, `ListModel` item types, window actions), gschema keys (§13) |
| 2 | Reading real Reddit | `agora-reddit-web`: session, bridge, request lanes and pacing, read extractors from slice 0; Welcome dialog with risk acknowledgement; embedded login; sign out; session-expired banner; `--self-test`; subscriptions list in `agora.db` | Signed in, the user can browse Home, Popular, subreddits and threads (including load more); the self-test passes; pacing tests pass |
| 3 | Media | Media loader (libsoup), images, galleries, fullscreen viewer, video with HLS and fallback, GIF rules, NSFW/spoiler blur, large cards, click-to-load for other hosts | Text, link, image, gallery, `v.redd.it` video, GIF and external-video posts all display as specified |
| 4 | Interaction | Write extractors; voting, saving, hiding, reporting, subscribe/unsubscribe; composer (all modes) with toolbar, preview, drafts; edit/delete; user profiles | The user can post, reply, edit, delete, vote and save end to end, each confirmed from Reddit's response |
| 5 | Inbox and desktop | Inbox and messages; notifications while the window is open; search with scope toggle; Shell search provider; URL routing, open-from-clipboard, drag-and-drop; share menu; Preferences complete; filters | Each feature in §10, §12 and §15 works inside the Flatpak build |
| 6 | Polish and source release | App and symbolic icon, empty-state art, shortcuts dialog, AppStream screenshots (from mock mode), full Orca and RTL pass, build instructions in README, tagged release notes (no binaries) | A fresh clone builds and installs with the documented `flatpak-builder` command; disclaimer present in README, About and onboarding; Jared uses it signed in for 7 days of ordinary reading and commenting with no captcha or block |
| Later | | Offline cache of viewed and saved posts with size limit and Clear Cache; background inbox checking after the window closes (Background portal, 30 min minimum); offline action queue; polls; share portal if one is added to GNOME; multi-account | |

### 20.1 Rules for every slice

1. **Read first:** this spec in full. A slice brief (`docs/slices/SLICE-N.md`) adds goal, scope, deliverables and exit checklist; it does not repeat the spec.
2. **One branch and one PR per slice**, named `slice-N-short-name`. The PR description lists the exit criteria with checkboxes. Sub-agents inside a slice work on branches off the slice branch, each limited to the module paths the lead gives them.
3. **Contracts:** before the end of slice 1, `agora-core`, `docs/STORES.md` and the gschema may change freely. After slice 1, changing them needs Jared's sign-off and a note in `docs/CONTRACTS-CHANGELOG.md`.
4. **Definition of done:** the CI list in §19 passes; public items are documented; user-facing strings are translatable; UI work includes the §16 keyboard and screen-reader checks and screenshots at 360 px and 1280 px widths taken in mock mode (for example with `gnome-screenshot` under a nested `mutter --headless` session, or by hand). If no display is available, the keyboard walkthrough checklist replaces the screenshots.
5. **Fixtures, never live Reddit:** agents never send traffic to reddit.com. Live runs (slice 0, the self-test, and the slice exit checks marked "signed in") are done by Jared or by the lead with Jared, at human pace. Fixtures are recorded only from those runs, scrubbed of usernames and tokens, and committed to `crates/agora-reddit-web/tests/fixtures/`.
6. **Review:** at the end of each slice, run an EGM drift report (spec vs code) and decide with Jared whether the spec or the code changes.

---

## 21. Decisions

Settled during the 2026-10-31 reviews (`docs/reviews/`):
1. **Data access:** no Reddit API; a logged-in WebKit session with a JS bridge in an isolated world (§4). Reddit only.
2. **Extraction:** one method per operation, chosen by the slice 0 spike; a fallback only after a method breaks; never operate the page's buttons (§4.5).
3. **Backend trait:** kept as the seam for `MockBackend`; Reddit names; no capability flags (§4.3).
4. **HTTP:** no tokio or reqwest. Media through libsoup with Agora's own User-Agent, falling back to WebKit's network session if the CDN refuses (§4.6).
5. **1.0 scope:** no offline cache and no background checking after the window closes; drafts and a saved subscription list are kept (§14, §20).
6. **Build plan:** vertical slices replace the 23 parallel work packages (§20).
7. **External reddit.com links:** no global `https` handler; `GApplication` open-URI, Ctrl+L, drag-and-drop (§12.2).
8. **Sharing:** Copy Link / Open in Browser / Send by Email menu (§12.3).
9. **Translation:** gettext plumbing in 1.0; Weblate only if contributors ask (§17).
10. **App ID:** `io.github.ClickCalickClick.Agora` (§2).
11. **Distribution:** none. Public source, self-built; Flathub, stores, binaries and Circle are out (§1, §18).
12. **Disclaimers:** README, About dialog and onboarding, from one `DISCLAIMER.md`, plus a first-run risk acknowledgement (§6, §18).

Settled by slice 0 (`docs/SPIKE-0.md`, 2026-11-01):
13. **Extraction results:** recorded in `crates/agora-reddit-web/STRATEGIES.md`. Same-origin `.json` is blocked; every operation uses reddit.com's own partials and GraphQL calls. The spec changes listed in `docs/SPIKE-0.md` §6 are folded into §4 at the start of slice 1.
14. **Posting:** written in Agora and submitted through the hidden page, which runs Reddit's own invisible reCAPTCHA as shipped. One user press gives one post. Agora never retries, batches, caches tokens or uses a solver. A visible challenge or captcha rejection opens Reddit's own form in a dialog for the person to finish. Slice 4 confirms live that such posts are visible to other readers (`docs/SPIKE-0.md` §5).

---

## 22. Credits

Agora is inspired by [Giara](https://gitlab.gnome.org/World/giara), the GTK Reddit client by Gabriele Musco and contributors. No Giara code is reused; the credit appears in the README and the About dialog (`AdwAboutDialog` "Acknowledgements" section).
