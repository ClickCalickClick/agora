# Stage 8: Intermediate Zoom Out Review

Target: docs/SPEC.md full file. Reviewer: egm-zoom-out (fresh context), with Stage 5-7 reports. Verdict: REVISE (0 new Blocker, 2 new Major, 3 new Nit; Stage 5 B1 stands).

## Part A: rulings on Stage 6/7 asks vs Stage 5 simplicity cuts

**Accept (cheap, reduce ambiguity):** all wording/consistency fixes (C5, C11, D13, D14, T4-T14, S1-S3, S6, S8-S12). One short problem + alternatives section (~5-row table) replacing reliance on SPEC-REVIEW.md (D1/T1, D9/T3). Risks table only if it *consolidates* scattered risk text (T2). Typed JS args via `call_async_javascript` (D3), no fuzz test. Privacy: Reddit CDNs only, others on click, drop favicons (D8). Honest-traffic goal (D2 = M6). One pacing constants table in §4.6 (S7). Self-test inside pacing budget (D16). Isolation wording fix (D4); CSRF source is a spike output.

**Reject / absorbed by Stage 5 cuts:**
- D6, D7 (separate DOM view, click safety): M1 wins; no strategy 3 or click-writes unless the spike requires them.
- Read-back after writes: N7 wins; confirm from the site's own response, revert optimistic update on error.
- D5: keep throttling check as a spike item; reject recycle/memory budget; background concern gone under M5.
- C4, D11 (cache ownership, offline deleted content): M5 wins (defer cache). Keep drafts, keep `withdraw_notification`, weaken L512.
- D15: with cache deferred, no threading spec needed.
- C1, C3 (same-wave deps, ownership gaps): symptoms of M4; prefer fewer vertical-slice WPs, else minimal fixes.
- C2 (STRATEGIES.md owner): resolved by B1 spike in wave 0.
- C9, T13: merge §20 and §22.4 into one plan.
- D12: fold an "I understand" control into the existing Welcome disclaimer page.
- S4 Goal 6 accepted; S5 mood words optional.

**Media path settles D2, M3, M6 together:** fetch media with an honest default UA via libsoup (souphttpsrc already uses it); if the spike shows the CDN refuses, fetch via WebKit's network session. Either way tokio/reqwest, the `rt.rs` bridge and UA copying go away.

## Part B: new findings

- 🟡 **N-M1. Single request queue has no priority or cancellation; frozen trait can't express it** (L243, 246, 235, 210, 662). Clicks, scroll pages, inbox polls, self-test, writes share one ~1 req/s queue; stale backlog makes clicks wait seconds. Fix: interactive and background lanes; drop superseded interactive requests before sending; background yields. Put cancellation in the `Backend` contract decided after the spike.
- 🟡 **N-M2. Deferring the cache breaks the Shell search provider** (L424, 163, 173). Persist only the subscription list (small store or GSettings strv); drop "cached popular subreddits".
- 🟢 N-N1. §8.3 multi-account list is speculative; real constraint is one WebKit NetworkSession dir per account (L33, 400, 200). Delete §8.3, note that.
- 🟢 N-N2. `Cache` trait has one impl (L662); drop it.
- 🟢 N-N3. Spike has no go/no-go criterion (L637, 704): state when to rescope or stop if JSON isn't available for core reads.

## Net guidance

Apply first: (1) B1 spike before contracts, with N-M1 and N-N3; (2) M1 one strategy per op; (3) M5 defer offline cache and background mode, with N-M2; (4) M6 honest-traffic goal with the media-path decision. Then the accepted wording/consolidation items.
