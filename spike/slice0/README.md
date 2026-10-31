# Slice 0 spike (throwaway)

Prototype used once, live, to answer SPEC §20 slice 0. Results: [`docs/SPIKE-0.md`](../../docs/SPIKE-0.md).
Not part of the app; slice 2 rebuilds the bridge in Rust in `crates/agora-reddit-web`.

- `spike.py` — GTK app holding a hidden WebKitGTK 6.0 view, bridge in the isolated world `agora`,
  pacing (≥ 1 s + jitter) and a block latch. Driven one command at a time through a FIFO.
- `ctl.py '<json command>'` — sends a command and prints the result.
- `bridge.js` — `agora.run(op, args)`: paced `get`/`post` (form or GraphQL JSON with the CSRF cookie), `env`, `timers`.
- `recorder.js` — page-world recorder of the front end's own write requests while a person clicks by hand (secrets redacted).
- `dom_*.js` — DOM reads used during the run (no network).
- `fixture.py`, `scrub.py` — turn raw bodies into scrubbed fixtures.

Profile, cookies and raw bodies live in `~/.local/share/agora-spike/` and must never be committed.
Only run it with the account owner present, on a throwaway account, at human pace (SPEC §20.1 rule 5).
