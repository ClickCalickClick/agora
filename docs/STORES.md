# Stores and actions

The contract between the UI and the rest of Agora (SPEC §3.4). Frozen at the end
of slice 1 together with the `Backend` trait (`crates/agora-core/src/backend.rs`),
the operation table (SPEC §4.5) and the gschema keys (SPEC §13). After that,
changes need Jared's sign-off and an entry in `docs/CONTRACTS-CHANGELOG.md`
(SPEC §20.1 rule 3).

Rules that apply to every store:

- Stores are GObjects in `crates/agora-app/src/stores/`. Views bind to their
  properties and list models and call their methods; views never call the
  backend.
- Every store holds an `Rc<Context>`: the `Rc<dyn Backend>`, the
  `gio::Settings`, the `SessionStore`, a `mock` flag, and the toast sink.
- Every async reply is checked against a generation counter, so a reply for a
  feed or post the user has left is dropped.
- Errors become a `LoadState::Error` plus a translated `error` message for
  page-level loads, or a toast with Retry (when retryable) for row-level
  actions. `Cancelled` is silent; `SessionExpired` and `Blocked` are shown by
  the window banner through `SessionStore`.

## Item types (list model items)

| Type | Wraps | Properties |
|---|---|---|
| `PostObject` | `agora_core::Post` (`post()`, `with(f)`, `update(post)`) | `score: i64`, `vote: i32` (1, 0, -1), `saved: bool`, `read: bool`, `comment-count: u64` |
| `CommentObject` | one row of a flattened tree: a `Comment`, a "more" token, or a "continue" parent | `kind: CommentRowKind` (`Comment`, `More`, `Continue`), `depth: u32` (capped at 8), `collapsed: bool`, `count: u32` (hidden replies when collapsed; comments behind a More row), `score: i64`, `score-hidden: bool`, `vote: i32`, `saved: bool`, `highlight-line: i32` (-1 or the depth whose thread line is lit), `loading: bool` |
| `SubredditObject` | `agora_core::Subreddit` (`subreddit()`) | `subscribed: bool` |
| `FooterObject` | — | `state: LoadState` |

`LoadState`: `Idle`, `Loading`, `Error`, `Done`.

## SessionStore

| | |
|---|---|
| Properties | `username: String` (empty when signed out), `state: SessionState` (`Unknown`, `SignedIn`, `SignedOut`, `Expired`, `Blocked`) |
| Methods | `check()` runs `whoami`; `can_request() -> bool` |
| Listens to | `Backend::events()`: `SignedIn`, `SignedOut`, `SessionExpired`, `Blocked` |

## FeedStore

| | |
|---|---|
| Model | `posts() -> gio::ListStore<PostObject>` |
| Properties | `title`, `state: LoadState`, `error`, `subreddit: SubredditObject?` (filled by `subreddit_info` for subreddit feeds), `sort-key`, `top-range` |
| Methods | `set_feed(Feed)`, `set_sort(Sort)`, `refresh()`, `load_more()` (no-op while loading or when done), `feed()`, `sort()` |
| Pacing | Page 1 loads on `set_feed`/`set_sort`/`refresh`. Later pages load only when the user scrolls within about 10 rows of the end (SPEC §4.6); nothing is prefetched |

## ThreadStore

| | |
|---|---|
| Model | `rows() -> gio::ListStore<CommentObject>`: the visible rows of the flattened tree |
| Properties | `post: PostObject?`, `state: LoadState`, `error`, `focused: bool` (showing one branch), `sort-key` |
| Methods | `open(&PostObject)`, `open_id(PostId)`, `close()`, `reload()`, `set_sort(CommentSort)`, `focus_comment(CommentId)` / `unfocus()` ("Continue this thread"), `toggle_collapsed(&row)`, `highlight_branch(&row, on)`, `load_more(&row)` |
| Flattening | Rows are indented relative to the shallowest comment received (`Comment::depth` stays absolute in the backend), so a focused branch starts at level 0. Comments deeper than 8 levels become one "Continue this thread" row per parent. Collapsing moves the branch's rows into the comment's stash and back, so nested collapse state survives |

## SubscriptionsStore

| | |
|---|---|
| Model | `list() -> gio::ListStore<SubredditObject>`, favorites (`favorite-subreddits`) first, then by name |
| Properties | `state: LoadState` |
| Methods | `refresh()` |

## Writes

Not a GObject: a cloneable helper holding the `Context`. Each method changes
the object at once and reverts it, with an error toast, if Reddit's response is
an error (SPEC §4.5). One write per item and kind (vote, save) is in flight at
a time; a second press while it is pending is ignored, so a revert never undoes
a later change.

`vote_post(&PostObject, VoteState)`, `vote_comment(&CommentObject, VoteState)`
(voting the current vote again clears it), `toggle_save_post`,
`toggle_save_comment`. Subscribing goes through `win.toggle-subscribe`.

## Stores planned for later slices

Their shape is fixed now so later slices do not reshape the window:

| Store | Slice | Model / properties |
|---|---|---|
| `NotificationsStore` | 5 | `ListStore<NotificationObject>`, `state`, `unread-count`; `refresh()`, `load_more()`, `mark_all_read()` |
| `SearchStore` | 5 | `ListStore` of `PostObject` / `SubredditObject` / `UserObject` by kind; `query`, `kind`, `scope`, `state`; `search()`, `load_more()` |
| `ProfileStore` | 4 | `user`, `ListStore<PostObject>`; `open(Username)` |
| `DraftsStore` | 4 | `ListStore<DraftObject>` backed by `agora.db`; `save(context, text)`, `delete(context)` |
| `FiltersStore` | 5 | `muted-subreddits`, `muted-keywords` from GSettings; `filter(&[Post]) -> (kept, hidden_count)` |

## Actions

| Action | Where | Accelerator | Does |
|---|---|---|---|
| `app.quit` | application | Ctrl+Q | Quit |
| `app.about` | application | | About dialog with the disclaimer |
| `app.shortcuts` | application | Ctrl+? | Keyboard shortcuts dialog |
| `window.close` | GTK | Ctrl+W | Close the window |
| `win.toggle-sidebar` | window | F9 | Show or hide the sidebar (property action on `show-sidebar`) |
| `win.refresh` | window | F5, Ctrl+R | Reload the post if the post pane has focus, else the feed |
| `win.back` | window | | Back: leave a focused branch, then the post pane |
| `win.zoom-in`, `win.zoom-out`, `win.zoom-reset` | window | Ctrl++, Ctrl+-, Ctrl+0 | Content zoom 80–200% (`content-zoom`) |
| `win.toggle-subscribe(s)` | window | | Join or leave the subreddit shown in the list pane |
| `feed.refresh` | feed page (widget action) | | Reload the feed |
| `feed.sort(s)` | feed page (property action on `sort-target`) | | `best`, `hot`, `new`, `rising`, `top:<range>` |
| `post.sort(s)` | post page (property action on `sort-target`) | | Comment sort key |
| `post.copy-link`, `post.open-in-browser` | post page (widget actions) | | Share menu (SPEC §12.3) |

Single-letter keys (SPEC §11) are handled by the window in the capture phase
and do nothing while a text field has focus: `j`/`k` move the selection in the
pane that has focus, `o` opens the selected post, `a`/`z`/`s` vote and save the
selected post or comment, Space collapses the selected comment, `h` goes back.
