# Phase 11 Manual Acceptance

## Rule

No current-candidate human receipt was produced in this implementation session. Automated native-window
and reducer checks are retained as useful regression evidence but are not promoted to manual PASS.

## Summary

| Matrix | Manual rows | MANUAL PASS | NOT TESTED | FAIL | USER WAIVED |
| --- | ---: | ---: | ---: | ---: | ---: |
| Phase 11 | 21 | 0 | 21 | 0 | 0 |
| Phase 11-B amendment | 5 | 0 | 5 | 0 | 0 |

Inherited Phase 9 and Phase 10 manual rows likewise remain governed by their checked-in matrices; this
report does not rewrite older receipts.

## IME

- Microsoft Pinyin: `NOT TESTED` on the current candidate.
- WeChat Input Method: `NOT TESTED` on the current candidate.
- Candidate positioning, composition cancellation and one-step commit Undo therefore remain release
  acceptance gaps even though deterministic IME transaction tests pass.

## Tool Window and Lifecycle

- taskbar, Alt+Tab, Alt+Tab-away, tray recovery: `NOT TESTED` visually/interactively;
- second-instance activation has automated coverage, but no current human receipt;
- caret off-screen regression has automated coverage and no longer disables the native overlay path.

## Dock and Pin

- Top/Left/Right dock, 24 DIP capture, nearest edge, sensor reachability and no Bottom behavior:
  `NOT TESTED` in a physical pointer/display session;
- Pin ON/OFF reducer transitions are automatically equivalent for focus-loss, manual/Esc, sensor reveal,
  hover leave and floating exclusion;
- actual Right Dock timing with Pin ON/OFF remains `NOT TESTED`.

## Visual and OS Integration

- zoom 50/100/300%, 220x120 DIP, opacity 40, Preview math/images: `NOT TESTED` by human vision;
- traditional clipboard producers and native export dialog: `NOT TESTED`;
- real hard-kill recovery: `NOT TESTED`;
- dual monitor, mixed DPI and display disconnect: `NOT TESTED`.

## Receipt Requirement

Changing a row requires the exact candidate commit and EXE SHA-256, Windows build, display topology/DPI,
input-method version where relevant, ordered steps, observed result and checked-in receipt location.
