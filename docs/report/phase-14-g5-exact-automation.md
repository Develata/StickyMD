# Phase 14 G5 Exact-Candidate Automation

## Result

G5 exact desktop harness implemented. Development diagnostic over the previous exact EXE completed
G5-01..04 in sequence. This dirty-harness run is implementation evidence only; it is not a clean
release receipt and becomes irrelevant after the next source freeze.

## Authority Boundary

Rust owns candidate identity, isolated copied-program directories, window/file assertions, artifact
hashing and receipt serialization. `windows-uia.ps1` only captures the visible StickyMD window to a
requested PNG path. It does not interpret pixels or decide PASS.

The exact receipt can fully replace P12-M03/M04 because their release fact is the actual HWND's
ToolWindow/AppWindow shell eligibility. The remaining G5-covered rows retain human authority where
they require real IME, physical mixed-DPI/System-theme transitions or first visual judgment. G5 only
removes repeated setup and supplies mechanically verified, candidate-bound companion screenshots.

## Cohesive Groups

| Group | Mechanical facts | Screenshot evidence |
| --- | --- | --- |
| G5-01 | `WS_EX_TOOLWINDOW=true`, `WS_EX_APPWINDOW=false`, real foreground loss/recovery, post-recovery durable edit | ToolWindow paper |
| G5-02 | physical 220×120 resize, Source edit/save, Preview selection/scroll, Split source/preview interaction, recoverable geometry | Source/Preview/Split compact |
| G5-03 | Source/Preview/Split 50/100/300% shortcut path, reset, alpha 40, Light→System→Dark→Light config cycle | 13 zoom/opacity/theme captures |
| G5-04 | rendering-stress Preview/Split, real physical wheel to bottom, canonical byte invariance at every boundary | top, deep-bottom, split-bottom |

## Evidence Contract

`g5-exact-qualification.json` binds source commit, harness commit, clean state, version, Windows build,
EXE hash, ZIP hash, ordered group results and every screenshot's repository-relative path plus SHA-256.
Readiness reopens each screenshot, rejects missing/unsafe paths and recomputes its hash. Minimum
artifact counts are 1/3/13/3 for G5-01..04.

The first rendering implementation used `Ctrl+End`, which changed only selection and did not scroll
Preview. Hash review exposed identical top/bottom captures. The corrected path routes a physical
wheel to a cursor explicitly placed inside Preview, batches large scrolls, and rejects identical
top/bottom screenshots. Canonical `note.md` bytes are rechecked after startup, view switches,
selection, capture, wheel and split interaction.

## Development Verification

- `cargo test -p stickymd-smoke --locked`: PASS (89 unit + 2 CLI integration tests).
- Targeted old-candidate G5-01..04: PASS after harness root-cause corrections.
- Full old-candidate G5-01..04 after the final physical-wheel correction: PASS; artifact counts
  1/3/13/3 and Preview top/bottom screenshot hashes differ.
- After moving G5 screenshot assembly out of the generic exact harness, one full diagnostic lost
  foreground focus while entering G5-04. The isolated G5-04 rerun and the following complete G5
  rerun both passed; no product or screenshot-path defect reproduced.
- Screenshot review: deep-bottom capture contains the rendering sentinel, bottom-image region,
  format appendix and oversized placeholder.

## Remaining Human Authority

- Microsoft Pinyin and WeChat IME flows.
- Physical mixed-DPI/multi-monitor/unplug/sleep/RDP behavior.
- Runtime operating-system theme switch.
- First visual approval of typography, formulas, images, placeholders and compact/zoom screenshots.

## Next

Commit the baseline-verified cohesive tooling/docs change, generate a new exact candidate, then
collect clean G3 → G4 → G5 receipts serially. Do not tag, publish or push.
