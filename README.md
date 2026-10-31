# Agora

**A Reddit client for GNOME.**

Agora is a native GNOME app for browsing and discussing Reddit, written in Rust with GTK 4 and libadwaita. It adapts from desktop to phone, works with keyboard, touch, and screen readers, and follows your system style and accent color.

> Agora is an unofficial client and is not affiliated with or endorsed by Reddit, Inc.

## Status

Planning. The full design lives in [docs/SPEC.md](docs/SPEC.md) and is under review before development starts. No app code yet.

## Planned features

- Three-pane adaptive layout (subreddits, posts, post view) that collapses on narrow screens
- Reading, voting, posting, replying, and inbox
- Markdown composer with formatting toolbar, live preview, and auto-saved drafts
- Image viewer, galleries, and inline video
- Desktop notifications for replies and mentions, with optional background checking
- GNOME Shell search provider for your subreddits
- Full keyboard shortcuts and screen reader support
- Offline cache for recent and saved posts
- NSFW blurring and muted subreddits/keywords

## Bring your own API key

Agora does not ship with Reddit API credentials. On first launch, a short guided setup walks you through registering your own free "installed app" client ID on Reddit.

## Credits

Agora is inspired by [Giara](https://gitlab.gnome.org/World/giara) by Gabriele Musco and contributors, which showed what a Reddit client that belongs on GNOME could look like. Agora is a fresh rewrite and reuses no Giara code.

## Code of Conduct

This project follows the [GNOME Code of Conduct](https://conduct.gnome.org/).

## License

GPL-3.0-or-later. See `LICENSE` (added with the first code commit).
