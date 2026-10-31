# Stage 9: Implementation Readiness Test

Target: docs/SPEC.md DetailedRange 642-735 (§22, §23), standard implementation-readiness-standard.md. All three reviewers completed.

## egm-implementation-readiness (DetailedRange 642-735) — NOT READY, 6 Blocker / 6 Major / 3 Nit

- R1 🔴 WP-01 contracts are a sketch (657-664, 679; §4.3 208-212): "at minimum" + "…"; no signatures, pagination cursors, return types; async/Send strategy undecided (WebKit is main-thread so futures likely !Send); no event mechanism for `session-expired` (219); model fields, Capabilities, Cache trait, Block/Inline AST variants undefined. Spell them out in §22.2.
- R2 🔴 Naming contradiction: §4.3 neutral `Community` vs frozen `SubredditName`/`Subreddit` (660-661 vs 212).
- R3 🔴 Stores contract not frozen when dependants start (668-669, 684-685, 704-705): STORES.md written by WP-07 in the same wave WP-06 needs it; action names ownership conflicts; no store list beyond §3.4 (Search, Profile, Subreddit, Subscriptions, Drafts missing). Freeze store list + action names in §22.2.
- R4 🔴 Inverted/missing deps (680, 686-697): WP-02→WP-03 same wave with no frozen bridge API; WP-09/12 (wave 2) need `ShareMenu` from WP-18 (wave 3); WP-11 mounts `MediaView` (WP-13, same wave, not in deps); WP-08/14/17 missing WP-07; WP-09 thumbnails need WP-04/13.
- R5 🔴 Cross-cutting work with no owner (649 vs 678-700): cache read-through, deleted-content purge/expiry, offline banner (NetworkMonitor), media HTTP fetcher, `main.rs`/CLI (`--self-test`, URL args), About dialog vs `application.rs`, shortcuts dialog/accelerator table, Drafts sidebar entry, session-expired banner mount, accounts storage, "Check Reddit connection" action.
- R6 🔴 Acceptance criteria an agent bound by rule 6 can't meet (653, 680, 683, 700): STRATEGIES.md from lead's live run, 50-sample corpus from lead (not in deps, no delivery point); WP-22 "a day of normal use"; WP-21 Orca pass. Make lead deliverables explicit predecessors; recast live checks as lead hand-back steps.
- R7 🟡 "Complete" gschema has no key definitions (667, 678; §13 495-502). Add a key table (name, type, default, range).
- R8 🟡 Pacing/backoff untestable as written (681; §4.6 243-247): give jitter range, burst size/window, backoff multiplier/cap, "unexpected redirect".
- R9 🟡 No operation inventory or bridge protocol (680-681; §4.2 202): add an op table (name, args, return model, write?), JS naming, error payload, timeouts, error mapping.
- R10 🟡 WP-04 storage design open (682; §14 509-514): schema, migrations, LRU unit, two DBs implied.
- R11 🟡 §8.1 "animated height transition" (389) contradicts WP-12 "never row height" (690).
- R12 🟡 `stores/filters*` ownership overlap; filter API for "N hidden" row unspecified (685, 697; §15 522).
- R13 🟢 Brief template's mock-scenarios line applies to non-UI WPs (724).
- R14 🟢 WP-20 deps "—" but screenshots need a working app; branding overlap with WP-00; CODE_OF_CONDUCT/README not owned (678, 698).
- R15 🟢 Rule 5 has no fallback when a11y tools are missing (652).

## egm-codebase-details (DetailedRange 642-735, plus cited sections) — FAIL, 4 Blocker / 9 Major / 2 Nit

- K1 🔴 `run_javascript` doesn't exist in WebKitGTK 6.0 / `webkit6` (202, 680-681). Use `evaluate_javascript` or `call_async_javascript_function`, passing the bridge's world name.
- K2 🔴 `ScriptWorld` is a web-process-extension API (203, 681). In the UI process isolation = a world-name string passed to `UserScript::for_world`, `register_script_message_handler("agora", Some(world))`, `evaluate_javascript(..., Some(world), ...)`. Name the world string.
- K3 🔴 WP-00 owns all of `data/` (678) but every UI WP must add `.blp` and per-module CSS under `data/resources/` (137, 650). Narrow WP-00; give each UI WP `data/resources/ui/<module>/*` and `style/<module>.css`.
- K4 🔴 `ShareMenu` (WP-18, wave 3) used by WP-09/11/12 (wave 2); `MediaView` (WP-13) mounted by WP-11 in the same wave without a dep (671, 686-690, 696).
- K5 🟡 "GTK 4.23" is a development series; GNOME 51 ships GTK 4.24 stable (94). Feature flag `v4_24`. libadwaita 1.10, AdwSidebar, GtkEnumList check out.
- K6 🟡 Meson has no glob; "generate gresource XML and POTFILES from globs" (654, 678) needs a named script and re-trigger rule; Blueprint batch-compile must feed compile_resources.
- K7 🟡 Unowned: `agora-reddit-web/Cargo.toml`, `lib.rs`; `agora-app/Cargo.toml`, `main.rs`, `config.rs.in`, `stores/mod.rs`, `services/mod.rs`, `render/mod.rs`. Path prefixes inconsistent; rule 7's `ui/<module>/mod.rs` vs flat globs vs `ui/share_menu.rs`.
- K8 🟡 WP-02 needs bridge API (Rust signature, JS calling convention, timeout, error shape) that isn't frozen while WP-03 runs in parallel (680).
- K9 🟡 `agora-bridge.js` "bundled in the gresource" but `agora-reddit-web` must pass plain `cargo test`; use `include_str!` (202, 681).
- K10 🟡 `Backend` trait: async fn in trait isn't object-safe; WebKit futures are !Send. Decide `async_trait(?Send)` / `LocalBoxFuture` / generics (655, 662).
- K11 🟡 CI needs the GNOME 51 SDK container (name it); WP-00 CI list vs §19 list differ (601, 652, 678).
- K12 🟡 `GTK_A11Y=test` doesn't report unlabeled widgets (601).
- K13 🟡 Screenshot procedure underspecified: adaptive preview doesn't force 360 px, no screenshot tool under xvfb, no `--mock` flag defined (652).
- K14 🟢 `cargo clippy -D warnings` → `cargo clippy --all-targets -- -D warnings` (652).
- K15 🟢 gschema `.xml.in` vs `.xml`; `.Devel` vs base app ID; `NetworkSession::new` also takes a cache dir (134, 200, 502, 667).

## egm-style (DetailedRange 642-735) — 0 Blocker / 3 Major / 5 Nit

- Y1 🟡 "Jared's session" (653, 710) vs "the lead" (680, 683): use one defined role.
- Y2 🟡 Unverifiable acceptance: "a day of normal use" (700), "icon per guidelines" (698), "Loupe-style viewer" (691).
- Y3 🟡 "without reading the whole conversation history" (644) assumes outside context.
- Y4 🟢 Mixed voice/idioms/caps: "Stay in your lane", "drive-by edit", "NOT" (649, 714-728).
- Y5 🟢 WP-12 implementation detail in summary column (690).
- Y6 🟢 Spelling/hyphenation: maximised/sanitised vs virtualized; "wave-2" vs "Wave 0" (667-708).
- Y7 🟢 "+" and "incl." shorthand; lowercase row start (682-700).
- Y8 🟢 Frozen `Backend` list ends "subscribe…" with "at minimum" (659, 662): frozen surface undefined. Wave 4 has no agent count; WP-20 "any wave" vs table Wave 4 (698, 708).
