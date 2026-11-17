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
# not to its bytes (plan 11 #phase-16-macos-host). The fingerprint and the build both run in this
# repository with a clean environment: only HOME and a fixed PATH (the directory of the caller's
# cargo, then the system directories), so no variable can change the build without changing
# the fingerprint. Taken again after the bundle is staged, so a change to the inputs between the
# two is refused; a change undone in between is not detected.
cargo_path=$(command -v cargo) || {
    echo "cargo is not on PATH" >&2
    exit 1
}
cargo_dir=$(CDPATH= cd -- "$(dirname -- "$cargo_path")" && pwd)
clean() {
    (cd "$repo" && env -i HOME="$HOME" PATH="$cargo_dir:/usr/bin:/bin:/usr/sbin:/sbin" "$@")
}
inputs() {
    clean cargo run --quiet --locked -p stickymd-smoke -- mac-ledger inputs
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

# The target directory is fixed, so the executable copied below is the one this build produced.
clean cargo build -p stickymd-mac --release --locked --target "$target" --target-dir "$repo/target"

mkdir -p "$output"
bundle="$output/StickyMD.app"
staging="$output/.StickyMD.app.staging"
rm -rf "$staging"
mkdir -p "$staging/Contents/MacOS"
cp "$repo/target/$target/release/stickymd-mac" "$staging/Contents/MacOS/StickyMD"
sed -e "s/@VERSION@/$version/g" -e "s/@INPUTS@/$inputs_before/g" "$packaging_dir/Info.plist" >"$staging/Contents/Info.plist"
plutil -lint "$staging/Contents/Info.plist" >/dev/null

# Every input has now been read: the sources by the build, the Info.plist template just above.
inputs_after=$(inputs)
if [ "$inputs_after" != "$inputs_before" ]; then
    echo "the product inputs changed during the build; build again" >&2
    rm -rf "$staging"
    exit 1
fi

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
