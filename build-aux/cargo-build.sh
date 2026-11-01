#!/bin/sh
# Called by the `cargo-build` target in meson.build.
# Usage: cargo-build.sh CARGO MANIFEST TARGET_DIR PROFILE OUTPUT
#
# Meson passes AGORA_APP_ID, AGORA_PROFILE, AGORA_LOCALEDIR, AGORA_PKGDATADIR
# and CARGO_TARGET_DIR in the environment; crates/agora-app/build.rs reads
# them. Flatpak builds set CARGO_NET_OFFLINE=true and CARGO_HOME to the
# vendored sources (build-aux/README.md).
set -eu

cargo=$1
manifest=$2
target_dir=$3
profile=$4
output=$5

if [ "$profile" = "development" ]; then
    "$cargo" build --manifest-path "$manifest" --target-dir "$target_dir" -p agora-app --bin agora
    built="$target_dir/debug/agora"
else
    "$cargo" build --manifest-path "$manifest" --target-dir "$target_dir" -p agora-app --bin agora --release
    built="$target_dir/release/agora"
fi

cp "$built" "$output"
