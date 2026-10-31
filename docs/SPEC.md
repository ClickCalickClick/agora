# Agora: Design Specification

**Status:** Draft for review (2026-10-31)
**Owner:** Jared (ClickCalickClick)
**Inspiration:** [Giara](https://gitlab.gnome.org/World/giara) by Gabriele Musco

Agora is a native GNOME client for Reddit, written in Rust with GTK 4 and libadwaita. It is a new project, not a fork: it takes Giara's idea (a Reddit app that feels like it belongs on GNOME) and rebuilds it for current GNOME, adaptive layouts, touch, and GNOME Circle.

This document is the single source of truth for what Agora does and how it is built. Anything marked **Open question** must be settled during review, before the matching roadmap phase starts.

---

## 1. Goals and non-goals

### Goals

1. Feel like a first-party GNOME app: HIG-compliant, adaptive, accessible, translatable.
2. Meet the [GNOME Circle](https://circle.gnome.org/) quality bar (HIG, accessibility, CoC, Flathub). Applying to Circle itself is **aspirational**: see the status note in §18.
3. Cover everyday Reddit use: browsing, reading, voting, posting, replying, and the inbox.
4. Work equally well with mouse and keyboard, touch, and a screen reader.
5. Distribute on Flathub.

### Non-goals

- Moderation tools (mod queue, removals, flair management, mod mail). Out of scope permanently, not just deferred.
- Chat (Reddit's chat API is not public).
- Swipe-to-vote or swipe-to-save gestures. Swipes are for navigation only.
- Support for anything older than the current stable GNOME runtime (§3.1).
- Multiple accounts in 1.0 (kept possible in the data model; see §8.3).
- A bundled Reddit API key. Users register their own client ID.

---

## 2. Identity and discoverability

| Item | Value |
|---|---|
| Name | Agora |
| App ID | `io.github.ClickCalickClick.Agora` (reverse-DNS of a domain we control; change if a custom domain is bought before 1.0) |
| GenericName | Reddit Client |
| Summary (AppStream) | "Browse and discuss Reddit" |
| Desktop `Keywords` | `Reddit;Subreddit;Forum;Social;News;Discussion;` (translatable) |
| Categories | `Network;News;GTK;GNOME;` |
| License | GPL-3.0-or-later (same family as Giara; Circle requires an OSI license) |
| Metadata license | CC0-1.0 |
| Code of Conduct | GNOME Code of Conduct (`CODE_OF_CONDUCT.md` links to it) |

**Discoverability:** searching "Reddit" in GNOME Software, Flathub, and the Activities overview must find Agora. This is done through `GenericName`, `Keywords`, and the AppStream `<keywords>` and `<summary>`. The name "Agora" itself must not contain "Reddit" (trademark).

**Trademark note:** the app must not use the Reddit logo or Snoo in its icon, and AppStream must state that Agora is an unofficial client not affiliated with Reddit, Inc.

**Name clash (decide before WP-00 bakes in the app ID):** no Flathub app is called Agora, but the "Agora Linux App Store" (agoralinux.app, a Flatpak directory built on Flathub) and Agora.io (a well-known real-time SDK vendor) both exist. A community app named Agora next to a Flatpak store named Agora may confuse Flathub searches. If the name changes, the app ID changes with it, before the first Flathub submission. The `io.github.ClickCalickClick.Agora` ID is otherwise correct: Flathub verifies `io.github.*` IDs against the matching GitHub repo.

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
| HTTP | `reqwest` (rustls) on a `tokio` runtime | **Decided.** One `tokio` runtime on a dedicated thread. A single bridge helper `spawn_tokio(fut)` in `agora-app/src/rt.rs` (owned by WP-06) sends results back over a `oneshot` channel awaited with `glib::spawn_future_local`. No tokio types anywhere else in `agora-app`. Keeps library crates testable with `#[tokio::test]` and no display. |
| JSON | `serde`, `serde_json` | Typed models for Reddit "Thing" objects |
| Portals | `ashpd` | Background, OpenURI, Secret, Settings |
| D-Bus | `zbus` | GNOME Shell search provider |
| Secrets | `oo7` | Uses the Secret portal inside Flatpak, keyring outside |
| Cache DB | `rusqlite` (bundled SQLite) | Metadata and post bodies |
| Image decoding | `glycin` | Same sandboxed loader stack as Loupe; supports animated formats |
| Markdown | `pulldown-cmark` plus a Reddit extension layer | Reddit adds spoilers (`>!text!<`), superscript (`^`), `/r/` and `/u/` autolinks |
| Video | `GtkVideo` / `GtkMediaFile` (GStreamer backend) | Reddit hosted video is DASH/HLS; see §7.4 |
| i18n | `gettext-rs` | |
| Logging | `tracing` | Never logs tokens or post content at info level |
| Build | Meson + Cargo | Standard GNOME Rust template layout |

### 3.3 Repository layout (planned)

The code is a **Cargo workspace of small crates** so independent work packages (§22) can be built and tested in isolation by separate agents. Only `agora-app` depends on GTK; the library crates are plain Rust and testable without a display.

```
agora/
├── Cargo.toml                 # workspace
├── crates/
│   ├── agora-core/            # shared types: IDs, Thing models, errors, traits (contracts)
│   ├── agora-api/             # Reddit HTTP client, OAuth, rate limiting
│   ├── agora-cache/           # SQLite store, image cache, eviction, drafts
│   ├── agora-markdown/        # Reddit Markdown → document AST (no GTK)
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
 │  Cache (SQLite + files)│◄──┤  API client (async)      │
 └────────────────────────┘   │  OAuth · rate limit · JSON│
                              └─────────────┬────────────┘
                                            ▼
                                    oauth.reddit.com
```

- **Stores** are GObjects that own state and expose `gio::ListModel`s, so views bind to them directly (`GtkListView` + `GtkSignalListItemFactory`). Views never call the API.
- **API client** is a plain async Rust type (no GTK), unit-testable with recorded fixtures.
- **Cache** sits between stores and the API: reads return cached data first, then refresh.

---

## 4. Reddit API and authentication

### 4.1 Bring-your-own client ID

Agora ships with no API credentials. Each user registers a Reddit app of type **installed app** at `https://www.reddit.com/prefs/apps` and pastes the client ID into Agora. Installed apps have no client secret, so nothing secret is stored besides the user's tokens.

- Redirect URI to register: `http://127.0.0.1:65010/agora-auth` (fixed, shown in onboarding with a Copy button).
- User-Agent: `linux:io.github.ClickCalickClick.Agora:<version> (by /u/<username>)`, as Reddit's API rules require.

**Risk (must verify before Phase 1):** Reddit has tightened API access since 2023 (paid tiers for high volume, and stricter review of new apps). Confirm that self-service registration of an installed app still works, what the current free rate limit is, and whether personal-use apps need approval. If registration is gated, onboarding must explain the process and the app must handle "app not approved" errors clearly.

### 4.2 OAuth flow

1. User taps **Sign In**. Agora generates a random `state`, starts a one-shot HTTP listener on `127.0.0.1:65010`, and opens the authorize URL in the browser through the OpenURI portal:
   `https://www.reddit.com/api/v1/authorize?client_id=…&response_type=code&state=…&redirect_uri=…&duration=permanent&scope=…`
2. The browser redirects to the loopback listener. Agora validates `state`, shows a small "You can close this tab" page, and stops the listener.
3. Agora exchanges the code at `/api/v1/access_token` (HTTP Basic with client ID and empty secret).
4. Refresh token goes to the keyring via `oo7`. The access token stays in memory only and is refreshed on 401 or before expiry.
5. Sign out revokes the token (`/api/v1/revoke_token`) and clears the keyring entry and personal cache.

Scopes: `identity read mysubreddits subscribe vote save submit edit history privatemessages report`. No `modposts`, `modconfig`, or other mod scopes.

**Fallback:** if port 65010 is busy, show an error that names the port. (Reddit requires the redirect URI to match exactly, so a random port is not possible.)

### 4.3 Anonymous browsing

Before sign-in (or if the user skips it), Agora can browse with an application-only token (`grant_type=https://oauth.reddit.com/grants/installed_client`). This still requires a client ID. Voting, posting, inbox, and subscriptions are disabled with an inline "Sign in to vote" prompt instead of hidden buttons.

### 4.4 Rate limiting and networking

- Read `X-Ratelimit-Remaining` / `X-Ratelimit-Reset` on every response; a token bucket delays requests before the limit is hit.
- Background inbox checks use the same budget and back off first.
- Network state comes from `gio::NetworkMonitor`. Offline mode shows an `AdwBanner` ("You're offline — showing saved content") rather than error dialogs.
- Errors surface as `AdwToast`s with a Retry action; never as modal dialogs.

### 4.5 Endpoints used (summary)

Listings (`/best`, `/hot`, `/new`, `/top`, `/rising`, `/r/{sub}/…`), comments (`/comments/{id}`, `/api/morechildren`), user (`/api/v1/me`, `/user/{name}/…`), subscriptions (`/subreddits/mine/subscriber`, `/api/subscribe`), actions (`/api/vote`, `/api/save`, `/api/unsave`, `/api/hide`, `/api/report`), posting (`/api/submit`, `/api/comment`, `/api/editusertext`, `/api/del`), media upload (`/api/media/asset.json`), inbox (`/message/inbox`, `/message/unread`, `/api/read_message`, `/api/compose`), search (`/search`, `/r/{sub}/search`, `/subreddits/search`), subreddit info (`/r/{sub}/about`, `/r/{sub}/about/rules`, `/api/v1/{sub}/post_requirements`).

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

A skippable welcome tour shown in an `AdwDialog` with an `AdwCarousel` and dot indicators:

1. **Welcome:** app icon, one-sentence pitch, "Unofficial Reddit client" note.
2. **Set up API access:** explains why a client ID is needed, a button that opens `reddit.com/prefs/apps`, step-by-step text (choose "installed app", paste this redirect URI with a Copy button), and an entry for the client ID with live validation (format check, then a test token request).
3. **Sign in:** Sign In button (OAuth flow), or "Browse without signing in".
4. **Done:** quick tips (keyboard shortcuts, Preferences).

"Skip" is always available. If skipped without a client ID, the main window shows an `AdwStatusPage` with a "Set Up Reddit Access" button that reopens step 2. The client ID can be changed later in Preferences.

---

## 7. Reading experience

### 7.1 Post rows (list)

Text-first rows, built as a custom `GtkListBoxRow`-style widget in a `GtkListView`:

- Title (wraps to 3 lines, `.heading` weight), then a subtitle line: `r/sub · u/author · 3h · flair`.
- Small rounded thumbnail (64×64, `border-radius: 8px`) on the trailing edge when the post has media.
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
- Reddit-hosted video (`v.redd.it`) is delivered as DASH/HLS with separate audio. Use the HLS playlist through GStreamer's `hlsdemux`; verify in Phase 3 that the GNOME runtime's GStreamer plugins handle it. **Fallback:** play the highest `fallback_url` MP4 (video only) with a "No audio" note, plus Open in Browser.
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
- **Background checking:** when the user enables "Check for new messages in the background" in Preferences, Agora requests permission through the **Background portal** (with autostart), then keeps running hidden after the window closes and polls `/message/unread` every 5 minutes (configurable 5/15/30/60), backing off when offline or rate-limited.
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
**Notifications:** Enable notifications, background checking + interval, which types (replies, mentions, messages).
**Storage:** cache size limit (100 MB / 250 MB / 500 MB / 1 GB), current usage, **Clear Cache** button.
**Account:** Reddit client ID (change), signed-in user, Sign Out.

All persisted in GSettings (`io.github.ClickCalickClick.Agora.gschema.xml`).

---

## 14. Offline cache

- **What:** the most recent pages of each feed the user viewed, opened posts with their loaded comments, all saved posts, thumbnails, and images the user has viewed.
- **Where:** `$XDG_CACHE_HOME/agora/` (SQLite DB `cache.db` + `images/` content-addressed by URL hash). Drafts and preferences are not cache; drafts live in `$XDG_DATA_HOME/agora/agora.db`.
- **Policy:** LRU eviction when over the size limit; saved posts are evicted last. Entries carry `fetched_at`; UI shows cached content immediately and refreshes in the background when online.
- **Offline UX:** banner, read-only actions; votes/saves made offline are **not** queued in 1.0 (they show "You're offline" toast). **Open question:** queue offline actions in a later release?
- Sign out clears personal cache (inbox, saved, drafts).

---

## 15. Filters

- **NSFW:** §7.5.
- **Muted subreddits:** their posts are removed from Home and Popular; visiting the subreddit directly still works with a banner "You muted this subreddit · Unmute".
- **Muted keywords:** posts whose titles match are hidden from feeds. A collapsed "N posts hidden by filters" row appears at the end of a page so filtering is never silent.
- Filtering is local only (not synced to Reddit).

---

## 16. Accessibility (Circle requirement, from day one)

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
- **Platform:** GNOME's Damned Lies requires hosting on GNOME GitLab. Since the repo is on GitHub, use **Weblate** (hosted.weblate.org libre plan). Revisit if the project moves to GNOME GitLab (`World/`) for Circle.

---

## 18. GNOME Circle and HIG checklist

| Requirement | How Agora meets it |
|---|---|
| Follows the GNOME HIG | libadwaita widgets throughout, adaptive layout, standard header bars, no custom window chrome |
| Free license | GPL-3.0-or-later, CC0 metadata |
| Code of Conduct | GNOME CoC in repo |
| App icon | Original full-color icon in GNOME style (made with App Icon Preview, follows the icon guidelines), plus symbolic icon; no Reddit marks |
| AppStream metadata | Complete: description, screenshots (light + dark), releases, content rating (OARS: user-generated content, social chat), `<branding>` colors, keywords |
| Translatable | gettext + Weblate |
| Accessible | §16 |
| Adaptive / mobile | §5, tested at 360×640 |
| Distributed on Flathub | Phase 6 |
| Privacy | No telemetry; network traffic only to Reddit and the media hosts the user opens |
| Maintained | Release notes per version, issue templates |

**Circle status (as of 2026-10-31):** on 2026-05-29 the GNOME Circle committee stopped accepting new submissions until its backlog clears, and adopted the AI policy already used for GNOME Shell extensions. Flathub tightened its AI policy at the same time. Agora is planned to be written largely by AI agents, so before treating Circle as a goal, read the current Circle criteria and Flathub policy text. The HIG, accessibility and quality targets in this table stand on their own either way; only Phase 7 is affected.

Flatpak permissions (minimal): `--share=network`, `--socket=wayland`, `--socket=fallback-x11`, `--share=ipc`, `--device=dri`, `--socket=pulseaudio` (video audio). No filesystem access (portals only). Secret, Background, OpenURI, FileChooser, Notification all go through portals. `oo7` uses the Secret portal, so no `org.freedesktop.secrets` talk-name is requested.

---

## 19. Testing and quality

- **Unit tests:** API models parsed from recorded JSON fixtures (no live API in CI); Markdown parser; URL router; filter matching; cache eviction.
- **Integration:** a mock Reddit server (`wiremock`) for OAuth and listing flows.
- **UI:** manual test script per phase (desktop, narrow 360 px, touch, keyboard-only, Orca).
- **CI (GitHub Actions):** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, Flatpak build with the target GNOME SDK, `GTK_A11Y=test` run to catch unlabeled widgets, `cargo deny`, `appstreamcli validate`, `desktop-file-validate`, `glib-compile-schemas --strict`.
- Rust edition 2024; MSRV pinned to the Rust SDK extension of the target runtime.

---

## 20. Phased roadmap

Each phase ends with a usable build and the accessibility + keyboard walkthrough. Phases describe *what the app can do* at each milestone; §22 splits the same work into work packages for parallel agents (Phase 0 ≈ wave 0, Phases 1–2 ≈ waves 1–2, Phases 3–5 ≈ waves 2–3, Phase 6 ≈ wave 4).

| Phase | Name | Delivers | Exit criteria |
|---|---|---|---|
| 0 | Foundations | Meson + Cargo scaffold, Flatpak manifest, CI, app ID, GSettings, i18n plumbing, empty `AdwApplicationWindow`, About dialog, CoC/README | CI green; Flatpak installs and launches |
| 1 | Auth & API core | Welcome tour, client ID entry, OAuth loopback flow, keyring storage, app-only anonymous token, API client with rate limiting, typed models, fixtures | Sign in/out works; `/api/v1/me` shown in avatar popover |
| 2 | Read-only browsing | Three-pane adaptive layout, sidebar with subscriptions, feeds + sorting, post rows with thumbnails, post view with text/link posts, comment tree with collapse + load more, Markdown renderer, empty/error states, keyboard nav j/k | Daily-driver for reading text posts on desktop and phone |
| 3 | Media | Image viewer, galleries, inline video, GIF looping rules, NSFW/spoiler blur, large card previews, image cache | All common post types display correctly |
| 4 | Interaction | Voting, saving, hiding, reporting, subscribe/unsubscribe, composer (all modes) with toolbar, preview, drafts, edit/delete, user profiles | Can post, reply, and edit end to end |
| 5 | Inbox & desktop integration | Inbox, messages, notifications, Background portal checking, Shell search provider, URL routing + open-from-clipboard, share menu, search with scope toggle | All integrations working inside Flatpak |
| 6 | Polish & release | Preferences complete, filters, offline cache with limits + Clear Cache, content zoom, full shortcuts dialog, icon, screenshots, AppStream, translations on Weblate, Orca audit | 1.0 on Flathub |
| 7 | Circle (aspirational) | Apply to GNOME Circle if submissions are open and the AI policy permits it (§18) | Accepted into Circle |
| Later | | Multi-account, offline action queue, polls, share portal if available | |

---

## 21. Open questions for the review

1. Is Reddit self-service API registration (installed app) still available and free at personal-use volumes? (§4.1)
2. ~~HTTP stack~~ Decided: `reqwest` + `tokio` thread behind one bridge (§3.2).
3. External reddit.com links: confirm dropping the global URL-handler idea in favor of Open With / clipboard / drag-and-drop. (§12.2)
4. Sharing: confirm the fallback share menu until a share portal exists. (§12.3)
5. Hosting: stay on GitHub (Weblate) or move to GNOME GitLab `World/` for Damned Lies and Circle visibility? (§17)
6. Queue offline votes/saves, or keep offline mode read-only? (§14)
7. App ID: keep `io.github.ClickCalickClick.Agora` or buy a domain first? Changing it after Flathub release is painful. (§2)
8. Name clash: keep "Agora" despite the Agora Linux App Store and Agora.io, or rename before the app ID is fixed? (§2)
9. Circle: still a goal given closed submissions and the AI policy? (§18)

---

## 22. Work breakdown for parallel agents

Development is planned to be split across multiple Opus 5.5 sub-agents, with a lead agent coordinating. This section makes each piece of the spec a self-contained **work package (WP)** an agent can take without reading the whole conversation history.

### 22.1 Rules for every agent

1. **Read first:** this spec's §1, §2.1, §3, the WP's own row below, and the spec sections the WP cites. Nothing else is required context.
2. **Stay in your lane:** an agent edits only the paths its WP owns. Changes needed elsewhere go back to the lead as a request, never as a drive-by edit.
3. **Contracts before code:** shared types and traits live in `agora-core` and are frozen by WP-01 before parallel work starts. Changing a contract needs the lead's sign-off and a note in `docs/CONTRACTS-CHANGELOG.md`.
4. **One branch and one PR per WP**, named `wp-NN-short-name`. The PR description lists the acceptance criteria with checkboxes.
5. **Definition of done (all WPs):** `cargo fmt`, `cargo clippy -D warnings`, and `cargo test` pass for the touched crates; public items are documented; user-facing strings are translatable; UI WPs include the keyboard and screen-reader checks in §16 and a screenshot at 360 px and 1280 px widths.
6. **Fixtures, not live API:** agents never call the real Reddit API in tests. They use JSON fixtures in `crates/agora-api/tests/fixtures/` (created by WP-02) or the mock server.
7. **No hand-edited shared lists:** WP-00 makes Meson generate the gresource XML and `po/POTFILES` from directory globs, and concatenates `data/resources/style/<module>.css` at build time. WP-06 pre-creates every `ui/<module>/mod.rs` stub named in §22.3. Later packages only add files inside the paths they own.
8. **UI built against mocks:** UI WPs bind to stores fed by an in-memory `MockRedditClient` (implements the `RedditClient` trait from WP-01), so UI work never waits on the network layer.

### 22.2 Contracts (frozen by WP-01)

`agora-core` defines, at minimum:
- IDs: `PostId`, `CommentId`, `SubredditName`, `Username`, `Fullname` (`t3_…`).
- Models: `Post`, `Comment`, `MoreComments`, `Subreddit`, `User`, `Message`, `Listing<T>`, `Media` (Image, Gallery, Video, Gif, Link, None), `Vote`, `Sort`, `TimeRange`.
- Traits: `RedditClient` (async: listings, comments, more-children, vote, save, submit, comment, inbox, search, subscribe…), `TokenStore`, `Cache` (get/put/evict, drafts).
- `AgoraError` enum (Network, RateLimited { reset }, Unauthorized, NotFound, Forbidden, AppNotApproved, Parse, Io).
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
| 00 | 0 | Scaffold & CI | root `Cargo.toml`, `meson.build`, `build-aux/`, `data/` (desktop, metainfo, complete gschema), `po/`, `.github/` | §2, §3, §13, §18, §19 | — | Flatpak builds and launches an empty `AdwApplicationWindow`; complete gschema frozen; gresource/POTFILES/CSS generated from globs; CI runs fmt, clippy, test, appstream/desktop validation |
| 01 | 0 | Core contracts | `crates/agora-core/` | §3.4, §22.2 | 00 | All types/traits in §22.2 compile with docs; serde round-trip tests; `MockRedditClient` with sample data |
| 02 | 1 | API client | `crates/agora-api/` (except `auth/`) | §4.4, §4.5 | 01 | Implements `RedditClient`; rate limiter honours headers; every endpoint parsed from fixtures; error mapping to `AgoraError` |
| 03 | 1 | OAuth & tokens | `crates/agora-api/src/auth/`, keyring `TokenStore` impl | §4.1–4.3 | 01 | Loopback flow with state check, code exchange, refresh, revoke, app-only token; tested with `wiremock`; refresh token only in keyring |
| 04 | 1 | Cache & drafts | `crates/agora-cache/` | §14, §9 (drafts) | 01 | SQLite schema + migrations; LRU eviction with saved-last rule; image cache; drafts CRUD; size reporting; clear cache |
| 05 | 1 | Markdown parser | `crates/agora-markdown/` | §7.6 | 01 | Reddit extensions (spoilers, superscript, r/ u/ autolinks, tables); property tests; corpus of 50 real-world samples |
| 06 | 1 | App shell & layout | `agora-app/src/ui/window*`, `ui/sidebar*`, `ui/*/mod.rs` stubs, `application.rs`, `rt.rs` | §5.1–5.2, §5.4, §11 (global keys) | 00, 01 | Three-pane adaptive layout with breakpoints; `AdwSidebar` sections; navigation actions (§22.2); tokio bridge; navigation and back gestures; status pages; works against mock client |
| 07 | 1 | Stores | `agora-app/src/stores/` | §3.4 | 01 | GObject stores exposing `ListModel`s, pagination, refresh, optimistic vote/save; `docs/STORES.md` frozen; unit tests with mock client |
| 08 | 2 | Onboarding & account | `ui/welcome*`, `ui/account*` | §6, §4.2 UX | 03, 06 | Welcome tour carousel; client ID validation; sign in/out; skip path; status page to resume setup |
| 09 | 2 | Post list & rows | `ui/post_list*`, `ui/post_row*` | §7.1, §2.1 (quiet rows) | 06, 07 | Rows, thumbnails, badges, card mode toggle, context menu/long-press, pull-to-refresh, infinite scroll, j/k |
| 10 | 2 | Markdown renderer | `agora-app/src/render/` | §7.6 | 05 | AST → GTK widgets incl. tables, code, quotes, spoilers; zoom scaling; accessible labels |
| 11 | 2 | Post view | `ui/post_view*`, `ui/subreddit_header*` | §7.2, §2.1 (community headers) | 07, 10 | Header, link card, body, action bar, comment sort, subreddit header with Join |
| 12 | 2 | Comment tree | `ui/comments*` | §8, §2.1 (conversation lines) | 07, 10 | Flattened virtualized tree, thread lines with accent hover, animated collapse, load more, continue thread, a11y depth |
| 13 | 2 | Media | `ui/media*`, `ui/viewer*` | §7.3–7.5 | 04, 06 | Inline images, galleries, Loupe-style viewer, video with HLS + fallback, GIF loop rules, NSFW/spoiler blur |
| 14 | 3 | Composer | `ui/composer*` | §9 | 04, 10 | All post types, subreddit picker + requirements, toolbar, preview toggle, drafts autosave/restore, discard dialog |
| 15 | 3 | Profiles & actions | `ui/profile*`, `ui/report*` | §5.3, §7.2 actions | 11, 12 | User profile tabs, report dialog, edit/delete own content, subscribe/unsubscribe |
| 16 | 3 | Inbox & notifications | `ui/inbox*`, `services/notify*`, `services/background*` | §5.3, §12.4 | 07, 08 | Inbox tabs, mark read, compose message, GNotification, Background portal + polling with backoff |
| 17 | 3 | Search & Shell provider | `ui/search*`, `services/search_provider*`, `data/*.search-provider.ini` | §10 | 04, 09 | Scoped search with toggle, result tabs, go-to-subreddit (Ctrl+K), D-Bus search provider from cache |
| 18 | 3 | Links & sharing | `services/url_router*`, `ui/share_menu.rs` | §12.1–12.3 | 06 | Router with unit tests for all URL shapes, open-with/CLI args, clipboard open, drag-and-drop, share menu |
| 19 | 3 | Preferences & filters | `ui/preferences*`, filter logic in `stores/filters*` | §13, §15 | 04, 07 | All preference pages bound to GSettings; muted subs/keywords with "N hidden" row; Clear Cache |
| 20 | 4 | Identity & assets | `data/icons/`, status page illustrations, AppStream screenshots | §2.1, §18 | — (design can start any time) | App + symbolic icon per guidelines, empty-state art, branding colors, screenshots light/dark |
| 21 | 4 | Accessibility & i18n audit | edit rights across `agora-app` for a11y/i18n fixes only (lead reviews); `po/` | §16, §17 | 08–19 | Orca and keyboard-only pass on every screen; RTL check; Weblate set up |
| 22 | 4 | Release | Flathub manifest, release notes | §18, §20 | all | Flathub submission passes review |

### 22.4 Waves and parallelism

- **Wave 0** (serial, 1 agent): WP-00 then WP-01. Everything depends on the contracts.
- **Wave 1** (up to 6 agents in parallel): WP-02, 03, 04, 05, 06, 07.
- **Wave 2** (up to 6 agents): WP-08 to 13.
- **Wave 3** (up to 6 agents): WP-14 to 19.
- **Wave 4**: WP-21 and WP-22; WP-20 runs alongside any wave.

The lead agent merges each wave, runs the full app against the real API with Jared's client ID, and fixes integration seams before starting the next wave.

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
