# Agora spec review

**Reviewer:** Claude Fable 5.1
**Date:** 2026-10-31
**Reviewed:** `docs/SPEC.md` (draft 2026-10-31), `README.md`, and the decision list in project memory
**Verdict:** The spec is well written and the architecture and work-package plan are sound. Do **not** start development as specced, because the premise in §4 (every user registers their own Reddit client ID) no longer works, and Reddit has published an end date for the public Data API the whole app depends on. Decide the backend question first (§1 below); almost everything else in the spec survives that decision.

Priorities: **P0** blocks development. **P1** must change before wave 0 starts. **P2** should change before the affected wave. **P3** nice to have.

---

## 1. P0: Reddit API access is closing

What I verified from Reddit's own announcements (r/redditdev, r/modnews, reddithelp.com) and press coverage from September and October 2026:

| Date | What happens |
|---|---|
| Nov 2025 | "Responsible Builder Policy": every new OAuth app needs prior approval through a ticket. Self-service registration at `reddit.com/prefs/apps` effectively ended. Developers report the page now redirects to the policy and tickets go unanswered. |
| ~June 2026 | Unauthenticated `.json` endpoints shut. Old Reddit became login-only. |
| Aug 2026 | Reddit says third-party apps must move to the Developer Platform (Devvit). Exemptions only "for partners with written agreements", named examples are accessibility apps and the mobile client Narwhal. |
| Sept 30, 2026 | Schedule published: RSS ends Nov 13, 2026. |
| **Oct 31, 2026 (today)** | **New requests for public API access are no longer accepted.** |
| Jan 12, 2027 | Access starts closing for unregistered apps. |
| March 2027 | Public Data API access closes for all remaining apps and users. |
| Oct 29, 2026 | Steve Huffman's Q3 letter: Reddit is becoming a logged-in-only experience, tests start outside the US. |

Consequences for the spec:

- **§4.1 bring-your-own client ID** is dead. A new user cannot register an installed app, so onboarding step 2 cannot be completed by anyone who does not already hold a client ID.
- **§4.3 anonymous browsing** is dead for the same reason (the `installed_client` grant still needs a client ID), and Reddit is ending logged-out access anyway.
- **Devvit is not a client platform.** Devvit apps are TypeScript programs hosted by Reddit and rendered inside Reddit's own apps. A native GTK client cannot be "ported" to it. The only sanctioned route for a third-party client is a written agreement with Reddit, which Reddit has so far given to a handful of established mobile clients.
- **Even a client that works today stops in March 2027** unless it holds such an agreement.
- Smaller: **r/all was removed in April 2025** (still reachable on Old Reddit only). The "All" feed in §5.1 and §13 should go.

Sources: ppc.land, 2026-10-04, "Reddit to halt public Data API access in March 2027"; TechRepublic, "Reddit RSS and Public API Shutdown: Key Deadlines"; reddithelp.com, "Reddit Developer Platform App Migration Program 2026 Terms"; r/redditdev posts "Introducing the Responsible Builder Policy", "Our Plans for the Future of Reddit's Public Data API", "Moving Data API Apps to the Developer Platform"; Business Insider, 2026-10-30, on logged-in-only browsing.

### Options

| | Option | Assessment |
|---|---|---|
| A | **Keep the UI, swap the backend to open "Reddit-like" networks** (Lemmy, PieFed, Mbin). Open, documented APIs, no API keys, posting and inbox supported, Rust client crates exist for Lemmy. | **Recommended.** About 85% of the spec carries over unchanged: layout, rows, comment tree, composer, inbox, notifications, search provider, cache, filters, identity, work packages. §4 is rewritten, "subreddit" becomes "community", and onboarding becomes "pick an instance, sign in or browse". Discoverability by searching "Reddit" is still allowed via keywords ("Reddit alternative"), but the GenericName would change. Agora as a name fits a federated public square even better. |
| B | **File a Reddit access request today and pause.** Today is the last day Reddit accepts new public API requests. If Jared already holds an app registered before March 25, 2026, register it now at Reddit's developer sign-up so it survives the Jan 12 cutoff. | Cheap, do it regardless. But a new third-party client is unlikely to be approved, and approval still ends in March 2027 without a written agreement. Do not plan around it. |
| C | **Build Reddit-only as specced.** | Not recommended. Users cannot obtain client IDs, and the app has a public expiry date. Flathub would carry a broken app within months. |
| D | **Design the backend as pluggable from day one** (a `Backend` trait instead of `RedditClient`), ship A first, add Reddit later only if an agreement materialises. | Combine with A. Costs little now because WP-01 already freezes a client trait; it just needs Reddit-neutral naming and a `Capabilities` struct (votes, downvotes, DMs, flair, polls) so the UI hides what a backend lacks. |

**Recommendation: A + D, and do B today because it is free.** This is Jared's call because it changes the project goal.

---

## 2. P1: fix before wave 0

1. **Platform target is a release behind.** The spec targets GNOME 49 / GTK 4.20 / libadwaita 1.8. GNOME 51 shipped on 2026-09-16 with GTK 4.23 and libadwaita 1.10. The Circle committee already sends "outdated SDK" reminders to apps on the 49 SDK. Target `org.gnome.Platform//51`, and in the spec say "the current stable GNOME runtime" with the version in one place. Concretely useful in 1.9/1.10: `AdwSidebar` (use it for §5.1 instead of a hand-built list with `.navigation-sidebar`), `AdwCssClassBinding` (drives "read posts fade back" and vote-state colouring declaratively), `GtkEnumList` for the sort dropdowns, and GTK now honours reduced motion in Spinner, Stack and Revealer, which simplifies §7.4 and §8.1.
2. **GNOME Circle is currently closed and has an AI policy.** On 2026-05-29 the Circle committee stopped accepting new submissions until the backlog clears, and adopted the AI policy already used for GNOME Shell extensions for new submissions. Flathub tightened its AI policy at the same time. The development plan is to have Opus sub-agents write the code. Before committing to "Circle after 1.0" as a goal, read the current text of the Circle app criteria and Flathub's policy and decide whether Circle is still a realistic target. The HIG, accessibility and quality goals stand on their own either way, so this changes the stated goal, not the engineering. Treat Phase 7 as aspirational.
3. **Name clash.** There is no "Agora" app on Flathub that I could find, but agoralinux.app ("Agora Linux App Store") is a Flatpak directory built on top of Flathub, and Agora.io is a well-known SDK vendor. A Reddit-style app named Agora next to a Flatpak store named Agora will confuse Flathub searches. Decide now, before the app ID is baked in. If the name stays, the app ID is fine (see §5, question 7).
4. **Fixtures need a working account.** WP-02 and WP-05 depend on recorded JSON fixtures and a 50-sample Markdown corpus. Recording them needs live API access. For Reddit that needs a client ID Jared may not have. For option A it is trivial (any Lemmy instance serves JSON openly). Put "who records the fixtures, with which credentials" into WP-02's brief.

---

## 3. P2: architecture (Rust / gtk-rs)

The architecture in §3 is the right shape: plain-Rust library crates, GObject stores exposing `ListModel`s, views that never touch the network. Points to tighten:

1. **HTTP stack (open question 2): use `reqwest` with rustls on a dedicated `tokio` runtime thread.** Reasons: the library crates stay testable with `#[tokio::test]` and `wiremock` and need no GLib main loop; sub-agents can run `cargo test` without a display; `reqwest` has far more Rust ecosystem support. The cost is one extra thread and a bundled TLS stack, which many GNOME Rust apps (Fractal, Newsflash) accept. The bridge is a single helper: `spawn_tokio(fut) -> impl Future` that uses a `oneshot` channel and is awaited with `glib::spawn_future_local`. Put that helper in `agora-app/src/rt.rs`, owned by WP-06, and forbid tokio types elsewhere in `agora-app`. `soup3` is the "more GNOME" answer in theory, but it drags the main loop into every library test and gains nothing the user can see.
2. **Store API is a contract too.** WP-09, 11, 12, 16 and 19 bind to GObject property names, signals and item types of the stores from WP-07, but §22.2 freezes only `agora-core`. Add a §22.2 item: WP-07 ships `docs/STORES.md` listing each store's properties, signals, actions and `ListModel` item type, frozen at the end of wave 1.
3. **Navigation is a contract.** Five wave-2 packages need "open post X", "open subreddit Y", "open user Z", "push continue-thread page". Define these as window actions (`win.open-post(s)`, `win.open-subreddit(s)`, `win.open-user(s)`, `win.open-url(s)`) in WP-06 and list them in `agora-core` or `STORES.md`, so no UI package needs the window type.
4. **Comment collapse animation in a `GtkListView`.** Animating row height inside a virtualised list is awkward; rows are recycled and the list measures them eagerly. Two workable approaches: wrap each row's content in a `GtkRevealer` with `slide-up` for the *children* being hidden, or animate opacity and remove the rows on completion. Put "no animated height on recycled rows, animate the branch instead" in WP-12's brief so the agent does not burn time.
5. **Markdown.** `pulldown-cmark` has no Reddit extensions. Plan the extension layer as a pre-pass on source text for `>!spoiler!<` and `^(superscript)` into custom inline markers, and a post-pass on the event stream for `r/` and `u/` autolinks. The API also returns `body_html`; keep it as a debugging fallback only, never render it. (If the backend becomes Lemmy, its Markdown is closer to CommonMark with `::: spoiler` blocks and the layer shrinks.)
6. **Video.** `GtkVideo` on the GNOME 51 runtime handles HLS through `hlsdemux2` from gst-plugins-good; DASH needs gst-plugins-bad, which the runtime also ships. Keep the fallback, but note that Reddit video URLs need the OAuth `User-Agent` or they return 403. (Lemmy video is plain files or external links, so §7.4 mostly collapses to "external link card".)
7. **Composer dependencies.** Confirm `gtksourceview5` and `libspelling` are in the GNOME 51 SDK and runtime (both are used by GNOME Text Editor, so they should be). If `libspelling` is missing, add it as a Flatpak module in WP-00, not later.
8. **Image loading.** `glycin` is correct. Note it needs the `glycin-loaders` present in the runtime; inside Flatpak the loaders run in bwrap sandboxes, which is fine, but on non-Flatpak dev builds the host must have `glycin-loaders` installed. Put that in the README's build section.
9. **Secrets.** `oo7` with the Secret portal needs no permission in the manifest. Drop the "talk-name `org.freedesktop.secrets`" sentence from §18 entirely; a Flathub reviewer will flag it.
10. **Loopback OAuth port.** Fine if Reddit stays. For Lemmy the login is a plain password/TOTP POST, and the loopback server disappears.
11. **Small HIG/GNOME items missing from the spec:** persist window size and maximised state in GSettings; `AdwAboutDialog` should use `from_appdata` so release notes come from the metainfo; the composer on narrow screens should be an `AdwBottomSheet`, not a full-screen dialog, when it is a short reply; `AdwToolbarView` with `top-bar-style: raised` for scrolled pages. Nothing structural.

Everything else in §3, §14, §16, §17, §19 is what I would have written. Rust 2024 edition and MSRV pinned to the SDK extension are right.

---

## 4. P2: visual identity vs Giara, and HIG

The identity section is strong and clearly not Giara. "Quiet rows", "conversation lines", "community headers in a soft card", "calm by default" and the status-page illustration set are each real differentiators that are still entirely built from libadwaita, which is the right trade. Keep all of it. Two cautions:

- **Icon concept.** A portico whose negative space forms a speech bubble is a good idea for 128 px, but it has two levels of meaning (columns and bubble) and will not survive 16 px or the symbolic version. Brief the icon designer (WP-20) to design the symbolic icon first, as a single silhouette, and let the full-colour icon be an elaboration of it. The GNOME icon style is flat with a slight front perspective and very few shapes.
- **Sidebar.** Use `AdwSidebar` with sections (libadwaita 1.9+) rather than a hand-built list; it gives the HIG look, keyboard handling and the overlay sidebar style for free, and the design-wise distinct piece should be the rows and comments, not the sidebar.

HIG compliance otherwise checks out: triple pane via nested split views is the documented pattern, dialogs are `AdwDialog`/`AdwAlertDialog`, toasts for errors, status pages, shortcuts dialog, preferences dialog, header capitalisation, system accent only. The feature list in memory is fully covered by the spec; nothing decided is missing.

---

## 5. Answers to the open questions (§21)

1. **Reddit self-service registration:** No. Closed since November 2025, and all new public API requests stop today. See §1.
2. **HTTP stack:** `reqwest` + `tokio` on a runtime thread, behind one bridge helper. See §3.1.
3. **External reddit.com links:** Confirm the spec's recommendation. Do not register `x-scheme-handler/https`. Ship `GApplication` open-URI handling (which also gives "Open With"), Ctrl+L open-from-clipboard, and drag-and-drop. Also handle `%U` in the desktop file so file managers and browsers can hand over a URL.
4. **Sharing:** Confirm. There is still no Share portal in xdg-desktop-portal as of GNOME 51. Copy Link, Open in Browser and Send by Email is the correct fallback, with the menu abstracted so a portal can slot in.
5. **Hosting:** Stay on GitHub with Weblate for now. Circle does not require GNOME GitLab; it requires Flathub, HIG, accessibility, a CoC and a `.doap` file. Revisit only when and if a Circle application is actually possible (submissions are closed at the moment, see §2.2).
6. **Offline actions:** Keep offline read-only in 1.0. The toast is honest and the queue adds conflict handling that is not worth it before the backend question is settled.
7. **App ID:** Keep `io.github.ClickCalickClick.Agora`. Flathub verifies `io.github.*` IDs against the GitHub repository, so the repo must be named `agora` (matching is case-insensitive). Buying a domain buys nothing here. If the name changes (question 8), change the ID with it, before the first Flathub submission.
8. **Name clash:** See §2.3. No Flathub app uses it, but the Agora Linux App Store and Agora.io exist. Decide now.

---

## 6. P2: work-package review (§22)

The structure is good: contracts first, mock client, one crate per concern, file ownership, one PR per package, fixtures not live API. These are the changes that will prevent the wave-2 merge from turning into a conflict pile:

1. **Shared files nobody owns.** `data/resources/resources.gresource.xml`, `po/POTFILES.in`, `data/resources/style.css`, `crates/agora-app/src/ui/mod.rs` and the gschema are each touched by most UI packages. Fix in WP-00: generate the gresource XML and `POTFILES.in` from directory globs at Meson time, split CSS into `style/<module>.css` concatenated at build, and have WP-06 pre-create every `ui/<module>/mod.rs` stub listed in §22.3 so later packages only fill files they own.
2. **GSettings keys.** §13 already enumerates every key. Freeze the complete gschema in WP-00 (not stubs), so WP-09 (card mode), WP-11 (zoom), WP-13 (NSFW) and WP-19 do not all edit it in wave 2.
3. **Stores and navigation as contracts.** See §3.2 and §3.3; add both to §22.2.
4. **WP-21 cannot "file issues to owning WPs".** By wave 4 those agents are gone. Give WP-21 edit rights across `agora-app` for accessibility and i18n fixes, with the lead reviewing.
5. **WP-18 share menu has no path.** Give it `agora-app/src/ui/share_menu.rs` and have WP-09, 11, 12 consume it through a single `ShareMenu::for_url()` constructor defined in the brief.
6. **WP-13 and WP-11 integration.** Media widgets are built in WP-13 but mounted by WP-11. Specify in both briefs the one widget type and constructor WP-11 will call (`MediaView::new(&Media)`) so neither waits on the other.
7. **Screenshots and UI checks in acceptance criteria.** Opus sub-agents working in containers need a display. Put into the rules for every UI package: run under `xvfb-run`, use `ADW_DEBUG_ADAPTIVE_PREVIEW=1` for the 360 px shot, and capture with GTK's inspector or the `gtk4-broadwayd` backend. If the lead's environment cannot give that, drop the screenshot requirement and keep the keyboard walkthrough as a checklist.
8. **Brief template additions.** Add three fields: "Mock scenarios to cover" (empty, error, offline, 1000 items, RTL, 360 px), "Contracts you may NOT change", and "Questions to ask the lead before starting" so agents block early instead of improvising.
9. **Wave sizes.** Six parallel agents in one crate is the upper limit. Consider four per wave with WP-10 (renderer) and WP-05 (parser) merged early because WP-11, 12 and 14 all wait on them.
10. **If the backend pivots (option A)**, WP-02 and WP-03 are rewritten (Lemmy API client, password/TOTP login, instance picker), WP-08 changes its tour, and everything else keeps its brief with "subreddit" renamed. The package graph does not change.

---

## 7. P3: smaller edits

- §2: `Categories` should also include `InstantMessaging;` or `Feed;` only if accurate; `Network;News;` is right. Add `X-Purism-FormFactor=Workstation;Mobile;` to the desktop file for phone stores.
- §5.1: remove "All" (see §1) or scope it to backends that have it.
- §7.1: 64 px thumbnails should be requested at 2× for HiDPI; cache the chosen size.
- §11: `h` for back collides with Vim users' expectation of "left"; fine, but say so in the shortcuts dialog group name ("Navigation").
- §16: add `GTK_A11Y=test` runs to CI to catch unlabeled widgets automatically.
- §18: content rating should include `social-info` (user names) and `social-contacts`, not only user-generated content.
- §19: add `cargo deny` or `cargo audit` since the dependency tree with `reqwest` and `rusqlite` is large.
- README: the "Bring your own API key" section must be rewritten or removed whichever option is chosen.

---

## 8. What to do next, in order

1. Jared decides between options A, B, C, D in §1 (I recommend A + D, with B done today because it costs nothing).
2. The spec thread updates §3.1 to GNOME 51, applies §2 and §6 of this review, and rewrites §4 and §6 for the chosen backend.
3. Create the repo named `agora`, commit spec + review, then start wave 0.
