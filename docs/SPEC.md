# Agora: Design Specification

**Status:** Revised after review (2026-10-31): public source, self-built, browser-session backend
**Owner:** Jared (ClickCalickClick)
**Inspiration:** [Giara](https://gitlab.gnome.org/World/giara) by Gabriele Musco

Agora is a native GNOME client for Reddit, written in Rust with GTK 4 and libadwaita. It is a new project, not a fork: it takes Giara's idea (a Reddit app that feels like it belongs on GNOME) and rebuilds it for current GNOME, adaptive layouts, and touch.

**Public source, self-built, not distributed.** Reddit is closing its public API (see `SPEC-REVIEW.md`), so Agora reads and writes Reddit through the user's own logged-in browser session in an embedded web engine (§4). The source is public on GitHub with clear disclaimers (§18); there are no binaries, store listings or Flathub releases. Each user builds and runs it themselves, on their own account, for personal use.

This document is the single source of truth for what Agora does and how it is built. Anything marked **Open question** must be settled during review, before the matching roadmap phase starts.

---

## 1. Goals and non-goals

### Goals

1. Feel like a first-party GNOME app: HIG-compliant, adaptive, accessible, translatable.
2. Hold to the [GNOME Circle](https://circle.gnome.org/) quality bar (HIG, accessibility, translatability) even though this build is not submitted anywhere.
3. Cover everyday Reddit use: browsing, reading, voting, posting, replying, and the inbox.
4. Work equally well with mouse and keyboard, touch, and a screen reader.
5. Public source on GitHub, self-built by each user, with the disclaimers in §18.
6. Generate traffic indistinguishable from a person using reddit.com in a browser (§4.6).
7. Keep every Reddit-specific detail behind a clean `Backend` trait (§4.3). Only the Reddit backend is in scope.

### Non-goals

- Moderation tools (mod queue, removals, flair management, mod mail). Out of scope permanently, not just deferred.
- Chat (Reddit's chat API is not public).
- Swipe-to-vote or swipe-to-save gestures. Swipes are for navigation only.
- Support for anything older than the current stable GNOME runtime (§3.1).
- Multiple accounts in 1.0 (kept possible in the data model; see §8.3).
- Reddit's OAuth API, API keys, or client IDs (§4.1).
- Distribution: no Flathub, no app stores, no prebuilt binaries or bundles on GitHub Releases, no GNOME Circle application. Users build from source.
- Other networks (Lemmy, PieFed, …): not in scope.

---

## 2. Identity and discoverability

| Item | Value |
|---|---|
| Name | Agora |
| App ID | `io.github.ClickCalickClick.Agora` (matches the GitHub repo) |
| GenericName | Reddit Client |
| Summary (AppStream) | "Browse and discuss Reddit" |
| Desktop `Keywords` | `Reddit;Subreddit;Forum;Social;News;Discussion;` (translatable) |
| Categories | `Network;News;GTK;GNOME;` |
| License | GPL-3.0-or-later (same family as Giara) |
| Metadata license | CC0-1.0 |
| Code of Conduct | GNOME Code of Conduct (`CODE_OF_CONDUCT.md` links to it) |

**Discoverability:** searching "Reddit" in the Activities overview (and GNOME Software, for a local install) must find Agora. This is done through `GenericName`, `Keywords`, and the AppStream `<keywords>` and `<summary>`. The name "Agora" itself must not contain "Reddit" (trademark).

**Trademark note:** the app must not use the Reddit logo or Snoo in its icon, and AppStream must state that Agora is an unofficial client not affiliated with Reddit, Inc.

**Name clash (flagged, low urgency now):** no Flathub app is called Agora, but the "Agora Linux App Store" (agoralinux.app, a Flatpak directory built on Flathub) and Agora.io (a real-time SDK vendor) both exist. It matters little for a self-built app that is never listed in a store; revisit only if distribution plans change.

### 2.1 Visual identity

Agora must be recognisably its own app while looking unmistakably GNOME. The rule: **everything structural comes from libadwaita; everything that makes it "Agora" is a small number of deliberate signature touches.** No custom theming, no custom window chrome, no fixed brand color in the UI.

**Relationship to Giara.** Giara is inspiration for the *idea* only. Agora is written from scratch in Rust, and reuses none of Giara's code, UI files, CSS, icons, artwork, or screenshots. Where a design choice would make Agora look like Giara, pick the alternative. Concretely, Agora differs in:
- **Layout:** three panes (sidebar, list, post) instead of a feed-plus-detail flow, with phone layouts built on `AdwNavigationView`.
- **Feed style:** text-first rows with a small trailing thumbnail by default (large cards are opt-in), rather than an image-led feed.
- **Icon and name:** a new name and an original icon concept (below), with no reference to Giara's icon shape or palette.

**Brand concept: "the public square".** An agora was the open square where people gathered to talk. The identity leans on that: calm, open, conversational, civic rather than loud.

**App icon (concept, to be designed in Phase 6):**
- A small open plaza seen from the GNOME icon perspective: a row of simple columns (a portico) whose negative space forms a speech bubble.
- Palette: a warm stone/sand base with one muted teal accent. Deliberately **not** Reddit orange-red, and no Snoo, alien, or Reddit mark.
- Follows the GNOME app icon guidelines (made in App Icon Preview, tested at 16–128 px and on light and dark backgrounds), plus a matching symbolic icon.
- AppStream `<branding>` uses the sand (light) and deep teal (dark) colors so Software/Flathub pages feel on-brand. The in-app UI still uses the **system accent color**.

**Signature touches (all built from stock libadwaita widgets and style classes):**
1. **Conversation lines.** The comment thread lines (§8.1) are Agora's most visible detail: hairline neutral lines that light up in the accent color along the whole branch on hover or keyboard focus, with a smooth collapse animation.
2. **Quiet rows.** Post rows are typographic: strong title, one muted metadata line, score and comments as small trailing figures. Read posts fade back. The result looks closer to a GNOME mail or news reader than to a social feed.
3. **Community headers.** Each subreddit page opens with a compact header built from its icon, name, and description in a rounded `.card`, with Join as a pill button. Subreddit banner images are shown small and softened, so third-party art never overwhelms the GNOME look.
4. **Calm by default.** No autoplay, no badges counting up in the sidebar except Inbox, no red notification dots. Motion uses libadwaita's standard springs and respects reduced animation.
5. **Friendly empty states.** Custom illustrated `AdwStatusPage` icons in the same "public square" style (an empty plaza for "No posts", a closed gate for "Offline", a mailbox for "Inbox zero").

**Writing voice.** Plain, warm, and brief in all UI text ("Nothing new in your inbox", not "0 unread messages found"). Follows the GNOME HIG writing style (header capitalisation for buttons and titles, sentence case for descriptions).

**What stays strictly stock:** header bars, sidebars, split views, dialogs, preferences, toasts, banners, buttons, typography, and color, so Agora automatically picks up future GNOME visual changes.

---

## 3. Platform and stack

### 3.1 Platform target

- **Target: the current stable GNOME runtime.** As of this spec that is **GNOME 51** (`org.gnome.Platform//51`), **GTK 4.23**, **libadwaita 1.10**. This line is the only place the version is stated; everything else says "the target runtime".
- No compatibility shims for older versions. Bump to each new GNOME runtime within one cycle.
- Newer libadwaita/GTK features the spec relies on: `AdwSidebar` with sections (§5.1), `AdwCssClassBinding` (read-state and vote-state styling), `GtkEnumList` for sort dropdowns, and GTK's built-in reduced-motion handling in Spinner, Stack and Revealer.

### 3.2 Language and core crates

| Concern | Crate | Notes |
|---|---|---|
| UI | `gtk4`, `libadwaita` (gtk-rs) | Feature flags pinned to the target runtime's GTK / libadwaita (§3.1) |
| UI definitions | Blueprint (`.blp`) compiled to `.ui` | Composite templates via `#[derive(CompositeTemplate)]` |
| GObject / async | `glib`, `gio` | UI work runs on the GLib main context |
| Reddit traffic | `webkit6` (WebKitGTK 6.0) | Hidden logged-in WebView, JS bridge via script message handlers; see §4.2 |
| HTTP (media, tests) | `reqwest` (rustls) on a `tokio` runtime | **Decided.** Used for media CDN downloads (§4.6) and test servers, never for reddit.com pages. One `tokio` runtime on a dedicated thread. A single bridge helper `spawn_tokio(fut)` in `agora-app/src/rt.rs` (owned by WP-06) sends results back over a `oneshot` channel awaited with `glib::spawn_future_local`. No tokio types anywhere else in `agora-app`. Keeps library crates testable with `#[tokio::test]` and no display. |
| JSON | `serde`, `serde_json` | Bridge messages and Reddit JSON parsed into `agora-core` models |
| Portals | `ashpd` | Background, OpenURI, Settings |
| D-Bus | `zbus` | GNOME Shell search provider |
| Cache DB | `rusqlite` (bundled SQLite) | Metadata and post bodies |
| Image decoding | `glycin` | Same sandboxed loader stack as Loupe; supports animated formats |
| Markdown / HTML | `pulldown-cmark` plus a Reddit extension layer; `html5ever`/`scraper` for HTML bodies | Spoilers (`>!text!<`, pre-pass), superscript (`^`), `r/` and `u/` autolinks (post-pass). HTML from page extraction maps to the same AST (§4.5) |
| Video | `GtkVideo` / `GtkMediaFile` (GStreamer backend) | Reddit hosted video is DASH/HLS; see §7.4 |
| i18n | `gettext-rs` | |
| Logging | `tracing` | Never logs cookies, CSRF tokens or post content at info level |
| Build | Meson + Cargo | Standard GNOME Rust template layout |

### 3.3 Repository layout (planned)

The code is a **Cargo workspace of small crates** so independent work packages (§22) can be built and tested in isolation by separate agents. Only `agora-app` and `agora-reddit-web` depend on GTK/WebKit; `agora-reddit-web` keeps its parsers in a display-free module so they are testable with plain `cargo test`.

```
agora/
├── Cargo.toml                 # workspace
├── crates/
│   ├── agora-core/            # shared types: IDs, Thing models, errors, traits (contracts)
│   ├── agora-reddit-web/      # Backend impl: hidden WebView session, JS bridge, ops/*.js, parsers, pacing
│   ├── agora-cache/           # SQLite store, image cache, eviction, drafts
│   ├── agora-markdown/        # Reddit Markdown and HTML → document AST (no GTK)
│   └── agora-app/             # GTK/libadwaita app (everything below)
├── meson.build
├── build-aux/io.github.ClickCalickClick.Agora.Devel.json   # Flatpak manifest
├── data/
│   ├── *.desktop.in.in, *.metainfo.xml.in.in, *.gschema.xml.in
│   ├── *.search-provider.ini
│   ├── icons/ (scalable + symbolic)
│   └── resources/ (Blueprint files, style/<module>.css; gresource XML generated at build)
├── po/  (LINGUAS; POTFILES generated at build)
├── crates/agora-app/src/
│   ├── main.rs, application.rs, config.rs.in
│   ├── stores/     # GObject stores (Session, Feed, Thread, Inbox, Prefs)
│   ├── services/   # notifications, background, search provider, url routing
│   ├── render/     # markdown AST → GTK widgets
│   └── ui/         # one module per page/widget (window, sidebar, post_row, …)
└── docs/
    ├── SPEC.md
    └── work-packages/  # one brief per work package (§22), generated from this spec
```

### 3.4 Architecture overview

```
 ┌──────────────── UI (GTK main thread) ─────────────────┐
 │  Window → Pages → Rows/Widgets      (GObject props,   │
 │                                      list models)     │
 └───────────────▲──────────────────────────┬────────────┘
                 │ signals / list models    │ commands
 ┌───────────────┴──────────────────────────▼────────────┐
 │  Stores (GObject): Session, Feed, Thread, Inbox, Prefs│
 └───────────────▲──────────────────────────┬────────────┘
                 │                          │
 ┌───────────────┴────────┐   ┌─────────────▼────────────┐
 │  Cache (SQLite + files)│◄──┤  Backend trait           │
 └────────────────────────┘   │  RedditWebBackend: pacing│
                              │  · JS bridge · parsers   │
                              └─────────────┬────────────┘
                                            ▼
                         hidden WebKit view on www.reddit.com
```

- **Stores** are GObjects that own state and expose `gio::ListModel`s, so views bind to them directly (`GtkListView` + `GtkSignalListItemFactory`). Views never call the API.
- **Backend** is reached only through the `Backend` trait (§4.3); parsers are unit-tested with recorded fixtures.
- **Cache** sits between stores and the backend: reads return cached data first, then refresh.

---

## 4. Data access: the browser-session backend

### 4.1 Why a browser session

Reddit has closed self-service API registration (November 2025), stopped accepting new public API requests (2026-10-31), and is closing the public Data API for everyone in March 2027 (see `SPEC-REVIEW.md` §1). Agora therefore does **not** use the OAuth API or a client ID. Instead it drives Jared's own logged-in reddit.com session through a hidden web engine, so every request Reddit sees is real browser traffic from a real browser, and Agora only re-presents the results natively.

Agora is **public source, self-built, for personal use** (§1, §18). It works the way a user script or browser extension does, and it carries the same risks:
- **Fragility:** reddit.com's private endpoints and page markup change without notice. Expect to update the extractors (§4.5) regularly. The design isolates all Reddit-specific knowledge in one crate and a set of versioned scripts so a breakage is a small, local fix.
- **Account risk:** automated use of the website is against Reddit's user agreement. Pacing traffic like a person (§4.6) keeps the footprint small, but the risk to the account is Jared's to accept.

### 4.2 Architecture

```
 Agora UI (GTK)                                   hidden WebKitGTK WebView
 ───────────────                                  ─────────────────────────
 Stores ──► Backend trait ──► RedditWebBackend ──► run_javascript("agora.fetchFeed(...)")
                                   ▲                          │  same-origin fetch / DOM read
                                   │                          ▼
                                   └── parse JSON ◄── script message handler "agora"
                                       into agora-core models   (postMessage({id, ok, data}))
```

- **Engine:** WebKitGTK 6.0 via the `webkit6` crate (GTK 4 API), which the GNOME runtime already ships.
- **Session:** one `webkit6::NetworkSession` with a persistent data directory (`$XDG_DATA_HOME/agora/web/`). Cookies, local storage, and the login live there, exactly as in a normal browser profile. Agora never reads or stores the password; it never copies cookies out of WebKit.
- **Hidden view:** a `WebView` that is created but never shown, kept on `https://www.reddit.com/` so all calls are same-origin. It is a normal desktop WebKit, so its User-Agent, TLS fingerprint, cookies and headers are a real browser's. Agora does not override the User-Agent.
- **Bridge:** Agora injects one user script (`agora-bridge.js`, bundled in the gresource) via `UserContentManager`. It exposes `window.agora.<operation>(requestId, args)` functions. Each finishes by calling `window.webkit.messageHandlers.agora.postMessage({id, ok, data | error})`. Rust registers the `agora` handler, matches replies to pending requests by `id`, and resolves the corresponding future. `run_javascript` is used only to kick off an operation, never to return large data.
- **Isolation:** the bridge runs in an isolated `ScriptWorld` so page scripts cannot call or observe it.
- **Threading:** WebKit lives on the GTK main thread. The backend is a GObject-free Rust type that holds a handle to the view and exposes `async fn`s; no tokio needed for Reddit traffic (§3.2 tokio remains for media downloads and tests).

### 4.3 Pluggable backend

The UI never talks to Reddit types directly. `agora-core` defines:

- `trait Backend` — async methods for listings, post + comments, more comments, vote, save, hide, report, submit, comment, edit, delete, inbox, mark read, compose message, search, subscribe, subreddit info, user profile.
- `struct Capabilities` — flags such as `downvotes`, `private_messages`, `flair`, `polls`, `nsfw`, `media_upload`, `search_scope_community`. The UI hides controls a backend lacks.
- Neutral naming in the trait (`Community`, `community_name`); Reddit-specific wording ("subreddit", "r/") stays in UI strings.

`agora-reddit-web` is the only implementation. Other backends are out of scope, but the trait stays free of Reddit types so the UI never depends on the backend crate.

### 4.4 Sign-in and session

- **Sign in** opens a visible `WebView` in an `AdwDialog` on `https://www.reddit.com/login` using the same `NetworkSession`. Jared logs in with Reddit's own page (password, 2FA, SSO, captchas all handled by Reddit). Agora detects success when the bridge's `agora.whoami()` returns a username, then closes the dialog.
- **Session expiry:** if any operation returns "logged out", the backend emits `session-expired`; the UI shows an `AdwBanner` ("Signed out of Reddit · Sign In") and stops issuing requests.
- **Sign out** clears the `NetworkSession` website data and the personal cache.
- No logged-out browsing: Reddit is moving to logged-in-only, and signing in is the first-run step (§6).

### 4.5 Extraction strategy

Every operation is a small, versioned JavaScript function in `crates/agora-reddit-web/js/ops/<operation>.js`, paired with a Rust parser in `src/parse/<operation>.rs`. Each operation tries strategies in order and reports which one it used:

1. **Same-origin JSON** — `fetch()` with `credentials: "include"` against the JSON endpoints the logged-in site still serves (for example `/r/{sub}/hot.json`, `/comments/{id}.json`, `/api/me.json`). Preferred: gives Markdown bodies and full metadata.
2. **Site-internal endpoints** — the endpoints reddit.com's own front-end calls (for example HTML partials under `/svc/shreddit/…`), parsed from the returned markup inside the page with `DOMParser`.
3. **Page DOM** — navigate the hidden view to the normal page and read Reddit's web components (`shreddit-post`, `shreddit-comment` attributes and slots).

Writes (vote, save, comment, submit, message) reuse the site's own requests with the page's CSRF token, or, as a last resort, operate the page's own controls in the hidden view. Every write is followed by a read-back to confirm it took effect; a mismatch becomes an error toast, never a silent success.

Which strategy works for which operation must be established in WP-02's first week against Jared's live session and recorded in `crates/agora-reddit-web/STRATEGIES.md`.

**Self-test:** a hidden "Check Reddit connection" action in Preferences (and `agora --self-test`) runs every read operation once and reports which ones broke, so a Reddit change is diagnosed in seconds.

**Body format:** strategy 1 yields Markdown; strategies 2–3 yield HTML. The renderer accepts both: `agora-markdown` gets an `html_to_ast` path (sanitised subset: p, a, em, strong, del, code, pre, blockquote, ul/ol/li, table, sup, spoiler spans, img) alongside the Markdown parser.

### 4.6 Human-paced traffic

The goal is that Agora's traffic is indistinguishable in volume and rhythm from Jared browsing reddit.com in a tab.

- **One request at a time** through the hidden view, with a minimum gap of ~1 s between requests plus small random jitter, and short bursts allowed only right after a user action (open a post → post + comments).
- **No prefetch:** load the next page of a feed only when the user scrolls near the end; never load comments for posts the user hasn't opened; never pre-open links.
- **No bulk caching:** the cache (§14) stores only what the user actually viewed.
- **Inbox checks:** every 15 minutes while the window is open; background checking is **off by default** and, when enabled, polls no more than every 30 minutes and pauses overnight-length idle (no checks after 12 h without the user opening the app).
- **Respect pushback:** on HTTP 429, a captcha/interstitial page, or any unexpected redirect to login, stop all traffic, show a banner, and back off exponentially (starting at 5 minutes). Never retry automatically in a tight loop.
- **Media:** images and video load from Reddit's public media CDNs (`i.redd.it`, `preview.redd.it`, `v.redd.it`, `external-preview.redd.it`) only when on screen, as a browser would. They are fetched with the same User-Agent string the WebView reports and no cookies. Avatars and thumbnails are lazy-loaded the same way.

### 4.7 Networking and errors

- Network state comes from `gio::NetworkMonitor`. Offline mode shows an `AdwBanner` ("You're offline — showing saved content").
- Errors surface as `AdwToast`s with a Retry action, never modal dialogs. `AgoraError` gains `SessionExpired`, `Blocked` (captcha/429), and `ExtractorBroken { operation, strategy }`.

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

Medium (≈ 700–1100 sp): sidebar collapses into an overlay; list + post view stay side by side.
Narrow (< 700 sp, phones): single pane, navigation stack: Sidebar → List → Post.
```

- Implemented with an outer `AdwOverlaySplitView` (sidebar) wrapping an `AdwNavigationSplitView` (list / content), driven by `AdwBreakpoint`s. Exact breakpoints are tuned in Phase 2.
- Sidebar is an `AdwSidebar` with sections (stock HIG look, keyboard handling and overlay style): Feeds (Home, Popular), You (Saved, Inbox, Profile), Subscriptions (alphabetical, favorites pinned on top). There is no "All" feed: Reddit removed r/all in April 2025.
- Each pane has its own `AdwHeaderBar`. The sort dropdown (Best/Hot/New/Top/Rising, Top with time range) lives in the list pane's header bar.
- Back navigation: `AdwNavigationView` provides the back button, Alt+←, mouse back button, and the edge-swipe gesture on touch.

### 5.2 Main menu

Primary menu (`open-menu-symbolic`) in the sidebar header: Preferences, Keyboard Shortcuts, About Agora. Account avatar button opens a popover: profile, Sign Out.

### 5.3 Screens

| Screen | Widget base | Contents |
|---|---|---|
| Welcome tour | `AdwDialog` with `AdwCarousel` | §6 |
| Feed / post list | `GtkListView` in `GtkScrolledWindow` | Post rows (§7.1), pull-to-refresh, infinite scroll |
| Post view | `AdwNavigationPage` | Header, media, body, action bar, comment tree |
| Subreddit header | Banner area above the list | Icon, name, description, member count, Join/Leave, rules (opens dialog) |
| User profile | `AdwNavigationPage` | Avatar, karma, cake day; tabs: Posts, Comments, Saved (own profile only) via `AdwViewSwitcher` |
| Inbox | `AdwNavigationPage` | Tabs: All, Replies, Mentions, Messages; unread rows bold; Mark All Read |
| Composer | `AdwDialog` (sheet on narrow) | §9 |
| Search | `GtkSearchBar` in list pane | §10 |
| Image viewer | Fullscreen overlay page | §7.3 |
| Preferences | `AdwPreferencesDialog` | §13 |
| About | `AdwAboutDialog` | Credits Giara, links to repo, CoC, translators |
| Shortcuts | `AdwShortcutsDialog` | §11 |

### 5.4 Empty, loading, and error states

Every list has an `AdwStatusPage` for empty ("No posts here yet"), signed-out ("Sign in to see your inbox"), offline, and error states. Loading uses `AdwSpinner` placeholders, never a blocking dialog.

---

## 6. Onboarding (first launch)

A short welcome in an `AdwDialog` with an `AdwCarousel`:

1. **Welcome:** app icon, one-sentence pitch, the short disclaimer from §18 (unofficial, not affiliated with Reddit, personal use, may break, use at your own risk) with a link to the full text in About.
2. **Sign in to Reddit:** a "Sign In" button opens Reddit's real login page in an embedded web view (§4.4). Nothing to register, no client ID.
3. **Done:** quick tips (keyboard shortcuts, Preferences).

If the dialog is closed before signing in, the main window shows an `AdwStatusPage` "Sign in to Reddit" with a Sign In button. Sign In / Sign Out also live in the account popover and Preferences.

---

## 7. Reading experience

### 7.1 Post rows (list)

Text-first rows, built as a custom `GtkListBoxRow`-style widget in a `GtkListView`:

- Title (wraps to 3 lines, `.heading` weight), then a subtitle line: `r/sub · u/author · 3h · flair`.
- Small rounded thumbnail (64×64 logical, requested at 2× for HiDPI, `border-radius: 8px`) on the trailing edge when the post has media.
- Trailing column: score and comment count. Vote state shown by color of the score (accent for up, a muted secondary color for down).
- Badges as small pills: NSFW, Spoiler, Pinned, OC, Locked.
- Read posts dim their title (`.dim-label`).
- **Large card previews** (Preferences toggle): rows become cards with full-width media above the title.

Interactions:
- Click or Enter opens the post. Middle-click / Ctrl+Enter is not needed (single window).
- Right-click or long-press opens a context menu: Upvote, Downvote, Save, Hide, Share, Copy Link, Open in Browser, Open Subreddit, View Author, Mute Subreddit, Report.
- Pull to refresh on touch (`GtkScrolledWindow` edge overshoot + spinner); F5 / Ctrl+R on keyboard.
- Infinite scroll: next page loads when 10 rows from the end.

### 7.2 Post view

- Header: subreddit (link), author (link), age, flair, edited marker.
- Media (§7.3–7.4), link card for link posts (domain, title, favicon; opens in browser), or body rendered from Markdown (§7.6).
- Action bar: Upvote / score / Downvote, Comments count, Save, Share, overflow menu (Copy Link, Open in Browser, Hide, Report, Edit/Delete for own posts).
- Comment sort dropdown (Best, Top, New, Controversial, Old, Q&A).
- Comment tree (§8).
- Content zoom: Ctrl++ / Ctrl+- / Ctrl+0 scale post and comment text (persisted in GSettings, 80%–200%). UI chrome is not scaled; it follows the system text size.

### 7.3 Images and galleries

- Inline in post view at fitting width; galleries use an `AdwCarousel` with dots and arrow buttons.
- Tap/click opens a **Loupe-style fullscreen viewer**: black background, pinch and Ctrl+scroll zoom, pan, double-tap to zoom, swipe left/right between gallery items, swipe down or Esc to close. Toolbar: Save As (file chooser portal), Copy, Share, Open in Browser.
- Images decoded with `glycin` (sandboxed, supports animated WebP/GIF).

### 7.4 Video

- Inline `GtkVideo` in post view. Tap to play; **no autoplay**. Controls: play/pause, seek, volume, fullscreen.
- Reddit-hosted video (`v.redd.it`) is delivered as DASH/HLS with separate audio. Use the HLS playlist through GStreamer's `hlsdemux`; the GNOME 51 runtime ships `hlsdemux2` (gst-plugins-good) and DASH support (gst-plugins-bad). Set `souphttpsrc`'s `user-agent` to the WebView's string (§4.6); without a browser User-Agent the CDN returns 403. **Fallback:** play the highest `fallback_url` MP4 (video only) with a "No audio" note, plus Open in Browser.
- External video (YouTube etc.) shows a link card that opens in the browser.
- **Short silent GIFs/GIFVs** (no audio track, under ~30 s) loop automatically in the post view, **unless** the system "reduce animation" setting is on (`gtk-enable-animations` false), in which case they show a play button. They never autoplay in the list.

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
- Images embedded in text (Reddit image links) → inline pictures.

---

## 8. Comments

### 8.1 Tree

- Comments render as a flattened `GtkListView` of rows with a depth value (keeps virtualization for threads with thousands of comments).
- Each depth draws an indent of 12 px plus a **thread line**: 2 px, neutral gray (`alpha(currentColor, 0.15)`), which turns the **accent color on hover** for that comment's whole branch.
- Clicking a thread line or the comment header collapses the branch with an animated height transition (respecting reduced animation). Collapsed state shows "author · N replies".
- Max visual depth 8; deeper replies show "Continue thread →" which pushes a new page rooted at that comment.
- **Load more**: "N more replies" rows load inline via `/api/morechildren` and replace themselves with the results.
- OP's comments get an "OP" badge; the user's own comments get a "You" badge.

### 8.2 Comment actions

Upvote, Downvote, Reply, Save, Share/Copy Link, and for own comments Edit and Delete. Available through a hover/focus action row on desktop and a long-press menu on touch.

### 8.3 Data model note

Accounts are stored as a list with one entry in 1.0 so multi-account can be added later without migrating storage.

---

## 9. Composer

One composer used for new posts, replies, edits, and messages; opened as an `AdwDialog` (fills the screen on narrow).

- **Post types:** Text, Link, Image/Video (upload via file chooser portal), Poll is out of scope for 1.0.
- Subreddit picker with search; shows the subreddit's posting requirements and rules (`post_requirements`), required flair picker, NSFW/Spoiler toggles.
- **Editor:** `GtkSourceView` 5 with Markdown syntax highlighting, spell check (`libspelling`), and a **formatting toolbar**: Bold, Italic, Strikethrough, Link, Quote, Code, Spoiler, Bulleted list, Numbered list, Heading.
- **Live preview toggle** (`AdwToggleGroup`: Write / Preview) using the same renderer as §7.6. On wide windows, side-by-side preview is offered.
- **Auto-saved drafts:** saved to SQLite every few seconds and on close, keyed by context (new post in r/x, reply to comment y). Reopening the same context restores the draft. A Drafts list lives in the sidebar under "You".
- Closing with unsent text asks: Discard / Save Draft (`AdwAlertDialog`).
- Character counters for titles (300 max).

---

## 10. Search

- `Ctrl+F` or the search button opens a `GtkSearchBar` in the post list pane.
- **Scope:** when inside a subreddit, search is scoped to it by default, with a toggle "Search all of Reddit". On Home/Popular it searches everything.
- Results tabs: Posts, Subreddits, Users. Sort and time-range options in a dropdown.
- The sidebar search entry filters subscriptions instantly and offers "Go to r/…" for any typed name.
- **GNOME Shell search provider:** registered via `*.search-provider.ini` and the `org.gnome.Shell.SearchProvider2` D-Bus interface (zbus). Returns the user's subscribed subreddits that match first, then cached popular subreddits; activating a result opens Agora on that subreddit. Never hits the network for every keystroke; uses the cache.

---

## 11. Keyboard

Full keyboard operation; every action reachable without a pointer. Shortcuts listed in an `AdwShortcutsDialog` (Ctrl+?).

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

**Open question for review.** The memory decision was "register as reddit.com URL handler, user opts in". On Linux, apps cannot claim handling of `https://` links for one domain; `x-scheme-handler/https` would make Agora the handler for *all* web links, which is wrong. Realistic options:
1. Accept `reddit.com` URLs as command-line arguments / `GApplication` open-files, so "Open With → Agora" works for `.desktop`/URL files and `agora https://reddit.com/...` works from a terminal.
2. "Open Link from Clipboard" action (Ctrl+L) that detects a Reddit URL on the clipboard.
3. Accept dropped URLs on the window.
Recommendation: ship 1–3 and do not register a global https handler.

### 12.3 Sharing

**Open question for review.** There is currently no xdg-desktop-portal "Share" portal shipped in GNOME. Plan: Share menu offers Copy Link (with toast), Open in Browser, and Send by Email (`mailto:` via OpenURI). Adopt a share portal if one lands in GNOME later.

### 12.4 Notifications and background

- Desktop notifications for new inbox replies, username mentions, and messages, sent with `GNotification` (portal-backed in Flatpak). Clicking a notification opens the item. Grouped when several arrive at once.
- Notifications never quote content that has since been deleted (§14).
- **Inbox checks** follow the human-pace rules in §4.6: every 15 minutes while the window is open.
- **Background checking** is **off by default**. When enabled in Preferences, Agora requests the **Background portal** (with autostart), keeps the hidden web view alive after the window closes, and checks unread messages every 30 or 60 minutes (no faster option). It pauses after 12 hours without the app being opened and stops on any captcha, 429 or sign-out.
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
**Content:** Show NSFW content, blur NSFW media, blur spoilers, autoplay silent GIFs, muted subreddits (list editor), muted keywords (list editor; matched case-insensitively against titles and optionally comment text).
**Notifications:** Enable notifications, background checking (off by default; 30 or 60 min), which types (replies, mentions, messages).
**Storage:** cache size limit (100 MB / 250 MB / 500 MB / 1 GB), current usage, **Clear Cache** button.
**Account:** signed-in user, Sign Out, "Check Reddit connection" self-test (§4.5).

All persisted in GSettings (`io.github.ClickCalickClick.Agora.gschema.xml`).

---

## 14. Offline cache

- **What:** only what the user actually viewed: feed pages they scrolled through, posts they opened with the comments they loaded, thumbnails and images that were on screen. Saved posts are cached when the user opens the Saved view, never by a bulk sync (§4.6).
- **Where:** `$XDG_CACHE_HOME/agora/` (SQLite DB `cache.db` + `images/` content-addressed by URL hash). Drafts and preferences are not cache; drafts live in `$XDG_DATA_HOME/agora/agora.db`.
- **Policy:** LRU eviction when over the size limit; saved posts are evicted last. Entries carry `fetched_at`; UI shows cached content immediately and refreshes the one view on screen when online (no background refresh of other views).
- **Offline UX:** banner, read-only actions; votes/saves made offline are **not** queued in 1.0 (they show "You're offline" toast). Decided: offline stays read-only.
- **Deleted and removed content is dropped.** When a refresh shows a post or comment as deleted by its author or removed (`[deleted]`/`[removed]`, missing author, or a 404), Agora purges its cached copy, body, media and thumbnails, and shows the same placeholder reddit.com shows. Agora never displays or keeps a cached copy of content that is gone on Reddit, and has no "show deleted" feature. Inbox items that were deleted are removed the same way.
- **Entries expire:** anything not viewed for 30 days is evicted regardless of size, so stale copies don't linger.
- Sign out clears personal cache (inbox, saved, drafts).

---

## 15. Filters

- **NSFW:** §7.5.
- **Muted subreddits:** their posts are removed from Home and Popular; visiting the subreddit directly still works with a banner "You muted this subreddit · Unmute".
- **Muted keywords:** posts whose titles match are hidden from feeds. A collapsed "N posts hidden by filters" row appears at the end of a page so filtering is never silent.
- Filtering is local only (not synced to Reddit).

---

## 16. Accessibility (from day one)

- Every custom widget sets an accessible role, label, and description (`gtk::Accessible`). Post rows announce: title, subreddit, author, age, score, comment count, vote state, badges.
- Comment rows announce depth ("Level 3 reply by …") and collapsed state; expose `GTK_ACCESSIBLE_ROLE_TREE_ITEM` semantics where possible.
- Vote buttons are toggle buttons with pressed state, not color only.
- Focus order follows reading order; focus is visible; nothing is reachable by hover only (comment actions also appear on focus).
- All images have alt text: Reddit captions when present, otherwise "Image from post: <title>".
- Contrast: rely on libadwaita colors; no custom colors below WCAG AA.
- Testing: Orca walkthrough of every screen each phase; keyboard-only walkthrough each phase; `accerciser` checks for unlabeled widgets in CI review.

---

## 17. Internationalization

- All user-facing strings through gettext; plurals via `ngettext`.
- Dates and numbers via GLib (`GDateTime` formatting, locale-aware "3 hours ago").
- RTL layouts tested (Hebrew/Arabic pseudolocale).
- **Platform:** GNOME's Damned Lies requires hosting on GNOME GitLab. Since the repo is on GitHub, use **Weblate** (hosted.weblate.org libre plan). English plus gettext plumbing is enough for now; set up Weblate only if contributors ask to translate.

---

## 18. Disclaimers, legal hygiene and HIG checklist

Agora is public source on GitHub but is **not distributed**: no Flathub, no app stores, no prebuilt binaries. Flathub and GNOME Circle are not goals.

### Disclaimer requirements

These are requirements, not suggestions. `DISCLAIMER.md` and the README text exist from the start; WP-00 wires them into the app; every later WP keeps them intact.

1. **Disclaimer text.** One canonical text, `DISCLAIMER.md` at the repo root, reused verbatim (translatable) in the app:
   > Agora is an unofficial, independent project. It is not affiliated with, endorsed by, or sponsored by Reddit, Inc. "Reddit" is a trademark of Reddit, Inc. and is used only to describe what this app works with.
   > Agora is for personal use. It shows Reddit through your own reddit.com login, inside an embedded web browser. Automated use of reddit.com may violate Reddit's User Agreement; you are responsible for how you use it and for your account.
   > Agora depends on reddit.com's website and may stop working at any time.
   > The software is provided "as is", without warranty of any kind, as stated in the GPL-3.0 license. Use it at your own risk.
2. **README:** the disclaimer appears near the top, before any build instructions.
3. **About dialog:** the full disclaimer in a "Disclaimer" section of `AdwAboutDialog` (via `add_legal_section`), plus the GPL license. The onboarding Welcome page shows the short form (§6).
4. **No Reddit marks.** No Reddit logo, Snoo, wordmark, colors or screenshots of Reddit's own UI in the icon, README, metainfo or repo artwork. "Reddit" appears only in plain text, descriptively ("a client for Reddit"), never as part of the app name.
5. **No Reddit app credentials, ever.** The repo and the app contain no Reddit OAuth client IDs or secrets, no API keys, and no credentials extracted from Reddit's own web or mobile apps. The app only uses the cookies WebKit holds for the user's own login. CI runs a secrets scan (`gitleaks`) on every push.
6. **No user data in the repo.** Fixtures are scrubbed of usernames, IDs that identify the user, tokens and cookies before they are committed (§22.1 rule 6). Fixture content written by other Reddit users is kept to the minimum needed and anonymised.
7. **No circumvention.** Agora never bypasses captchas, rate limits, logins, paywalls, bans or blocks. When Reddit pushes back, it stops (§4.6).
8. **License:** GPL-3.0-or-later, whose warranty disclaimer and limitation of liability (sections 15–16) apply. `LICENSE` is added in WP-00.

Disclaimers lower the risk but do not guarantee that no one is held responsible. If that matters, get advice from a lawyer before inviting others to use the app.

### HIG checklist

| Requirement | How Agora meets it |
|---|---|
| Follows the GNOME HIG | libadwaita widgets throughout, adaptive layout, `AdwToolbarView` (raised top bar on scrolled pages), no custom window chrome |
| Window state | Size and maximised state persisted in GSettings |
| About | `AdwAboutDialog::from_appdata` with release notes, Giara credit, and the disclaimer section |
| App icon | Original icon per §2.1, symbolic icon designed first; no Reddit marks |
| AppStream metadata | Valid (`appstreamcli validate`) and states the disclaimer in its description |
| Translatable | gettext throughout |
| Accessible | §16 |
| Adaptive / mobile | §5, tested at 360×640; desktop file has `X-Purism-FormFactor=Workstation;Mobile;` |
| Privacy | No telemetry; network traffic only through the hidden web view to reddit.com and to Reddit's media CDNs |

### 18.1 Flatpak manifest

`build-aux/io.github.ClickCalickClick.Agora.Devel.json`, built by each user with `flatpak-builder --user --install` (no bundles are published):
- Runtime/SDK `org.gnome.Platform` / `org.gnome.Sdk` at the target version (§3.1), plus `org.freedesktop.Sdk.Extension.rust-stable`.
- **WebKitGTK 6.0** (`webkitgtk-6.0`) comes from the GNOME runtime; no extra module. WP-00 adds a CI check that `pkg-config --modversion webkitgtk-6.0` succeeds in the SDK, and adds `libspelling` as a module only if the SDK lacks it.
- Finish args: `--share=network`, `--share=ipc`, `--socket=wayland`, `--socket=fallback-x11`, `--device=dri`, `--socket=pulseaudio` (video audio). No filesystem access (portals for files). Background, OpenURI, FileChooser and Notification go through portals. No `org.freedesktop.secrets` talk-name (no secrets are stored outside WebKit's own profile).
- WebKit's web process sandbox runs inside the Flatpak sandbox as in GNOME Web; no extra permissions needed.

---

## 19. Testing and quality

- **Unit tests:** every `agora-reddit-web` parser against recorded fixtures (JSON, HTML partials and page DOM snapshots; never live Reddit in CI); Markdown and HTML → AST; URL router; filter matching; cache eviction; pacing scheduler (time-mocked).
- **Bridge integration:** a local test server serves saved reddit.com pages to a real hidden WebView under `xvfb-run`, so `ops/*.js` + parsers are tested end to end without touching Reddit.
- **Live check:** `agora --self-test` (§4.5) against Jared's session, run by hand, never in CI.
- **UI:** manual test script per phase (desktop, narrow 360 px, touch, keyboard-only, Orca).
- **CI (GitHub Actions):** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, Flatpak build with the target GNOME SDK, `GTK_A11Y=test` run to catch unlabeled widgets, `cargo deny`, `appstreamcli validate`, `desktop-file-validate`, `glib-compile-schemas --strict`.
- Rust edition 2024; MSRV pinned to the Rust SDK extension of the target runtime.

---

## 20. Phased roadmap

Each phase ends with a usable build and the accessibility + keyboard walkthrough. Phases describe *what the app can do* at each milestone; §22 splits the same work into work packages for parallel agents (Phase 0 ≈ wave 0, Phases 1–2 ≈ waves 1–2, Phases 3–5 ≈ waves 2–3, Phase 6 ≈ wave 4).

| Phase | Name | Delivers | Exit criteria |
|---|---|---|---|
| 0 | Foundations | Meson + Cargo scaffold, Flatpak manifest, CI, app ID, GSettings, i18n plumbing, empty `AdwApplicationWindow`, About dialog, CoC/README | CI green; Flatpak installs and launches |
| 1 | Session & backend core | Welcome dialog, embedded login, persistent WebKit session, JS bridge, pacing scheduler, `Backend` trait + `RedditWebBackend` read operations, fixtures, self-test | Sign in/out works; username shown in avatar popover; feeds load through the hidden view |
| 2 | Read-only browsing | Three-pane adaptive layout, sidebar with subscriptions, feeds + sorting, post rows with thumbnails, post view with text/link posts, comment tree with collapse + load more, Markdown renderer, empty/error states, keyboard nav j/k | Daily-driver for reading text posts on desktop and phone |
| 3 | Media | Image viewer, galleries, inline video, GIF looping rules, NSFW/spoiler blur, large card previews, image cache | All common post types display correctly |
| 4 | Interaction | Voting, saving, hiding, reporting, subscribe/unsubscribe, composer (all modes) with toolbar, preview, drafts, edit/delete, user profiles | Can post, reply, and edit end to end |
| 5 | Inbox & desktop integration | Inbox, messages, notifications, opt-in Background portal checking, Shell search provider, URL routing + open-from-clipboard, share menu, search with scope toggle | All integrations working inside Flatpak |
| 6 | Polish & release | Preferences complete, filters, offline cache with limits + Clear Cache, content zoom, full shortcuts dialog, icon, screenshots, AppStream, Orca audit | 1.0 tagged; builds from a fresh clone |
| Later | | Multi-account, offline action queue, polls, share portal if available | |

---

## 21. Decisions and remaining questions

Settled after the 2026-10-31 review (`SPEC-REVIEW.md`):
1. **Data access:** no Reddit API; a logged-in WebKit session with a JS bridge (§4), behind a clean `Backend` trait. Reddit only.
2. **HTTP:** `reqwest` + `tokio` on one thread, for media and tests only (§3.2).
3. **External reddit.com links:** no global `https` handler. `GApplication` open-URI (desktop file `%U`), Ctrl+L open-from-clipboard, drag-and-drop (§12.2).
4. **Sharing:** Copy Link / Open in Browser / Send by Email menu; no Share portal exists in GNOME 51 (§12.3).
5. **Hosting:** public source on GitHub; no Weblate for now (§17).
6. **Offline:** read-only (§14).
7. **App ID:** `io.github.ClickCalickClick.Agora` (§2).
8. **Distribution:** none. Public source, self-built; Flathub, stores, binaries and Circle are out (§1, §18).
9. **Disclaimers:** README, About dialog and onboarding, from one `DISCLAIMER.md` (§18).

Still open:
- **Extraction strategies:** which of §4.5's three strategies works for each operation against the live site. Answered by WP-02 in `STRATEGIES.md` before wave 2.
- **Name clash:** low priority while nothing is listed in a store (§2).

---

## 22. Work breakdown for parallel agents

Development is planned to be split across multiple Opus 5.5 sub-agents, with a lead agent coordinating. This section makes each piece of the spec a self-contained **work package (WP)** an agent can take without reading the whole conversation history.

### 22.1 Rules for every agent

1. **Read first:** this spec's §1, §2.1, §3, the WP's own row below, and the spec sections the WP cites. Nothing else is required context.
2. **Stay in your lane:** an agent edits only the paths its WP owns. Changes needed elsewhere go back to the lead as a request, never as a drive-by edit.
3. **Contracts before code:** shared types and traits live in `agora-core` and are frozen by WP-01 before parallel work starts. Changing a contract needs the lead's sign-off and a note in `docs/CONTRACTS-CHANGELOG.md`.
4. **One branch and one PR per WP**, named `wp-NN-short-name`. The PR description lists the acceptance criteria with checkboxes.
5. **Definition of done (all WPs):** `cargo fmt`, `cargo clippy -D warnings`, and `cargo test` pass for the touched crates; public items are documented; user-facing strings are translatable; UI WPs include the keyboard and screen-reader checks in §16 and a screenshot at 360 px and 1280 px widths, taken under `xvfb-run` (use `ADW_DEBUG_ADAPTIVE_PREVIEW=1` for the narrow shot). If no display is available, the keyboard walkthrough checklist replaces the screenshots.
6. **Fixtures, never live Reddit:** agents never send traffic to reddit.com. They use fixtures in `crates/agora-reddit-web/tests/fixtures/` (JSON, HTML partials, DOM snapshots). Fixtures are recorded only by the lead from Jared's session, with usernames and tokens scrubbed, and committed for agents to use.
7. **No hand-edited shared lists:** WP-00 makes Meson generate the gresource XML and `po/POTFILES` from directory globs, and concatenates `data/resources/style/<module>.css` at build time. WP-06 pre-creates every `ui/<module>/mod.rs` stub named in §22.3. Later packages only add files inside the paths they own.
8. **UI built against mocks:** UI WPs bind to stores fed by an in-memory `MockBackend` (implements the `Backend` trait from WP-01), so UI work never waits on the network layer.

### 22.2 Contracts (frozen by WP-01)

`agora-core` defines, at minimum:
- IDs: `PostId`, `CommentId`, `SubredditName`, `Username`, `Fullname` (`t3_…`).
- Models: `Post`, `Comment`, `MoreComments`, `Subreddit`, `User`, `Message`, `Listing<T>`, `Media` (Image, Gallery, Video, Gif, Link, None), `Vote`, `Sort`, `TimeRange`.
- Traits: `Backend` (async: listings, comments, more-children, vote, save, submit, comment, inbox, search, subscribe…) with `Capabilities` (§4.3), `Cache` (get/put/evict, drafts).
- `AgoraError` enum (Network, SessionExpired, Blocked { retry_after }, NotFound, Forbidden, ExtractorBroken { operation, strategy }, Parse, Io).
- Markdown AST types (`Block`, `Inline`) consumed by the renderer.

Also frozen as contracts (not in `agora-core`, but changed only with the lead's sign-off):
- **GSettings schema:** WP-00 ships the *complete* `io.github.ClickCalickClick.Agora.gschema.xml` with every key from §13 (plus window size and maximised state), not stubs. No other WP edits it.
- **Stores:** WP-07 ships `docs/STORES.md` listing each store's GObject properties, signals, actions and `ListModel` item types. Frozen at the end of wave 1; wave-2 UI packages bind only to what it lists.
- **Navigation:** window actions `win.open-post(s)`, `win.open-subreddit(s)`, `win.open-user(s)`, `win.open-url(s)`, `win.continue-thread(s)`, defined by WP-06 and listed in `docs/STORES.md`, so no UI package needs the window type.
- **Cross-package widgets:** `MediaView::new(&Media)` (built by WP-13, mounted by WP-11) and `ShareMenu::for_url(&str)` (built by WP-18, used by WP-09/11/12).

### 22.3 Work packages

`Deps` lists WPs that must be merged first. WPs in the same wave can run in parallel.

| WP | Wave | Name | Owns (paths) | Spec | Deps | Acceptance (summary) |
|---|---|---|---|---|---|---|
| 00 | 0 | Scaffold & CI | root `Cargo.toml`, `meson.build`, `build-aux/`, `data/` (desktop, metainfo, complete gschema), `po/`, `.github/`, `LICENSE`, `DISCLAIMER.md` | §2, §3, §13, §18, §19 | — | Flatpak builds and launches an empty `AdwApplicationWindow`; disclaimer in About; complete gschema frozen; gresource/POTFILES/CSS generated from globs; CI runs fmt, clippy, test, gitleaks, appstream/desktop validation |
| 01 | 0 | Core contracts | `crates/agora-core/` | §3.4, §22.2 | 00 | All types/traits in §22.2 compile with docs; serde round-trip tests; `MockBackend` with sample data and capability toggles |
| 02 | 1 | Reddit extractors | `crates/agora-reddit-web/js/ops/`, `src/parse/`, `src/backend.rs`, `STRATEGIES.md`, `tests/` | §4.3, §4.5, §4.7 | 01, 03 (bridge API only) | `RedditWebBackend` implements `Backend`; one `ops/*.js` + parser per operation with strategy fallback; writes confirmed by read-back; every parser tested on fixtures; `STRATEGIES.md` filled in from the lead's live run; `--self-test` |
| 03 | 1 | Web session & bridge | `crates/agora-reddit-web/src/session/`, `src/bridge/`, `src/pacing/`, `js/agora-bridge.js` | §4.2, §4.4, §4.6 | 01 | Persistent `NetworkSession`; hidden view lifecycle; isolated script world; `postMessage` request/response with ids and timeouts; login detection, sign-out, session-expired signal; pacing scheduler with jitter, burst and backoff rules (time-mocked tests); bridge tested under `xvfb-run` against a local page |
| 04 | 1 | Cache & drafts | `crates/agora-cache/` | §14, §9 (drafts) | 01 | SQLite schema + migrations; LRU eviction with saved-last rule; image cache; drafts CRUD; size reporting; clear cache |
| 05 | 1 | Markdown & HTML parser | `crates/agora-markdown/` | §4.5, §7.6 | 01 | Markdown with Reddit extensions (spoiler pre-pass, superscript, r/ u/ autolinks post-pass, tables) and sanitised HTML → same AST; property tests; corpus of 50 real-world samples recorded by the lead |
| 06 | 1 | App shell & layout | `agora-app/src/ui/window*`, `ui/sidebar*`, `ui/*/mod.rs` stubs, `application.rs`, `rt.rs` | §5.1–5.2, §5.4, §11 (global keys) | 00, 01 | Three-pane adaptive layout with breakpoints; `AdwSidebar` sections; navigation actions (§22.2); tokio bridge; navigation and back gestures; status pages; works against mock client |
| 07 | 1 | Stores | `agora-app/src/stores/` | §3.4 | 01 | GObject stores exposing `ListModel`s, pagination, refresh, optimistic vote/save; `docs/STORES.md` frozen; unit tests with mock client |
| 08 | 2 | Onboarding & account | `ui/welcome*`, `ui/account*`, `ui/login_dialog*` | §6, §4.4 | 03, 06 | Welcome carousel; embedded login dialog; sign in/out; session-expired banner; status page to resume sign-in |
| 09 | 2 | Post list & rows | `ui/post_list*`, `ui/post_row*` | §7.1, §2.1 (quiet rows) | 06, 07 | Rows, thumbnails, badges, card mode toggle, context menu/long-press, pull-to-refresh, infinite scroll, j/k |
| 10 | 2 | Markdown renderer | `agora-app/src/render/` | §7.6 | 05 | AST → GTK widgets incl. tables, code, quotes, spoilers; zoom scaling; accessible labels |
| 11 | 2 | Post view | `ui/post_view*`, `ui/subreddit_header*` | §7.2, §2.1 (community headers) | 07, 10 | Header, link card, body, action bar, comment sort, subreddit header with Join |
| 12 | 2 | Comment tree | `ui/comments*` | §8, §2.1 (conversation lines) | 07, 10 | Flattened virtualized tree, thread lines with accent hover, animated collapse (animate the hidden branch with `GtkRevealer`/opacity, never row height on recycled rows), load more, continue thread, a11y depth |
| 13 | 2 | Media | `ui/media*`, `ui/viewer*` | §7.3–7.5 | 04, 06 | Inline images, galleries, Loupe-style viewer, video with HLS + fallback, GIF loop rules, NSFW/spoiler blur |
| 14 | 3 | Composer | `ui/composer*` | §9 | 04, 10 | All post types, subreddit picker + requirements, toolbar, preview toggle, drafts autosave/restore, discard dialog |
| 15 | 3 | Profiles & actions | `ui/profile*`, `ui/report*` | §5.3, §7.2 actions | 11, 12 | User profile tabs, report dialog, edit/delete own content, subscribe/unsubscribe |
| 16 | 3 | Inbox & notifications | `ui/inbox*`, `services/notify*`, `services/background*` | §5.3, §12.4 | 07, 08 | Inbox tabs, mark read, compose message, GNotification, opt-in Background portal checking at human pace (§4.6, §12.4) |
| 17 | 3 | Search & Shell provider | `ui/search*`, `services/search_provider*`, `data/*.search-provider.ini` | §10 | 04, 09 | Scoped search with toggle, result tabs, go-to-subreddit (Ctrl+K), D-Bus search provider from cache |
| 18 | 3 | Links & sharing | `services/url_router*`, `ui/share_menu.rs` | §12.1–12.3 | 06 | Router with unit tests for all URL shapes, open-with/CLI args, clipboard open, drag-and-drop, share menu |
| 19 | 3 | Preferences & filters | `ui/preferences*`, filter logic in `stores/filters*` | §13, §15 | 04, 07 | All preference pages bound to GSettings; muted subs/keywords with "N hidden" row; Clear Cache |
| 20 | 4 | Identity & assets | `data/icons/`, status page illustrations, AppStream screenshots | §2.1, §18 | — (design can start any time) | App + symbolic icon per guidelines, empty-state art, branding colors, screenshots light/dark |
| 21 | 4 | Accessibility & i18n audit | edit rights across `agora-app` for a11y/i18n fixes only (lead reviews); `po/` | §16, §17 | 08–19 | Orca and keyboard-only pass on every screen; RTL check |
| 22 | 4 | Source release | Build instructions, tagged release notes (no binaries) | §18.1, §20 | all | a fresh clone builds and installs with the documented `flatpak-builder` command; disclaimer present in README, About and onboarding; app runs signed-in for a day of normal use with no captcha or block |

### 22.4 Waves and parallelism

- **Wave 0** (serial, 1 agent): WP-00 then WP-01. Everything depends on the contracts.
- **Wave 1** (up to 6 agents in parallel): WP-02, 03, 04, 05, 06, 07.
- **Wave 2** (up to 6 agents): WP-08 to 13.
- **Wave 3** (up to 6 agents): WP-14 to 19.
- **Wave 4**: WP-21 and WP-22; WP-20 runs alongside any wave.

The lead agent merges each wave, runs the full app against Jared's live session (at human pace), and fixes integration seams before starting the next wave.

### 22.5 Brief template for each WP

When a WP is handed off, the lead writes `docs/work-packages/WP-NN.md` from this template:

```
# WP-NN: <name>
Goal: <one sentence>
Read: SPEC §1, §2.1, §3, §22.1, plus §<cited sections>
Owns: <paths>        Must not edit: everything else
Depends on (merged): <WPs>     Contracts used: <types/traits>
Deliverables: <list>
Acceptance criteria: <checklist, copied from §22.3 and expanded>
Mock scenarios to cover: empty, error, offline, 1000 items, RTL, 360 px
Contracts you may NOT change: <list>
Questions to ask the lead before starting: <list>
Out of scope: <explicit list>
Hand-back: PR link + notes on any contract change requests
```

---

## 23. Credits

Agora is inspired by [Giara](https://gitlab.gnome.org/World/giara), the GTK Reddit client by Gabriele Musco and contributors. No Giara code is reused; the credit appears in the README and the About dialog (`AdwAboutDialog` "Acknowledgements" section).
