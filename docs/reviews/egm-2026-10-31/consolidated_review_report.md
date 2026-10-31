# Agora spec: EGM Goldfish review, consolidated

**Target:** `docs/SPEC.md` on the PR #1 branch (`spec-review-updates`), 735 lines. **Date:** 2026-10-31.
**Mode:** EGM "consolidated report" preset. Five stages ran in order, using 11 fresh reviewers. Each reviewer started with no prior context and was told not to read the earlier Fable review. Every requested reviewer completed.

| Stage | Reviewers | Verdict | Report |
|---|---|---|---|
| 5 Zoom out | zoom-out | REVISE: 1 🔴, 6 🟡 | [zoom_out_report.md](zoom_out_report.md) |
| 6 Comprehension | comprehension | Understandable; 5 🟡, all in §22 | [comprehension_report.md](comprehension_report.md) |
| 7 Critic | design, structural, style | NOT READY: 3 🔴, 23 🟡 | [critic_report.md](critic_report.md) |
| 8 Zoom out again | zoom-out | REVISE: rules on the conflicts, adds 2 🟡 | [intermediate_zoom_out_report.md](intermediate_zoom_out_report.md) |
| 9 Implementation readiness | readiness, codebase details, style | NOT READY: 10 🔴, 18 🟡 | [implementation_readiness_report.md](implementation_readiness_report.md) |

**Bottom line.** The product design (what Agora is and how it looks) is understood and mostly holds up. The problems sit in three places:
- **(a) Build order.** Contracts are frozen before anyone has shown the browser-session approach works.
- **(b) Unneeded robustness and scope.** Examples are the three-strategy fallback, the offline cache, background checks, and capability flags.
- **(c) §22 is not ready to hand to fresh agents.** Contracts are sketches, dependencies are inverted, ownership has gaps, and some WebKit API names are wrong.

Most of (c) shrinks once (a) and (b) are settled, so those decisions come first.

---

## A. Decisions for Jared (they change scope or earlier decisions)

These are the items I would escalate to you, in the order I'd ask them, one at a time.

1. **Defer the offline cache and background checks to "Later".**
   - Sources: Stage 5 M5, Stage 8 rulings, and N-M2.
   - With no prefetch allowed, the offline cache only re-shows posts you have already seen. It still costs a second render path, eviction rules, a purge of deleted content, and an extra owner in §22.
   - Background mode keeps a hidden reddit.com tab alive after the window closes.
   - What stays: drafts, plus a small saved subscription list for the Shell search provider.
   - My recommendation: **defer both.**
2. **Use one extraction strategy per operation, chosen from a live spike. No click-driven writes.**
   - Sources: Stage 5 M1 and Stage 8; this removes D6 and D7.
   - The three-strategy chain triples parser and fixture work. Clicking Reddit's own buttons in a hidden view can act on the wrong item.
   - My recommendation: **accept.**
3. **Work breakdown: keep 23 small WPs in waves, or switch to fewer vertical slices.**
   - Sources: Stage 5 M4, Stage 6 items 1 and 3, Stage 9 R3–R5, K3, K4 and K7.
   - Most Stage 9 blockers are seams between packages that run in parallel.
   - The slices would be: shell, feed and post view against the mock; then session and extractors; then interaction; then the integrations. Contracts freeze after the first slice works end to end.
   - My recommendation: **vertical slices.**
4. **Drop the `Capabilities` flags and the neutral `Community` naming, and keep the `Backend` trait as the mock seam.**
   - Sources: Stage 5 M2, R2, and N-N2 (which also drops the single-impl `Cache` trait).
   - This partly reverses "keep the backend pluggable". The trait stays; only the flags for backends that will never exist go away.
   - My recommendation: **accept.**
5. **Media path: fetch through libsoup with an honest User-Agent, falling back to WebKit's own network session if the CDN refuses. Drop tokio and reqwest.**
   - Sources: Stage 5 M3 and M6, D2, Stage 8.
   - Copying the WebView's User-Agent into rustls is a classic bot signal and conflicts with "no circumvention".
   - My recommendation: **accept.**

## B. Proposed fixes (valid findings; apply if you say "auto accept")

**Build order and contracts**
- **B1 spike before WP-01.** The lead runs listing, post with comments, more comments, vote, comment and whoami against a live session. Results go into `STRATEGIES.md`, along with these four things:
  - a go/no-go threshold (N-N3);
  - whether a hidden view gets throttled (D5);
  - where the CSRF token comes from (D4);
  - two request lanes: interactive and background, with superseded clicks dropped (N-M1).
- **Freeze real contracts in §22.2** from what the spike finds:
  - trait signatures, the `?Send` async decision and the event mechanism (R1, K10);
  - the operation table and bridge protocol (R9, K8);
  - the store list and action names (R3);
  - the AST node list;
  - a gschema key table (R7);
  - a pacing constants table (S7, R8).

**Correctness**
- **WebKitGTK 6.0 API.** Use `call_async_javascript_function` with typed arguments and a named world string, and register the message handler only in that world (K1, K2, D3). Load `agora-bridge.js` with `include_str!` (K9).
- **Version.** GTK **4.24** (4.23 is a development series), stated in one place only (K5, N2, S2).
- **Privacy.** Allow images from Reddit's CDNs only; everything else loads on click. Drop link-card favicons (D8, N7).
- **Writes.** Confirm a write from the site's own response, revert the optimistic update on error, and remove the read-back (N7).
- **`/api/morechildren`.** Route it through an `ops/more_comments.js` operation (D10, N6).
- **Deleted content.** Withdraw any notification already shown for deleted content (D11). Weaken the "never displays" wording.
- **Thread lines.** §8.1 accent on hover *or focus*, and no height animation on recycled rows (T14, R11).
- **Tooling.**
  - The clippy command (K14).
  - The `GTK_A11Y=test` claim (K12).
  - The CI container and a single CI list (K11).
  - A Meson glob script (K6).
  - gschema and app ID naming (K15).
  - A `--mock` flag (K13).

**Document structure**
- Add a short **Problem statement** and an **Alternatives considered** table. It should cover: the OAuth API until March 2027, plain HTTP with cookies, a browser extension, RSS, a visible tab, and a hidden WebView. It replaces the reliance on SPEC-REVIEW.md, with primary sources and dates (D1, T1, T3, D9, S8).
- Add one **Risks and mitigations** table that pulls together the scattered risk text: account ban, breakage, JS bridge, cookies on disk, scale of large threads (T2).
- Restate Goal 6 as "no more traffic than a person browsing reddit.com; conforms to §4.6" (M6, D2, S4).
- Add "I understand the account risk" to the existing Welcome disclaimer page (D12).
- Merge §20 and §22.4 into one plan (C9, T13).
- Delete §8.3 (multi-account) and note that it would need one NetworkSession per account (N-N1).

**Consistency and wording** (cheap)
- §12.2 and §12.3 become "Decided" (C5, S1, T4).
- Use "the user" instead of "Jared" outside the project-process text (N5, S3, T7, Y1).
- Remove "memory decision" and "conversation history" (S9, T8, Y3).
- Make the Weblate wording match (N3, T6).
- Remove the Flathub leftovers (N4, T12).
- Use American spelling.
- Use "trait stays free of Reddit types" instead of "clean" (S6).
- Fix the repo tree (T11) and the §18 numbering (T9).
- Make acceptance criteria measurable (S4, Y2).

**§22 mechanics** (only if A3 keeps the 23-WP model; otherwise rewritten under slices)
- Correct the Deps column (R4, K4).
- Assign owners for the unowned files and cross-cutting work (R5, K3, K7).
- List lead deliverables as explicit predecessors (R6).
- Scope the brief template (R13).

## C. Declined (Nits, recorded only)

- Mood words in the brand section (S5).
- Implementation detail in the architecture sections (N1, Y5). These move naturally into the WP briefs.
- Filler words (S10) and shorthand (Y7), unless they are touched anyway.

## Contested

None. On GTK 4.23, project memory recorded "GTK 4.23" as decided. The reviewer's point that GNOME 51 ships 4.24 stable matches GTK's odd and even numbering, so this is listed as a proposed fix rather than contested.
