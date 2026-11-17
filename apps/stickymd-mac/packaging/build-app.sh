#!/bin/sh
# Builds an ad-hoc signed StickyMD.app for this Mac (arm64 only).
#
#   apps/stickymd-mac/packaging/build-app.sh [output-directory]
#
# The output directory is the note's identity: its note/ subdirectory is never touched here.
# Only the bundle itself is replaced.
set -eu

packaging_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$packaging_dir/../../.." && pwd)
output=${1:-"$repo/dist/macos"}
target=aarch64-apple-darwin

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$repo/Cargo.toml" | head -n 1)
if [ -z "$version" ]; then
    echo "could not read the workspace version from Cargo.toml" >&2
    exit 1
fi

# The product input fingerprint binds acceptance evidence to what decides the app's behaviour,
# not to its bytes (plan 11 #phase-16-macos-host). Taken before and after the build, so a change
# to the checkout during the build is refused rather than recorded.
inputs() {
    cargo run --quiet --manifest-path "$repo/Cargo.toml" -p stickymd-smoke --locked -- mac-ledger inputs
}
inputs_before=$(inputs)
case "$inputs_before" in
    *[!0-9a-f]* | "")
        echo "could not compute the product input fingerprint" >&2
        exit 1
        ;;
esac
if [ "${#inputs_before}" -ne 64 ]; then
    echo "the product input fingerprint is not a SHA-256: $inputs_before" >&2
    exit 1
fi

cargo build --manifest-path "$repo/Cargo.toml" -p stickymd-mac --release --locked --target "$target"

if [ "$(inputs)" != "$inputs_before" ]; then
    echo "the checkout changed during the build; build again" >&2
    exit 1
fi

mkdir -p "$output"
bundle="$output/StickyMD.app"
staging="$output/.StickyMD.app.staging"
rm -rf "$staging"
mkdir -p "$staging/Contents/MacOS"
cp "$repo/target/$target/release/stickymd-mac" "$staging/Contents/MacOS/StickyMD"
sed -e "s/@VERSION@/$version/g" -e "s/@INPUTS@/$inputs_before/g" "$packaging_dir/Info.plist" >"$staging/Contents/Info.plist"
plutil -lint "$staging/Contents/Info.plist" >/dev/null

codesign --force --sign - --timestamp=none "$staging"
codesign --verify --strict "$staging"

architectures=$(lipo -archs "$staging/Contents/MacOS/StickyMD")
if [ "$architectures" != "arm64" ]; then
    echo "expected an arm64-only executable, found: $architectures" >&2
    exit 1
fi

rm -rf "$bundle"
mv "$staging" "$bundle"
echo "$bundle"
