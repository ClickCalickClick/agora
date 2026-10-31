# Stage 5: Initial Zoom Out Review

Target: docs/SPEC.md (PR #1 branch, lines 1-735). Reviewer: egm-zoom-out (fresh context). Standard: zoom-out-standard.md.

Context sufficiency: PASS. Verdict: REVISE (1 Blocker, 6 Major, 7 Nit).

## 🔴 Blocker

**B1. Core contracts are frozen before the data-access approach is shown to work.** §22.1 rule 3 (L650), §22.2 (L657-664), WP-01/02 (L679-680), Wave 0 (L704), §4.5 (L233), §21 (L637). Whether JSON, `/svc/shreddit` partials or DOM extraction works is only learned by WP-02 in wave 1, but WP-01 freezes `Post`, `Comment`, `MoreComments`, `Media`, `Capabilities` and `Fullname` IDs in wave 0. If JSON is gone for some ops, fields, body format (HTML vs Markdown) and IDs change after six agents coded against them. Fix: add a feasibility spike by the lead against the live session (listing, post+comments, more comments, vote, comment, whoami), record it in `STRATEGIES.md`, then freeze contracts from observed data.

## 🟡 Major

**M1. Three-strategy fallback for every operation, plus UI automation for writes.** §4.5 (L225-237), WP-02 (L680), HTML-to-AST path (L112, L237, WP-05 L683), DOM fixtures (L597, L653). Triples extractor/parser/fixture code and forces a second body format. Spec already accepts hand fixes on breakage (L184). Pick one strategy per op from the spike; add a fallback only when it breaks. Driving the page's own buttons is hardest to keep within "no circumvention / human pace".

**M2. `Backend` trait `Capabilities` flags and neutral naming serve out-of-scope backends.** Goal 7 (L25), non-goal (L36), §4.3 (L206-214), §22.2 (L662), WP-01 (L679). With one backend every flag is true: dead UI logic with tests. Keep the trait (seam for `MockBackend`), drop `Capabilities` and the `Community` renaming; use Reddit terms.

**M3. Second async runtime (tokio + reqwest) when GNOME already ships libsoup 3.** §3.2 (L106), §4.6 (L248), §21 item 2 (L627), WP-06 (L684). Use `soup3` on the GLib main context; removes the thread bridge and a separate TLS stack.

**M4. Parallel-agent process is heavy for a personal, self-built app.** §3.3 (L120), §22 (L642-729), rule 7 (L654), §22.2 contracts + STORES.md + CONTRACTS-CHANGELOG (L650, L666-670), §22.5 (L712-729). Several contracts exist only to keep agents apart (`MediaView::new`, `ShareMenu::for_url`, full gschema, action list). Consider fewer, larger vertical-slice packages (shell+feed+post view on the mock first, then session/extractors, then interaction) and freeze contracts after the first slice works end to end.

**M5. Offline cache and background service are large for 1.0.** §3.4 (L162-173), §14 (L506-514), §13 (L499), §12.4 (L481), §10 (L424), WP-04/16/17. Offline only shows what was already seen (no prefetch), yet costs a second render path, eviction, deleted-content purge. Background mode keeps a hidden browser alive after close (account risk). Move both to "Later"; keep drafts.

**M6. Goal 6 "indistinguishable traffic" reads as detection evasion and conflicts with no-circumvention.** Goal 6 (L24), §4.6 (L241, L248), §7.4 (L360) vs §18 rule 7 (L566). Copying the WebView UA into non-browser clients to avoid a 403 is evasion. Reword Goal 6 to "no more traffic than a person browsing reddit.com"; fetch media through WebKit's own network session so no UA copying is needed.

## 🟢 Nit

- **N1.** Implementation detail in architecture sections: L106 `spawn_tokio`, L330 border radius, L388 indent/colour, L237 HTML tag list, WP-12 prescribes `GtkRevealer`.
- **N2.** "Single version line" (L94) broken: GNOME 51 repeated at L360, L629.
- **N3.** Weblate stated two ways: §17 (L544) vs §21 item 5 (L630).
- **N4.** Distribution artifacts despite non-goal: AppStream branding (L75), Circle bar (L20), screenshots (WP-20), `X-Purism-FormFactor` (L582), name clash (L58, L638). Say they're kept on purpose or cut.
- **N5.** "Jared's session / Jared's risk" (L181, L185, L218, L599, L653, L710) while source is public: say "the user" outside §22.
- **N6.** §8.1 `/api/morechildren` (L391) and `Fullname (t3_…)` (L660) assume JSON strategy; same root as B1.
- **N7.** Read-back after every write (L231) doubles write traffic and overlaps optimistic updates (WP-07); link-card favicon (L345) hits arbitrary domains, contradicting privacy row (L583).
