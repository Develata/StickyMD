#!/bin/sh
# Measures a built StickyMD.app in Source, Preview and Split: physical footprint after settling and
# idle CPU over a minute, caret blink included. Needs an unlocked screen with the display on.
#
#   apps/stickymd-mac/packaging/measure-resources.sh path/to/StickyMD.app
#
# Each mode runs in a throwaway program directory, so no real note is opened or changed.
set -eu

bundle=$1
idle_seconds=60
settle_seconds=5

cpu_seconds() {
    ps -o time= -p "$1" | awk -F'[:.]' '{ print $1 * 60 + $2 + $3 / 100 }'
}

for mode in source preview split; do
    directory=$(mktemp -d "${TMPDIR:-/tmp}/stickymd-measure.XXXXXX")
    cp -R "$bundle" "$directory/StickyMD.app"
    mkdir -p "$directory/note"
    printf 'version = 1\nview_mode = "%s"\n' "$mode" >"$directory/note/config.toml"
    {
        echo '# 资源基线'
        echo
        echo '行内公式 $e^{i\pi}+1=0$ 与一段中文。'
        echo
        echo '$$\sum_{k=1}^{n} k = \frac{n(n+1)}{2}$$'
        echo
        echo '- 列表项'
        echo '- `code`'
    } >"$directory/note/note.md"

    "$directory/StickyMD.app/Contents/MacOS/StickyMD" >/dev/null 2>&1 &
    pid=$!
    sleep "$settle_seconds"
    footprint=$(footprint -p "$pid" 2>/dev/null | awk '/phys_footprint:/ { print $2, $3; exit }')
    before=$(cpu_seconds "$pid")
    sleep "$idle_seconds"
    after=$(cpu_seconds "$pid")
    awk -v m="$mode" -v f="$footprint" -v b="$before" -v a="$after" -v s="$idle_seconds" \
        'BEGIN { printf "%-8s footprint=%s idle_cpu=%.2f%%\n", m, f, (a - b) * 100 / s }'
    kill -TERM "$pid"
    wait "$pid" 2>/dev/null || true
    rm -rf "$directory"
done
