# build-aux

Build and packaging helpers. Nothing here is needed for a plain
`cargo run -p agora-app`.

| File | Purpose |
|---|---|
| `io.github.ClickCalickClick.Agora.Devel.json` | Flatpak manifest for the development build (SPEC §18.3). |
| `cargo-sources.json` | Vendored crate list for offline Flatpak builds. **Generated, not committed** (see below). |
| `cargo-build.sh` | Called by the `cargo-build` target in `meson.build`; runs Cargo with the Meson-provided environment and copies the binary. |
| `gen-potfiles.py` | Writes `po/POTFILES.in` (Meson has no glob). `--check` is run by `meson test` (`potfiles`). |
| `sdk-run.sh` | Runs a command inside the GNOME SDK with the Rust extension, for local checks against the target runtime. |

## How the pieces fit

`crates/agora-app/build.rs` compiles the Blueprint files, the GResource
bundle and a development copy of the GSettings schema. Meson does not
compile resources: it calls Cargo with `AGORA_APP_ID`, `AGORA_PROFILE`,
`AGORA_LOCALEDIR` and `AGORA_PKGDATADIR` set, installs the `agora` binary, and
installs the desktop file, metainfo, schema and icons from `data/`.
(SPEC §19 still describes a `gen-resources.py`; build.rs replaced it.)

## Flatpak build

Flatpak builds run without network access, so every crate in `Cargo.lock`
is downloaded by flatpak-builder from `cargo-sources.json` and Cargo runs with
`CARGO_NET_OFFLINE=true` and `CARGO_HOME=/run/build/agora/cargo`.

Generate `cargo-sources.json` with
[flatpak-cargo-generator](https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo)
before the first build and **again after every change to `Cargo.lock`**:

```sh
# needs: python3 with aiohttp and tomlkit
python3 flatpak-cargo-generator.py Cargo.lock -o build-aux/cargo-sources.json
flatpak-builder --user --install --force-clean _flatpak build-aux/io.github.ClickCalickClick.Agora.Devel.json
```

CI regenerates it from `Cargo.lock` on every run, so it cannot go stale there.
A stale local copy shows up as Cargo failing to find a crate offline.

`blueprint-compiler` is built as a module from its upstream `v0.22.2` tag so
the build does not depend on which version the GNOME SDK ships.
