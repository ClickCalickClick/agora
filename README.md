# Agora

**A personal Reddit client for GNOME.**

Agora is a native GNOME app for reading and discussing Reddit, written in Rust with GTK 4 and libadwaita. It adapts from desktop to phone, works with keyboard, touch, and screen readers, and follows your system style and accent color.

> Agora is an unofficial, personal client. It is not affiliated with or endorsed by Reddit, Inc., and it is not published on Flathub or anywhere else.

## How it talks to Reddit

Reddit is closing its public API, so Agora does not use it and needs no API key. Instead you sign in to reddit.com once, inside an embedded web view. Agora keeps that browser session in a hidden WebKitGTK view, asks it for the pages and data you open, and shows the results in a native GNOME interface. Traffic is kept to the pace of a person using a browser: no prefetching, no bulk syncing, and background checks are off by default.

This depends on reddit.com's website, so expect occasional breakage when Reddit changes it. Automated use of the site is against Reddit's user agreement; use it on your own account at your own risk.

The data layer sits behind a pluggable backend, so a version for open networks like Lemmy or PieFed could be added later.

## Status

Planning. The design lives in [docs/SPEC.md](docs/SPEC.md), with the review that shaped it in [docs/SPEC-REVIEW.md](docs/SPEC-REVIEW.md). No app code yet.

## Planned features

- Three-pane adaptive layout (subreddits, posts, post view) that collapses on narrow screens
- Reading, voting, posting, replying, and inbox
- Markdown composer with formatting toolbar, live preview, and auto-saved drafts
- Image viewer, galleries, and inline video
- Desktop notifications for replies and mentions
- GNOME Shell search provider for your subreddits
- Full keyboard shortcuts and screen reader support
- Offline cache of what you've viewed
- NSFW blurring and muted subreddits/keywords

## Building (planned)

Built with Meson and Cargo against the current GNOME SDK, or locally with `flatpak-builder` using `build-aux/io.github.ClickCalickClick.Agora.Devel.json`. WebKitGTK 6.0 comes from the GNOME runtime. Non-Flatpak builds need WebKitGTK 6.0 and `glycin-loaders` installed on the host.

## Credits

Agora is inspired by [Giara](https://gitlab.gnome.org/World/giara) by Gabriele Musco and contributors, which showed what a Reddit client that belongs on GNOME could look like. Agora is a fresh rewrite and reuses no Giara code or assets.

## Code of Conduct

This project follows the [GNOME Code of Conduct](https://conduct.gnome.org/).

## License

GPL-3.0-or-later. See `LICENSE` (added with the first code commit).
