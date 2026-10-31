# Slice 0 spike: go/no-go report

**Date:** 2026-10-31 · **Run by:** the slice 0 lead with Jared, live on Jared's CachyOS machine
**Account:** a throwaway account created for the spike
**Engine:** WebKitGTK 2.54.2 (WebKit 6.0 API) through PyGObject, GTK 4.24, libadwaita 1.10
**Code:** [`spike/slice0/`](../spike/slice0/) (throwaway) · **Per-operation result:** [`crates/agora-reddit-web/STRATEGIES.md`](../crates/agora-reddit-web/STRATEGIES.md)

## 1. Verdict

**Go for slice 1, with three changes to the spec and one decision for Jared.**

The exit test in §20 is met: `listing`, `post_with_comments` and `more_comments` all have a
working same-origin method. But it is not the method the spec expected:

- **Same-origin `.json` is blocked.** The first `.json` call returned Reddit's
  "You've been blocked by network security" page. Every operation uses Reddit's own front-end
  partials and GraphQL calls instead (strategy 2), so bodies are HTML.
- **Writes that create content need a reCAPTCHA token.** Vote, subscribe and delete work.
  Comment needs a token from Google's invisible reCAPTCHA, which Agora must not produce
  (§5). Submit and send-message probably need one too.
- **Private messages have moved to Reddit Chat.** There is no inbox page left to read.

## 2. What was run

A Python + PyGObject prototype of exactly the SPEC §4.2 design: one persistent
`WebKit.NetworkSession`, a `WebView` that is created but never shown, a bridge script in the
isolated world `agora` (`UserScript.new_for_world`), a message handler registered for that world,
and every call made with `call_async_javascript_function(..., world="agora")` with typed
arguments. Large bodies come back in 60 KiB chunks through the `agora` message handler (a
1.7 MB comment tree arrived intact). Python was used only to move faster in a throwaway; the
C API is the same one the `webkit6` crate wraps.

Safeguards, all observed: one request at a time with at least 1.0 s + 0–500 ms jitter; a
latch that refused all further traffic after any 403 block page, 429 or redirect to login;
Jared signed in himself on Reddit's own page; cookie values were never read or printed; the
spike never clicked page controls. The write requests were learned by recording what the page
sent while **Jared clicked by hand**.

**Traffic from the spike:** 27 requests to reddit.com between 18:05 and 19:44 UTC (9 page loads,
15 partial or page reads, 3 writes) plus 5 media CDN fetches. Two were blocked (§4). Jared's
own clicks in the visible window are not counted.

## 3. Measurements asked for in §20

| Question | Answer |
|---|---|
| Is the hidden view throttled? | **Timers yes, fetch no.** 20 × `setTimeout(50 ms)` took 10.0 s hidden and 1.0 s mapped. `fetch()` from the bridge ran at full speed (100–1300 ms, network-bound). The bridge does not depend on timers, so no off-screen mapping is needed. Reddit's own page scripts barely render while hidden (the `/message/inbox/` app never drew), which is fine because Agora does not read rendered DOM |
| Where does the CSRF token live? | The `csrf_token` cookie, readable with `document.cookie` from the `agora` world. Partials send it as a form field, GraphQL as a JSON field. `reddit_session` is HttpOnly and never visible to the bridge |
| Do media CDNs accept an honest User-Agent? | **Yes.** `i.redd.it`, `preview.redd.it` (signed URL as served) and `styles.redditmedia.com` returned 200 to libsoup with `Agora/… (GNOME; +https://github.com/ClickCalickClick/agora)` and no cookies. `v.redd.it` not tried |
| Which page works as the lightweight home page? | **Not `/robots.txt`.** Both blocks happened while the view sat there; nothing sent from a real page was blocked. Use a real page (§6, change 2) |
| Isolated world holds? | Yes. From the page world, `typeof window.agora` and `window.webkit.messageHandlers.agora` are both `undefined` |

## 4. The two blocks

| Time (UTC) | Request | View was on | Result |
|---|---|---|---|
| 18:05 | `GET /api/me.json` (first request after sign-in) | `/robots.txt` | 403 "blocked by network security" |
| 19:37 | `POST /svc/shreddit/graphql` (clear a vote) | `/robots.txt` | 403 "blocked by network security" |

Each time all traffic stopped, the latch held, and nothing was retried until Jared agreed and
at least 5 minutes had passed. After the second block the same GraphQL call was sent from a
real post page and succeeded. The likely trigger is the page context (Referer and fetch
metadata of a `robots.txt` document). The first block may also mean `.json` is blocked
outright; the spike did not retry `.json` from a real page, because that would be probing a
block. Either way, `.json` is treated as gone. The account was not restricted: everything
after the blocks worked.

Separately, while the visible view sat on a post page for about 50 minutes, **Reddit's own
chat client** in that page polled `matrix.redditspace.com/_matrix/client/v3/sync` and got
HTTP 429. That traffic came from Reddit's own scripts, not Agora. It shows why the hidden
view must not keep a full Reddit front end running (§6, change 2).

## 5. Decision for Jared: posting and replying

`comment` sends a `recaptchaToken`. The page first gets it from Google's invisible
reCAPTCHA (the `CreateCaptchaToken` GraphQL call), and `create-comment` carries it. SPEC §0.3
and §4.6 say Agora never gets around a captcha. Calling the page's reCAPTCHA from the bridge
to mint tokens for automated posts would be exactly that. Options:

| Option | What the user sees | Cost | Fits the rules? |
|---|---|---|---|
| **A. Hand off to Reddit's own form (recommended)** | Agora's composer writes the text. **Post** opens Reddit's real reply or submit page in a visible `AdwDialog` WebView, with the text on the clipboard; the user pastes and presses Reddit's own button | Small. Less native for writing | Yes. A person does the captcha'd action in Reddit's own UI |
| B. Read-and-vote client | No commenting, posting or messaging in Agora; "Open in browser" for those | Smallest | Yes |
| C. Generate tokens through the page's reCAPTCHA | Fully native composer | Medium, fragile | **No**, it automates a captcha |

The spike recommends **A** for comment, submit and message, keeping the native composer for
drafting and preview. Edit was not tested and may not need a token.

## 6. Spec changes this calls for

1. **§4.5 strategy order:** drop same-origin `.json` as the preferred method; strategy 2
   (front-end partials and GraphQL) is the only method. `agora-markdown` gets the
   `html_to_ast` path in slice 1, not "if needed". Parsers must read inside `<template>`.
2. **§4.2 hidden view home page:** the view sits on a real reddit.com page, loaded once
   (for example `/r/popular/`). Reddit's own scripts there must not run background traffic:
   evaluate in slice 2 a `WebKit.UserContentFilter` that blocks the chat client
   (`matrix.redditspace.com`), ads and trackers (Google ad scripts loaded on every page), or
   reloading the home page periodically. Neither changes what Reddit's servers see from
   Agora's own calls.
3. **§4.5 operation table:** `inbox` becomes "notifications" (`/notifications`); private
   messages and `send_message` move to "Later" (Reddit Chat is a Matrix service and out of
   scope for 1.0). `comment`, `submit` and `send_message` follow the decision in §5.
4. **§4.5 writes:** add the GraphQL request shape (`operation`, `variables`, `csrf_token`) and
   the partial `Accept` header as shared helpers in the bridge. Learn each remaining write
   (save, hide, report, mark read, edit) by recording a hand click, the way this spike did.

## 7. Not covered (carry into slice 2 and 4)

- Home, Popular, user-posts and saved feeds (expected: the `*-more-posts` partial each page
  names in `shreddit-feed[reload-url]`).
- A non-empty subscriptions list (the method returned the expected empty JSON for a new
  account).
- Structured profile fields (profile parsing used page text).
- `v.redd.it` video through libsoup.
- save, hide, report, mark read, edit, submit, send message.

## 8. Fixtures

Committed to `crates/agora-reddit-web/tests/fixtures/`, scrubbed by
`spike/slice0/fixture.py`: scripts, styles and SVG removed; at most 3 posts and 3 top-level
comment threads kept; every author replaced with `user<N>` and the test account with
`agora_tester`; user ids, loid, correlation ids, partial signatures, CSRF values, country and
avatars redacted. The script asserts the account name is absent from every output. Raw bodies
and the WebKit profile stay outside the repository in `~/.local/share/agora-spike/`.
