#!/usr/bin/env bash
# Builds the Rust engine as a static library for the Xcode app.
#
#   ios/build-rust.sh [--profile fast] [--features ...]
#
# Extra arguments go to `cargo build` (after `--profile release --target
# aarch64-apple-ios`). The Xcode project (ios/project.yml) runs this as a
# pre-build phase, so the .a exists before the linker runs; CI runs the same
# script. `cargo build` itself is a no-op when nothing changed, so running it
# on every build is cheap.
#
# Needs: a Rust toolchain with the aarch64-apple-ios target (installed here when
# missing) and the iOS SDK, so this only runs on macOS.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"

profile=release
cargo_args=()
while [ $# -gt 0 ]; do
  case "$1" in
    --profile) profile="$2"; shift 2 ;;
    *) cargo_args+=("$1"); shift ;;
  esac
done

# A lock file of its own, seeded from the desktop one so every dependency keeps
# the version the desktop client is tested with (android/build-apk.sh does this).
if [ ! -f "$here/Cargo.lock" ]; then
  cp "$root/Cargo.lock" "$here/Cargo.lock"
fi

rustup target list --installed 2>/dev/null | grep -q '^aarch64-apple-ios$' \
  || rustup target add aarch64-apple-ios

cargo build --profile "$profile" --target aarch64-apple-ios --manifest-path "$here/Cargo.toml" "${cargo_args[@]}"

lib="$here/target/aarch64-apple-ios/$profile/libhorizon_ios.a"
[ -f "$lib" ] || { echo "error: $lib was not built" >&2; exit 1; }
echo "Built $lib ($(du -h "$lib" | cut -f1))"
