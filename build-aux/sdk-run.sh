#!/bin/sh
# Run a command inside the GNOME SDK (target runtime, SPEC §3.1) with the
# Rust SDK extension, from the repository root. CI uses this for fmt, clippy,
# the Meson build and the tests; it also works locally once the SDK and
# org.freedesktop.Sdk.Extension.rust-stable are installed.
#
#   build-aux/sdk-run.sh cargo fmt --all --check
#
# Network is shared so Cargo can fetch crates. GTK tests need a display:
# run this under xvfb-run (X11 is passed through).
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
branch=${AGORA_SDK_BRANCH:-51}

exec flatpak run \
    --share=network --share=ipc \
    --socket=x11 --socket=wayland --device=dri \
    --filesystem="$root" \
    --cwd="$root" \
    --env=PATH=/usr/lib/sdk/rust-stable/bin:/app/bin:/usr/bin:/bin \
    --env=CARGO_HOME="$root/.cargo-home" \
    --command=sh \
    "org.gnome.Sdk//$branch" \
    -c 'exec "$@"' sh "$@"
