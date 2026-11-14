#!/bin/zsh
# How LaunchServices treats one bundle opened twice, and copies of it at other paths.
# usage: run_bundle_matrix.sh <bundle_probe binary> <work dir>
set -u
BIN=$1; WORK=$2
rm -rf "$WORK"; mkdir -p "$WORK/A" "$WORK/B"

make_app() {
  local app=$1/Probe.app
  mkdir -p "$app/Contents/MacOS"
  cp "$BIN" "$app/Contents/MacOS/bundle_probe"
  cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>dev.stickymd.probe</string>
<key>CFBundleExecutable</key><string>bundle_probe</string>
<key>CFBundleName</key><string>Probe</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.0.1</string>
<key>LSUIElement</key><true/>
</dict></plist>
PLIST
  codesign --force --sign - "$app" 2>&1
}

count() { pgrep -f "Probe.app/Contents/MacOS/bundle_probe" | wc -l | tr -d ' '; }

make_app "$WORK/A"; make_app "$WORK/B"
codesign --verify --strict "$WORK/A/Probe.app" 2>&1; echo "verify fresh bundle: $?"
spctl -a -t exec -vv "$WORK/A/Probe.app" 2>&1 | head -3

export PROBE_SECONDS=12
open "$WORK/A/Probe.app"; sleep 2.5; echo "after open A: processes=$(count)"
open "$WORK/A/Probe.app"; sleep 1.5; echo "after open A again: processes=$(count)"
open "$WORK/B/Probe.app"; sleep 2.5; echo "after open B (same id, other path): processes=$(count)"
"$WORK/A/Probe.app/Contents/MacOS/bundle_probe" & sleep 2; echo "after direct exec of A: processes=$(count)"
open -n "$WORK/A/Probe.app"; sleep 2; echo "after open -n A: processes=$(count)"
sleep 12
pkill -f "Probe.app/Contents/MacOS/bundle_probe"
for f in "$WORK"/A/probe-*.log "$WORK"/B/probe-*.log; do echo "--- $f"; cat "$f"; done

# Does writing beside the executable inside the bundle break the seal?
mkdir -p "$WORK/A/Probe.app/Contents/MacOS/note"; echo x > "$WORK/A/Probe.app/Contents/MacOS/note/note.md"
codesign --verify --strict "$WORK/A/Probe.app" 2>&1 | head -3; echo "verify after writing inside the bundle: ${pipestatus[1]}"
