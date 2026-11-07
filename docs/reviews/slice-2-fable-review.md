# Slice 2 review (Fable, independent)

**Reviewed:** `slice-2-real-reddit` at `47806e9`, diff `2985210..47806e9` (three commits: drift fixes,
`agora-reddit-web`, sign-in/Welcome/sign-out/subscriptions). Ground truth: `docs/SPEC.md` (§3.4, §4,
§5.4, §6, §14, §18.1, §19, §20), `docs/SPIKE-0.md`, `docs/STORES.md`,
`docs/reviews/2026-11-01_DRIFT_REPORT.md` (summary items 1–5), `crates/agora-reddit-web/STRATEGIES.md`.
**Date:** 2026-11-06.

**Checks run** (no traffic to reddit.com or any other host; the bridge test's server is `127.0.0.1`):
- `cargo test --workspace`: 135 tests pass; the bridge integration test (`tests/bridge.rs`, real hidden
  WebView against the local fixture server) prints `bridge: ok`.
- `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.
- Real backend, fresh settings, `unshare -rn` (no network), `AGORA_LOG=debug`, 10 s: the Welcome dialog
  path logs no `loading the home page` and nothing else; only WebKit's empty cache directories are
  created. Nothing is attempted before the acknowledgement.
- `AGORA_MOCK=1 AGORA_MOCK_WELCOME=1` under Xvfb: starts, no errors.
- Four probe tests on a scratch copy of the workspace (not committed), quoted below: two scheduler
  unit tests, and two extra cases in the bridge test that drive `login_view` against the local server
  (`/login` → 302 `/`, and `/` does one `history.pushState`). One extra probe compiled the quiet-page
  filter in real WebKit.

**Working tree note:** `.github/workflows/ci.yml` is modified and uncommitted (the lead's parallel CI
work); not reviewed.

Overall: the backend is carefully built. The bridge really is isolated (the probe page sees
`window.agora` and the handler as `undefined`), arguments go through a `VariantDict` and
`JSON.parse`, never into script text, `csrf_token` is read only inside the `agora` world and only
attached by `partialPost`, the `more_comments` `src` and every `after` cursor are checked against a
`/svc/shreddit/` prefix before use, one request is in flight at a time including the home page
load, pushback drains every queued request and persists the back-off across restarts, nothing is
queued while blocked, sign-out clears all WebKit data and `agora.db`, and nothing sensitive is
logged (op names, the fixed home URL and listing cursors only). The two A items are both in the
sign-in path, which is the first thing the live run will exercise.

---

## A. Must fix before live traffic

### A1. After a session expiry, signing in again can never succeed

`crates/agora-reddit-web/src/backend.rs:173-181` (`refuse`), `:296-320` (`load_home`, the
`this.refuse("home")?` at line 308), `:604-618` (`login_view`, `inner.home.set(Home::NotLoaded)` at
line 616).

`refuse` lets only `"whoami"` through while `expired` is set. `login_view`'s `uri_notify` handler
sets `home = NotLoaded` so the next call reloads the home page (correct: it was loaded signed out).
But `load_home` then calls `refuse("home")`, which fails with `SessionExpired`, so `whoami` never
gets past `ensure_home`. `expired` is cleared only by a successful `whoami` or by `sign_out`.

Probe (scratch bridge case: `whoami` ok, `listing("gone")` → `SessionExpired`, then `login_view`
whose callback spawns `whoami`, exactly as `AgoraSignIn` does):

```
PROBE expired-then-sign-in: navigations=2 whoami results=[Err(SessionExpired), Err(SessionExpired)]
      home loads=0 calls=["/login", "/"]
```

Scenario live: Reddit signs the session out; the banner says "Signed out of Reddit · Sign In"; the
person signs in on Reddit's page in the dialog; the dialog never closes (it closes only on
`SignedIn`), the banner stays, and every further Sign In press opens another dialog that loads
`/login` and the post-login home page in a visible, unfiltered view (see A2). The only ways out are
Sign Out (which deletes the saved subscriptions) or restarting Agora. The existing `signed_out`
bridge case does not catch this because it never resets `home`.

Fix: in `load_home` check only the block (`blocked_for`), not `expired`; the operation-level
`refuse(op)` in `call` already enforces expiry for the real request. For example give `refuse` an
`allow_expired: bool`, or treat `"home"` like `"whoami"`. Add the probe above as a bridge case
asserting `whoami` returns the name and exactly one `/r/popular/` load.

### A2. Reddit's page scripts in the sign-in view drive Agora's traffic, and the visible view runs Reddit's full front end until the check succeeds

`crates/agora-reddit-web/src/backend.rs:609-618` (`connect_uri_notify`), `:272-294` (`ensure_home`),
`crates/agora-app/src/ui/sign_in.rs:42-58`, `crates/agora-app/src/stores/session.rs:77-96` (`check`).

Three things combine:

1. `uri_notify` fires on every URI change that is not a login path, including `history.pushState`
   (confirmed by the probe: a page doing one `pushState` produced a second navigation). Reddit's
   front end is a single-page app and the post-login redirect is already two hops. Each change
   resets `home` to `NotLoaded` and calls `SessionStore::check`, so each costs a full hidden home
   page load plus `whoami`; the count is set by Reddit's scripts, not by a user action.
2. `home.set(NotLoaded)` while a home load is already `Loading` (line 616 vs the `Loading` check at
   line 276) starts a second concurrent `load_home`: two `load_uri` calls on the same view, the
   first `load_page` resolving on the second load's `Finished`.
3. The visible login view is built with no `UserContentManager`, so the quiet filter (`quiet.rs`)
   does not apply to it. After login it sits on Reddit's home feed with the chat client and
   trackers running, concurrently with Agora's paced requests, until `SignedIn` closes the dialog,
   or forever if `whoami` fails (`CheckFailed`) or A1 applies. The spike saw exactly this traffic
   reach HTTP 429 (`docs/SPIKE-0.md` §4).

Probe (fresh backend, `whoami` ok, then the same `login_view` with the redirect + one `pushState`):

```
PROBE sign-in navigations: navigations=2 whoami results=[Ok(Some("agora_tester")), Ok(Some("agora_tester"))]
      home loads=2 calls=["/login", "/", "/r/popular/", "/r/popular/", "/user/me/", "/user/agora_tester/",
      "/user/me/", "/user/agora_tester/"]
```

(`/login` and `/` are the visible view's own navigation; `/user/agora_tester/` is the followed
redirect of `/user/me/`. Two navigations cost four paced Agora requests where one of each would do.)

Fix, all four are small:
- `login_view`: fire `on_navigated` once per view (a `Cell<bool>` latch); the view exists to detect
  leaving the login page, nothing more. Do not reset `home` when it is `Loading`.
- `SessionStore::check`: ignore a call while a `whoami` is in flight (one `Cell<bool>`). This also
  bounds the `network_changed` retries.
- `AgoraSignIn`: once the page has left the login path, stop the visible view
  (`stop_loading()` and `load_uri("about:blank")`, or close the dialog right away and let the
  status page / banner show the result of the check). Reddit's home feed must not keep running in a
  visible view while Agora checks the session.
- Give the login view a `UserContentManager` carrying the same quiet filter (not the bridge
  scripts), so even its brief life on Reddit's pages does not poll `matrix.redditspace.com`.

Add a bridge case: a login page that redirects twice yields one home load and one `whoami`.

---

## B. Should fix in this slice

### B1. Loading more of the *same* feed cancels the open post

`crates/agora-reddit-web/src/scheduler.rs:102-113` (`enqueue`, `View::Feed` branch).

The `Feed` branch sets `self.thread = None` and drops every queued `Thread` request whether or not
the feed changed. The doc comment says "a new feed also closes the post pane", but the code does it
for the current feed too. Probe tests (scratch, both fail on the code as is):

```
probe_load_more_on_the_same_feed_while_a_post_loads:
  send page1 (feed gnome); send post_page (thread a); enqueue page2 (feed gnome)
  enqueue_step(thread a, "comments")  →  left: Err("comments")  right: Ok(())
probe_load_more_on_the_same_feed_drops_a_queued_post:
  send page1; enqueue post_page (thread a); enqueue page2 (feed gnome)
  dropped  →  left: ["post_page"]  right: []
```

Scenario: wide layout, the person opens a post (`post_page` in flight, `comments` step to come) and
scrolls the list to within 10 rows of the end; `FeedStore::load_more` enqueues `Feed("r/x")`; the
`comments` step is refused, `post_with_comments` returns `Cancelled`, and `ThreadStore` shows an
empty post pane with no error (`Cancelled` → `Idle`). `FeedStore::session_changed` →
`load_subreddit_info` after a block ends has the same effect on a reloading thread.

Fix: inside the `Feed` branch, only reset `thread` and drop `Thread` requests when
`self.feed.as_ref() != Some(id)`. The existing tests still hold; add the two probes.

### B2. The subscriptions partial's `src` is fetched unvalidated

`crates/agora-reddit-web/js/ops/subscriptions.js:4-9`, `js/agora-bridge.js:20-37`.

`listing` and `search` cursors are accepted only when they start with `/svc/shreddit/`
(`parse/listing.rs:34`, `parse/search.rs`), and `more_comments` checks
`/svc/shreddit/more-comments/` (`backend.rs:407`). `subscriptions` takes whatever
`faceplate-partial[src*="common-left-nav"]` carries on the hidden page and passes it to `fetch`
with `credentials: 'include'`; an absolute URL would be fetched as is. The document is reddit.com's
own page, so this needs a page-side injection to matter, but the check is one line. Fix: in the op,
require `src.startsWith('/svc/shreddit/')`, otherwise return `{status: 0, error}`; in Rust, require
`reply.final_url` to start with `config.origin`. For defence in depth, `partialPost` (which attaches
`csrf_token`) should refuse any URL that is not path-relative (`/x`, not `//x` or a scheme).

### B3. `post_page` takes the first `<shreddit-post>` on the page, not the requested one

`crates/agora-reddit-web/src/parse/mod.rs:42-47`, `backend.rs:383`.

`post_page(html)` returns the first `<shreddit-post>` with an id and title. A real post page may
carry other `<shreddit-post>` elements (recommendations, "more posts" rails); the bridge test only
serves the *listing* fixture for `/r/gnome/comments/…/` and passes because the wanted post is first.
There is no post-page fixture yet. Fix: pass `&request.post` and select
`shreddit-post[id="t3_…"]`, falling back to `ExtractorBroken`; record a scrubbed post-page fixture in
the live run (`docs/SPEC.md` §20.1 rule 5).

### B4. `ensure_home` has no drop guard

`crates/agora-reddit-web/src/backend.rs:272-294`.

If the future running `load_home` is dropped while `home == Loading` (nothing in the app drops a
backend future today, but any later `select`/timeout in a store or the self-test would), `home`
stays `Loading` forever and every later call parks in `home_waiters` with nothing to wake it. Fix: a
guard struct that resets `home` to `NotLoaded` and fails the waiters with `Cancelled` on drop (as
`Permit` does for the scheduler).

### B5. The home page load accepts a redirect to `/login` and status 0 as success

`crates/agora-reddit-web/src/backend.rs:311-319`, `:262-268` (`on_home_page`).

`load_home` treats anything except 429, 403 and 5xx (including 0, 404 and a 200 at
`/login/?dest=…`) as a loaded home page, and `on_home_page` only checks the origin prefix. If
Reddit redirects a signed-out `/r/popular/` to the login page, every partial is then sent from a
login document as Referer; the spike showed page context is what triggered the blocks
(`docs/SPIKE-0.md` §4). Fix: after `Finished`, require a 2xx status and `!is_login_url(view.uri())`;
otherwise set `NotLoaded` and return `SessionExpired` / `Network`.

### B6. The feed shows nothing while the first session check runs

`crates/agora-app/src/stores/feed.rs:176-188`.

With the acknowledgement already given, start-up is: home page load, gap, `whoami`, gap, listing,
about 5–8 s. During that time `load_more` returns early with `refusal() == None` and the state stays
`Idle`, so the list is empty with no spinner. `ThreadStore::reload` already does the right thing
(`None => set_state(Loading)`); do the same here.

---

## C. Noted

- **Quiet filter compiles in real WebKit** (probe: `save_future` → `Ok`), so the `[^/]` class is
  accepted. The app only logs a warning if compilation fails; the bridge test could assert it.
  STRATEGIES.md does not yet record the filter choice (SPEC §4.2), and still lists Home/Popular as
  "not tried"; the lead is updating docs in parallel. The first live run should also count the
  hidden page's own requests while idle (telemetry, prefetch) to confirm the filter list is enough.
- **Expiry does not drain the queue.** `logged_out` (`backend.rs:215-220`) sets `expired` but queued
  waiters each still take a turn, are refused at `backend.rs:189`, and burn a full gap sending nothing
  (`Permit::drop` → `finished`). With a page, `subreddit_info` and a thread queued the UI waits 3–4 s
  for its errors. Drain like `pushback` does, failing waiters with `SessionExpired`.
- **Reconnection burst after a block.** `check` → `SignedIn` triggers `FeedStore` reload +
  `subreddit_info`, `ThreadStore` reload (two requests) and `SubscriptionsStore` refresh: up to six
  paced requests in about 8 s with no user action, right after Reddit said slow down. The spec's
  doubling back-off bounds it, but consider reloading only the list pane and leaving the post and
  subscriptions for the next user action.
- **Sign-out and an in-flight request.** `sign_out` drains the queue and loads `about:blank`, which
  aborts a running `fetch`; that request then fails with `Network` and the store toasts "network
  error" after a successful sign-out. Cosmetic; a `signing_out` flag mapping it to `Cancelled` fixes it.
- **Comment tree parser never reports `ExtractorBroken`.** `parse::comments::parse` returns an empty
  `Vec` for any HTML, so a 200 full-page shell (the 406 behaviour STRATEGIES.md describes) shows "no
  comments" rather than an error. Return `broken("post_with_comments")` when the body has neither
  `shreddit-comment` nor a `shreddit-comment-tree` marker.
- **Blocked state has no duration for the UI.** `SessionStore::refusal()` returns
  `retry_after: ZERO`, so a page error cannot say "for 5 minutes"; keep the backend's `retry_after`
  in the store.
- **`SessionStore` handlers accumulate.** `AgoraSignIn::new` and `AgoraWelcome::new` connect
  `state_notify` on the long-lived store with weak refs and never disconnect; harmless no-ops after
  the dialog dies, but one per dialog opened. Disconnect in `unrealize`/`closed`.
- **Blocked timer and the backend remainder.** `enter_blocked` fires `check` when its timer ends;
  if the backend's `blocked_until` has a sub-millisecond remainder, `refuse` returns a tiny
  `retry_after` and the timer re-fires a few times within a millisecond. No traffic, no notifications
  (same state), so harmless; rounding `retry_after` up to the next second would remove it.
- **`load_subreddit_info` has no in-flight guard**; `set_feed` and `session_changed` can both issue
  it. Results are checked by object identity, so the only cost is a possible duplicate request.
- **Welcome timing.** Continue starts traffic (home page load, `whoami`) before the person presses
  Sign In. Allowed by SPEC §6 (after the acknowledgement); the first two requests to reddit.com are
  therefore a signed-out `/r/popular/` and `/user/me/`.
- **Two requests per post open.** `thread` fetches the full post page and then the comments partial
  (the spec's burst example). The listing partial carries no `text-body` (fixture confirmed), so the
  page is needed for text posts, but it is a full Reddit page for every post including link and image
  posts. Worth measuring the size live; a lighter source for the body may exist.
- **`BACKOFF_RESET`** (a quiet hour after a back-off ends restarts doubling at 5 min) is a sensible
  addition the spec does not mention; add it to §4.6.
- **Fixture coverage.** Parsers hold up against every recorded fixture (`listing`, `post_comments`,
  `more_comments`, `subreddit_info_right_rail`, `subscriptions_left_nav_empty`, `user_profile_own`,
  `search_posts`, `blocked_network_security`). Missing fixtures: a post page (B3), Home/Popular/user/
  saved partials, a non-empty left-nav JSON, a `/notifications` page with entries, and a 429 body.
  These are the known provisional parts; record them in the first live run.
- **UI flow** read against SPEC §4.4, §5.4, §6, §14: Welcome gates Continue on the check button and
  stores `risk-acknowledged`; the carousel is non-interactive so page 2 is unreachable without
  Continue; closing Welcome early leaves the "Sign in to Reddit" status page whose button reopens
  Welcome; the Expired banner opens the sign-in dialog directly; Sign Out confirms, clears WebKit data,
  `agora.db` and every store, and shows the status page; mock mode is real (`AGORA_MOCK`,
  `AGORA_MOCK_WELCOME`, `AGORA_MOCK_SIGNED_OUT`, `AGORA_MOCK_FAIL`, `AGORA_MOCK_LATENCY_MS`). The
  Sign In dialog's close-on-`SignedIn` and the Welcome's scroll-to-done work in mock mode; live they
  depend on A1 and A2.
- **Drift report items 1–5** are addressed: `ThreadRequest.subreddit` and `open_id(subreddit, id)`;
  Join disabled until the id loads (`Writes::toggle_subscribe`, tested); every store and write gated
  on `can_request()` and reloading on `SignedIn`; the window no longer calls the backend; `Unknown`
  resolves to `CheckFailed` with a Retry banner and `Blocked` leaves by a timed `check`.
