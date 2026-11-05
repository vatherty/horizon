#!/usr/bin/env bash
# Builds Horizon's iOS app: dist/horizon-ios-unsigned.ipa, unsigned.
#
#   ios/build-ipa.sh
#
# Needs (ios/README.md, "Build"): a Mac with Xcode, XcodeGen (`brew install
# xcodegen`) and a Rust toolchain with the aarch64-apple-ios target. It uses no
# signing identity anywhere: the .ipa is unsigned, and SideStore/AltStore signs
# it on install with a free Apple ID. The .ipa holds only the engine; the job
# fails if it holds anything of Minecraft's (the workflow checks that).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
dist="$root/dist"

for arg in "$@"; do
  case "$arg" in
    -h|--help) awk 'NR>1 && /^#/ {sub(/^# ?/, ""); print; next} NR>1 {exit}' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) echo "unknown option $arg (--help lists it)" >&2; exit 2 ;;
  esac
done

die() { echo "error: $*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "$1 not found. $2"; }

need xcodegen "Install XcodeGen (brew install xcodegen)."
need xcodebuild "Install Xcode."
need cargo "Install Rust (rustup)."

# ---- the engine: the Rust static library -------------------------------------

"$here/build-rust.sh"

# ---- the app: Xcode project, build, package ----------------------------------

(cd "$here" && xcodegen generate)

xcodebuild -project "$here/Horizon.xcodeproj" -scheme Horizon \
  -sdk iphoneos -configuration Release \
  -derivedDataPath "$here/build/derived" \
  CODE_SIGNING_ALLOWED=NO CODE_SIGN_IDENTITY="" build

app="$here/build/derived/Build/Products/Release-iphoneos/Horizon.app"
[ -d "$app" ] || die "$app was not built"

rm -rf "$dist"
mkdir -p "$dist/Payload"
cp -r "$app" "$dist/Payload/"
ditto -c -k --sequesterRsrc --keepParent "$dist/Payload" "$dist/horizon-ios-unsigned.ipa"
rm -rf "$dist/Payload"

echo
echo "Built $dist/horizon-ios-unsigned.ipa ($(du -h "$dist/horizon-ios-unsigned.ipa" | cut -f1))"
echo "Install it with SideStore/AltStore, which signs it with a free Apple ID."
