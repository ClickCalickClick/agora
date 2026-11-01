# Slice 1: reading on sample data

**Goal:** a working Agora that reads posts and comment threads end to end on
`MockBackend`, with every contract the later slices build on frozen (SPEC §20).
No traffic to reddit.com.

## Delivered

- Cargo workspace: `agora-core` (models, `Backend`, errors, `MockBackend`),
  `agora-markdown` (`html_to_ast`, `markdown_to_ast`), `agora-app`.
- Scaffold: Meson, Flatpak manifest, CI workflow, gschema (all §13 keys),
  desktop file, metainfo, placeholder icon, `LICENSE`, `CODE_OF_CONDUCT.md`,
  gettext plumbing, About with the disclaimer.
- Window with the three-pane adaptive layout (wide, medium, narrow breakpoints),
  sidebar (Home, Popular, Saved, subscriptions), post list with quiet rows,
  community header with Join, infinite scroll, sort menu, post view with text
  and link posts, comment tree with conversation lines, collapse, "more
  replies", "continue this thread", Markdown/HTML renderer with spoilers,
  empty/loading/error states, content zoom, keyboard shortcuts dialog, `j`/`k`.
- Contracts: `docs/STORES.md`, `docs/CONTRACTS-CHANGELOG.md`.

## Running it

```bash
AGORA_MOCK=1 cargo run -p agora-app
```

`AGORA_MOCK_FAIL=listing=network,post_with_comments=notfound` scripts
failures; `AGORA_MOCK_LATENCY_MS=1500` slows every operation so loading states
are visible. Slice 1 always uses sample data and says so in a banner.

## Exit checklist

- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test --workspace`, `meson test` (desktop, schema, metainfo,
      POTFILES) pass locally
- [x] Reading works at 1280 px and 360 px (headless screenshots)
- [ ] Keyboard-only walkthrough by hand (below)
- [ ] Orca walkthrough by hand (below)
- [ ] CI green on GitHub (Flatpak, cargo-deny and gitleaks have only run in CI)
- [ ] Contracts frozen: Jared signs off in the PR

## Manual walkthrough (Jared)

Keyboard only, then again with Orca (Super+Alt+S):

1. Tab into the sidebar, arrow to r/gnome, Enter. The list shows the community
   header and posts; Orca reads each row as title, then subreddit, author, age,
   points, comments.
2. Press `j` three times, then `k`. The selection moves; Enter or `o` opens the
   post.
3. In the post, Tab to the comment list. `j`/`k` move between comments; Orca
   says "Level N reply by …". Space collapses and expands a comment.
4. On a "more replies" row press Enter: the replies load in place.
5. `a`, `z`, `s` vote and save the selected comment; the score changes.
6. Ctrl++ and Ctrl+- change the text size of the post body only.
7. Narrow the window to phone width: the sidebar becomes an overlay (F9 or
   the header button), and Back returns from a post to the list.
