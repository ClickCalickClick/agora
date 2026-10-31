# Stage 6: Comprehension Test

Target: docs/SPEC.md (PR #1 branch). FeatureRange 1-641, DetailedRange 642-735. Reviewer: egm-comprehension (fresh context).

## What this proposes

Agora is a new native GNOME client for Reddit, written in Rust with GTK 4 and libadwaita. It is public source and each user builds it themselves; nothing is distributed. Reddit's public API is closing, so Agora signs the user in to reddit.com inside a WebKitGTK view. A hidden, logged-in WebView then fetches data through an injected JavaScript bridge, using three fallback strategies: same-origin JSON, site-internal endpoints, and reading the page DOM. Traffic is paced like a person browsing. Everything Reddit-specific sits behind a `Backend` trait. The UI (three adaptive panes, post rows, comment tree, composer, inbox, search, media, preferences, offline cache) is built from stock libadwaita widgets plus a few signature touches. §22 splits the build into 23 work packages (WP-00 to WP-22), run in waves 0-4 by parallel AI sub-agents under a lead agent. Shared contracts are frozen first: `agora-core` types, the GSettings schema, `docs/STORES.md`, navigation actions and two shared widgets. UI work runs against a `MockBackend`, and agents use recorded fixtures, never live Reddit. Each WP gets a brief written from a fixed template.

## How the existing system works, per this document

No code exists yet. The repository holds only planning documents. The "existing system" is the target platform: GNOME 51, GTK 4.23 and libadwaita 1.10, with WebKitGTK 6.0 in the runtime. It also builds on reddit.com's logged-in web front end, which still serves JSON endpoints such as `/r/{sub}/hot.json`, internal `/svc/shreddit/` partials and `shreddit-*` web components. These change without notice, and Reddit pushes back with 429s and captchas. Giara is inspiration only.

## Findings

1. 🟡 **Same-wave dependencies contradict "same wave runs in parallel"** (L674, 680, 688, 690, 700, 705-708). WP-02 deps WP-03 (both wave 1; "bridge API only" unexplained); WP-11/12 dep WP-10 (wave 2); WP-22 deps "all" incl. WP-21 (wave 4). Unclear whether dependents wait for merge, start on a stub, or start once an interface is agreed.
2. 🟡 **Who establishes STRATEGIES.md against the live site is contradictory** (L233, 637, 653, 680, 710). "WP-02's first week against Jared's live session" vs rule 6 "agents never send traffic to reddit.com" vs "lead's live run" vs lead runs the app only after a wave merges. Handoff not described; "first week" assumes a calendar §22 doesn't use.
3. 🟡 **Ownership gaps/overlaps break "stay in your lane"** (L649, 654, 668-669, 680, 684-685, 694-697, 500). `docs/STORES.md` shipped by WP-07 but in no one's paths; `stores/filters*` (WP-19) inside WP-07's dir; `--self-test` (WP-02) is a flag on the binary WP-06 owns; services must be registered in `application.rs` (WP-06) with no stubs; Subscribe/Join claimed by WP-11 and WP-15.
4. 🟡 **No WP owns cache-then-refresh composition** (L162-173, 662, 682, 685). Neither WP-04 nor WP-07 covers read-through/refresh, deleted-content purge on refresh, or 30-day expiry.
5. 🟡 **§12.2 and §12.3 still say "Open question for review" though §21 says decided** (L11, 466, 474, 628-629, 696). "The memory decision" (L466) references context the reader lacks.
6. 🟢 Rule 7 stub list ambiguous vs §22.3 globs (L654, 684-698).
7. 🟢 Relative "Owns" paths have no stated base (L680-697).
8. 🟢 `MockBackend` vs "mock client" (L655, 679, 684, 685).
9. 🟢 Phase-to-wave mapping overlaps (L608, §20 vs §22.4); unclear if §20 exit criteria gate waves.
10. 🟢 Rule 1 reading list incomplete: every WP needs §22.2; WP-13 and WP-16 need §4.6 (L648, 719).
11. 🟢 Wording assumes earlier context: "conversation history" (L644), "`AgoraError` gains" (L253), SPEC-REVIEW reliance (L9, 181), `/api/morechildren` vs "no OAuth API" (L391, 181).
