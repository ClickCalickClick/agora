# Extraction strategies

One method per operation (SPEC §4.5). Checked live in the slice 0 spike on
2026-10-31 against a throwaway account, WebKitGTK 2.54.2. Full write-up:
[`docs/SPIKE-0.md`](../../docs/SPIKE-0.md). Fixtures are in `tests/fixtures/`.

**Strategy 1 (same-origin `.json`) is blocked.** `/api/me.json` returned HTTP 403
with Reddit's "You've been blocked by network security" page. Every operation
below therefore uses **strategy 2**: the HTML partials and GraphQL calls
reddit.com's own front end makes, sent from the hidden view while it sits on a
real reddit.com page. Bodies are HTML, so `agora-markdown` needs `html_to_ast`.

## Rules every operation follows

- **Home page:** the hidden view must sit on a real reddit.com page (for
  example `https://www.reddit.com/r/popular/` or the subreddit being read).
  Both blocks in the spike happened while it sat on `/robots.txt`; nothing sent
  from a real page was blocked.
- **CSRF:** the `csrf_token` cookie is readable through `document.cookie` from
  the `agora` world. POSTs send it as a `csrf_token` form field (partials) or
  JSON field (GraphQL).
- **Partial requests** send `Accept: text/vnd.reddit.partial+html, text/html;q=0.9`
  (what `faceplate-partial` sends) or use a URL that already carries
  `render-mode=partial`. Without it some routes return a full page shell (200)
  or HTTP 406.
- **Block detection:** HTTP 403 whose body contains
  `blocked by network security` is `Blocked` (fixture
  `blocked_network_security.html`), as are 429 and redirects to `/login`.
- **Templates:** comment trees put most comments inside `<template>` elements;
  parsers must descend into template content.

## Operations

| Operation | Status | Method (strategy 2) | Fixture |
|---|---|---|---|
| `whoami` | works | `GET /user/me/` follows a redirect to `/user/{name}/`; name is the last path segment of `response.url`. `shreddit-app[user-logged-in]` gives a cheap logged-in check on any page | — |
| `listing` (subreddit) | works | `GET /svc/shreddit/community-more-posts/{sort}/?name={sub}[&t={RANGE}][&after={cursor}]`. Posts are `<shreddit-post>` with attributes (`id`, `post-title`, `score`, `comment-count`, `post-type`, `content-href`, `permalink`, `created-timestamp`, `subreddit-name`, `author`…). Next cursor: the `after=` in the trailing `community-more-posts` URL. Skip `<shreddit-ad-post>` | `listing_subreddit.html` |
| `listing` (Home, Popular, user posts, saved) | not tried | Expected: the matching `*-more-posts` partials named in each page's `shreddit-feed[reload-url]` | — |
| `post_with_comments` | works | `GET /svc/shreddit/comments/r/{sub}/{post_id_without_t3}?render-mode=partial` for the comment tree (`<shreddit-comment>`: `thingid`, `depth`, `score`, `created`, `permalink`, body in `[slot=comment]`). Post body: from the listing row, or the post page's `<shreddit-post>` with `[slot=text-body]` | `post_comments.html` |
| `more_comments` | works | `POST` to the `src` of the `faceplate-partial.more-comments-partial` (`/svc/shreddit/more-comments/r/{sub}/{post_id}?sort=…&startingDepth=…&commentParentPositions=…`) with form `cursor` (from the partial's hidden input) and `csrf_token` | `more_comments.html` |
| `subreddit_info` | works | `GET /svc/shreddit/feeds/subreddit-right-rail?name={sub}` with the partial `Accept` header: rules, created date, members. `shreddit-subreddit-header` attributes on subreddit and post pages give name, title, description, weekly users. Posting requirements: `/svc/shreddit/subreddit-posting-eligibility-modal/{sub}/` (not tried) | `subreddit_info_right_rail.html` |
| `subscriptions` | works (empty list only) | Left-nav partial (`common-left-nav`, signed URL taken from the current page) with the partial `Accept` header; `left-nav-communities-controller[initialStateJSON]` holds JSON. Only seen empty (new account); confirm non-empty in slice 2 | `subscriptions_left_nav_empty.html` |
| `user_profile` | works (text) | `GET /user/{name}/`; karma, cake day and contributions parsed from page text. Look for structured attributes in slice 2 | `user_profile_own.html` |
| `inbox` | missing | `/message/inbox/` renders no messages; private messages now live in Reddit Chat (Matrix, `matrix.redditspace.com`). `/notifications` renders notifications (empty on the test account) | — |
| `search` | works | `GET /search/?q={q}&type=posts` page; next page via `/svc/shreddit/search/?q=…&type=posts&cursor=…` found in the page | `search_posts.html` |
| `vote` | works | `POST /svc/shreddit/graphql` JSON `{operation:"UpdatePostVoteState", variables:{input:{postId, voteState:"UP"\|"DOWN"\|"NONE"}}, csrf_token}`; response `data.updatePostVoteState.ok` | `write_requests.json`, `write_responses.json` |
| `subscribe` | works | GraphQL `UpdateSubredditSubscriptions` `{input:{inputs:[{subredditId, subscribeState:"SUBSCRIBED"\|"NONE"}]}}` | same |
| `delete` (comment) | recorded, not replayed | GraphQL `DeleteComment` `{input:{commentId}}` | `write_requests.json` |
| `comment` | **blocked by design** | `POST /svc/shreddit/{post_id}/create-comment` form (`mode=richText`, `content`=rich-text JSON, `richTextMedia`, `csrf_token`) **plus `recaptchaToken`** from Google's invisible reCAPTCHA. Agora must not generate or bypass it; see `docs/SPIKE-0.md` §5 | `write_requests.json` |
| `submit`, `send_message` | not tried | Expected to need reCAPTCHA like `comment` | — |
| `save`, `hide`, `report`, `mark_read`, `edit` | not tried | Expected: GraphQL operations; record by hand-click in slice 4 | — |

## Media

`i.redd.it`, `preview.redd.it` (signed URLs as served) and
`styles.redditmedia.com` all returned 200 to libsoup with Agora's own
User-Agent and no cookies. `v.redd.it` not tried.
