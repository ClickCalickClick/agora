# Contracts changelog

Changes to the frozen contracts after slice 1: the `Backend` trait and models in
`crates/agora-core`, the operation table (SPEC §4.5), `docs/STORES.md` and the
gschema keys (SPEC §13). Each entry needs Jared's sign-off (SPEC §20.1 rule 3).

| Date | Change | Why | Signed off |
|---|---|---|---|
| 2026-11-01 | Contracts frozen at the end of slice 1 | — | not recorded; Jared decides with the entries below |
| 2026-11-06 | `ThreadRequest` gains `subreddit`; `ThreadStore::open_id` takes the subreddit | Every comment endpoint names it (drift report item 1) | pending (slice 2 PR) |
| 2026-11-06 | `Backend::subscribe` takes the `t5_…` id instead of a whole `Subreddit` | Reddit needs only the id (item 7) | pending (slice 2 PR) |
| 2026-11-06 | `Comment` gains `permalink` | The parser reads it, and Copy Link needs it (item 7) | pending (slice 2 PR) |
| 2026-11-06 | `AgoraError::NotAvailable` | Writes reach the real backend in slice 4; removed then | pending (slice 2 PR) |
| 2026-11-06 | `SessionState::CheckFailed`; `SessionStore::refusal()`, `sign_out()`; `Writes::toggle_subscribe` | A failed check must be visible; views must not call the backend (items 4, 5) | pending (slice 2 PR) |
| 2026-11-06 | `win.toggle-subscribe` loses its parameter; adds `win.sign-in`, `win.sign-out` | The parameter was ignored; sign-in UI | pending (slice 2 PR) |
