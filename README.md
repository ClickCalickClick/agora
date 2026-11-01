# Agora

**An unofficial Reddit client for GNOME. Public source, build it yourself.**

Agora is a native GNOME app for reading and discussing Reddit, written in Rust with GTK 4 and libadwaita. It adapts from desktop to phone, works with keyboard, touch, and screen readers, and follows your system style and accent color.

## Disclaimer

> **Agora is an unofficial, independent project. It is not affiliated with, endorsed by, or sponsored by Reddit, Inc.** "Reddit" is a trademark of Reddit, Inc. and is used only to describe what this app works with.
>
> **Personal use only.** Agora shows Reddit through your own reddit.com login, inside an embedded web browser. Automated use of reddit.com may violate Reddit's User Agreement; you are responsible for how you use it and for your account.
>
> **It may break.** Agora depends on reddit.com's website and may stop working at any time.
>
> **No warranty.** The software is provided "as is", without warranty of any kind, as stated in the GPL-3.0 license. Use it at your own risk.

Agora is not published on Flathub, in any app store, or as prebuilt binaries. If you use it, you build it yourself from this source.

## How it talks to Reddit

Reddit is closing its public API, so Agora does not use it, and this repository contains no Reddit API keys or app credentials. Instead you sign in to reddit.com once, inside an embedded web view. Agora keeps that browser session in a hidden WebKitGTK view, asks it for the pages and data you open, and shows the results in a native GNOME interface. Traffic is kept to the pace of a person using a browser: no prefetching, no bulk syncing, and no checking for messages after you close the window.

## Status

Slice 1: the app runs on built-in sample data (`AGORA_MOCK=1`); reading real Reddit arrives in slice 2. The design lives in [docs/SPEC.md](docs/SPEC.md), with the reviews that shaped it in [docs/reviews/](docs/reviews/).

## Planned features

- Three-pane adaptive layout (subreddits, posts, post view) that collapses on narrow screens
- Reading, voting, posting, replying, and inbox
- Markdown composer with formatting toolbar, live preview, and auto-saved drafts
- Image viewer, galleries, and inline video
- Desktop notifications for replies and mentions while Agora is open
- GNOME Shell search provider for your subreddits
- Full keyboard shortcuts and screen reader support
- NSFW blurring and muted subreddits/keywords

## Building

Agora is built against the current GNOME platform. You build it yourself; there are no prebuilt packages.

### Flatpak (recommended)

Needs `flatpak-builder`, the GNOME 51 SDK and `org.freedesktop.Sdk.Extension.rust-stable`. First generate the vendored crate list (see [build-aux/README.md](build-aux/README.md); repeat after every `Cargo.lock` change), then:

```sh
flatpak-builder --user --install --force-clean _flatpak build-aux/io.github.ClickCalickClick.Agora.Devel.json
flatpak run io.github.ClickCalickClick.Agora.Devel
```

### On the host

Host dependencies:

- GTK 4.24 and libadwaita 1.10
- WebKitGTK 6.0 (needed from slice 2)
- blueprint-compiler
- Rust 1.92 or newer, with Cargo
- Meson and gettext

Quick run from the source tree, on sample data:

```sh
AGORA_MOCK=1 cargo run -p agora-app
```

Full build with Meson (installs into `_build/install`):

```sh
meson setup _build -Dprofile=development --prefix="$PWD/_build/install"
ninja -C _build install
AGORA_MOCK=1 XDG_DATA_DIRS="$PWD/_build/install/share:$XDG_DATA_DIRS" ./_build/install/bin/agora
```

`meson test -C _build` runs the Rust tests and validates the desktop file, AppStream metadata and GSettings schema.

## Credits

Agora is inspired by [Giara](https://gitlab.gnome.org/World/giara) by Gabriele Musco and contributors, which showed what a Reddit client that belongs on GNOME could look like. Agora is a fresh rewrite and reuses no Giara code or assets.

## Code of Conduct

This project follows the [GNOME Code of Conduct](https://conduct.gnome.org/).

## License

GPL-3.0-or-later. See `LICENSE`.
