# Stage 7: Critic Review (design, structural, style)

Target: docs/SPEC.md FeatureRange 1-641, standard feature-spec-standard.md. All three reviewers completed.

## egm-design (FeatureRange 1-641) — NOT READY, 2 Blocker / 9 Major / 4 Nit

- D1 🔴 No Alternatives section (177-185). Unweighed: plain HTTP to .json/old.reddit with session cookies, browser extension/userscript + native app, RSS/Atom read-only, official API until March 2027 then migrate, visible browser tab. Compare cost, breakage, account risk, detectability, complexity.
- D2 🔴 "Indistinguishable traffic" goal unachievable and conflicts with no-circumvention (24, 201, 227, 241, 248, 566). `fetch()` of .json from a never-rendered page isn't browser-like; rustls media fetches with copied WebKit UA = UA/TLS fingerprint mismatch (classic bot signal). Restate as human-paced low-volume; fetch media via WebKit or drop UA copying; document ban risk honestly.
- D3 🟡 JS injection via `run_javascript("agora.fetchFeed(...)")` (192, 202) with user/Reddit text as args. Require `call_async_javascript` with typed/JSON args + fuzz test; register `agora` handler only in isolated world.
- D4 🟡 Isolation claim overstated (203): isolated-world fetches still visible to service worker/server logs. CSRF token source from isolated world unspecified (231).
- D5 🟡 Hidden WebView lifecycle (201, 204, 481): never-shown view may be throttled/suspended, stalling the bridge; background mode keeps full SPA (telemetry, websockets, hundreds of MB) alive after close. Spike it in WP-02; load a lightweight same-origin page; recycle view; memory budget.
- D6 🟡 DOM strategy navigates the single bridge view (201, 229, 243): tears down in-flight requests, re-injection, dozens of sub-requests breaks one-at-a-time. Use a separate view; define queue behaviour; weight page loads in pacing.
- D7 🟡 Click-based writes risk acting on the wrong item (231, 543). Verify target by thing ID before clicking; forbid delete/report via clicks; define read-back settle window.
- D8 🟡 Privacy claim contradicted (237, 345, 379, 583): sanitiser allows `img`, Markdown images, favicons from any domain. Allow only Reddit CDN; others on click; or fix the claim.
- D9 🟡 Problem statement relies on SPEC-REVIEW.md (9, 181, 625); r/all removal (280) unsourced. Summarise evidence with primary sources.
- D10 🟡 `/api/morechildren` (391) is the API the spec says it avoids; route through `ops/more_comments.js` and STRATEGIES.md.
- D11 🟡 "Never displays deleted content" only holds online (510-513); notifications already shown aren't withdrawn (479). Weaken claim or label offline content; specify `withdraw_notification`.
- D12 🟡 Account risk assigned to "Jared" though any user can build it (181, 185, 233, 241, 599); add first-run account-risk acknowledgement.
- D13 🟢 §12.2/§12.3 still "Open question"; Flathub mention (75).
- D14 🟢 Runtime version repeated (360, 629).
- D15 🟢 Thread for rusqlite and draft autosave unspecified (110, 204, 509).
- D16 🟢 CDN-403 claim unproven (360); self-test should stay inside pacing budget (235, 243).

## egm-structural (FeatureRange 1-641) — FAIL, 1 Blocker / 7 Major / 6 Nit

- T1 🔴 No "Alternatives Considered" section. Central decision (hidden WebKit session vs OAuth API) justified only by a timeline sourced to SPEC-REVIEW.md. Not evaluated: plain HTTP scraping with a cookie jar, browser extension/userscript, third-party API/mirror, not building. Smaller choices too (native Markdown widgets vs WebView renderer §7.6, SQLite §14).
- T2 🟡 No consolidated "Risks & Mitigations" (risks scattered at 58, 184-185, 247, 566, 569). Security failure modes unanalysed: JS bridge surface (203), CSRF reuse (231), on-disk cookie profile (200), background web view after close (481). No scalability limits (thousands of comments at ~1 s pacing; morechildren fan-out).
- T3 🟡 No "Problem Statement" (7-11, 181): never says what's wrong with reddit.com in a browser or existing clients.
- T4 🟡 §12.2/§12.3 still "Open question" vs §21 settled (466, 474 vs 628-629).
- T5 🟡 Version single-source rule broken (94 vs 360, 629).
- T6 🟡 Weblate contradiction (544 vs 630); §21 files it under "Hosting".
- T7 🟡 "Jared" in normative text (181, 185, 218, 233, 241, 599).
- T8 🟡 "The memory decision" (466) references outside context.
- T9 🟢 §18 H3 numbering inconsistent (552, 571, 585).
- T10 🟢 "8.3 Data model note" (accounts) misplaced under Comments (398-400).
- T11 🟢 Repo tree draws `crates/agora-app/src/` as top-level (125-144).
- T12 🟢 "Software/Flathub pages" (75) vs no-Flathub.
- T13 🟢 Roadmap "Later" row has empty cells; phase-to-wave mapping overlaps (608, 619).
- T14 🟢 Thread line accent: §2.1 "hover or keyboard focus" vs §8.1 "hover" only (78 vs 388); §16 forbids hover-only.

## egm-style (FeatureRange 1-641) — Changes requested, 0 Blocker / 7 Major / 5 Nit

- S1 🟡 §12.2 (466), §12.3 (474) still "Open question for review" though §21 (628-629) says settled; drop "Recommendation:" (470).
- S2 🟡 Version stated beyond L94: L360 "GNOME 51 runtime", L629 "GNOME 51". Use "the target runtime".
- S3 🟡 "Jared" used for any user (181, 185, 218, 233, 241, 599). Use "the user"; keep Jared for project tasks.
- S4 🟡 Unmeasurable goals/exit criteria (19, 22, 24, 241, 614, 615, 617): "first-party feel", "indistinguishable", "daily-driver", "common post types" (list them), "all integrations working". Restate Goal 6 as "conforms to §4.6 pacing rules".
- S5 🟡 Subjective/promotional brand wording (62, 69, 75, 78, 79, 82, 84, 86, 184). Keep concrete rules; move mood words to a labelled design-intent note; reword outcome promises (86, 184) as intent.
- S6 🟡 "clean `Backend` trait" (25, 626) → "trait stays free of Reddit types" (214).
- S7 🟡 Vague numbers in compliance rules (243, 246, 362, 412, 275): ~1 s + small jitter, short bursts, overnight-length idle, ~30 s, every few seconds. Pacing has a unit test; give exact min gap, jitter range, burst size.
- S8 🟡 Unsourced facts driving design (181, 280, 360, 474): API closure dates, r/all removed April 2025, CDN 403 without browser UA, no Share portal. Add source or "(observed YYYY-MM-DD)".
- S9 🟢 "The memory decision" (466) refers to outside context.
- S10 🟢 Filler: "single source of truth", "Concretely", "Deliberately", "These are requirements, not suggestions", inline "Decided." notes (11, 64, 73, 106, 200, 466, 511, 554, 337).
- S11 🟢 Comma splice in composer post-type list incl. Poll exclusion (408).
- S12 🟢 Mixed British/American spelling (GNOME uses American); "Software/Flathub pages" (75) despite no Flathub; Weblate stated two ways (544 vs 630).
