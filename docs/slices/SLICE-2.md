# Slice 2: reading real Reddit

**Goal:** signed in, the user browses Home, Popular, subreddits and threads, including load more,
from real Reddit at human pace. See SPEC §4, §6, §14, §20.

## How to run

```bash
cargo run -p agora-app                # real Reddit; first run shows the Welcome dialog
AGORA_MOCK=1 cargo run -p agora-app   # sample data, no network
cargo run -p agora-app -- --self-test # every read once, about 15 seconds, signed in
```

`AGORA_MOCK_SIGNED_OUT=1` starts mock mode signed out, and `AGORA_MOCK_WELCOME=1` shows Welcome in
mock mode. Live runs follow SPEC §20.1 rule 5: only with Jared present.

## Exit checklist

- [x] Slice 1 drift report summary items 1 to 5 fixed
- [x] Request lanes and pacing, time-mocked tests for every value in SPEC §4.6
- [x] Bridge integration test: real hidden WebView against a local server playing fixtures
- [x] Welcome with risk acknowledgement; no traffic before it
- [x] Embedded login, sign out, signed-out page, expired banner, blocked banner that clears
- [x] Subscriptions saved in `agora.db`
- [ ] Live: sign in on Reddit's page
- [ ] Live: Home, Popular, a subreddit, a thread, load more
- [ ] Live: `--self-test` passes
- [ ] Keyboard and Orca walkthrough of the new screens
- [ ] Contract changes signed off in `docs/CONTRACTS-CHANGELOG.md`
