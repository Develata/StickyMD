# Phase 3 Manual IME Checklist

- `Status`: NOT TESTED
- `Executable`: `target/release/stickymd-win.exe`
- `Expected title`: `StickyMD Phase 3 — SOURCE EDITOR — NOT PERSISTED`

## Preparation

1. Build with `cargo build --release --locked`.
2. Start the executable and confirm the `NOT PERSISTED` warning is visible.
3. Do not enter irreplaceable text: this development shell intentionally has no save path.
4. Record Windows build, active input-method version, monitor scale, and result for every row.

## Result Matrix

Use only `PASS`, `FAIL`, or `NOT TESTED`.

| Test | Microsoft Pinyin | WeChat IME | Notes |
| --- | --- | --- | --- |
| basic `nihao` → `你好` commit | NOT TESTED | NOT TESTED | |
| continuous Chinese input | NOT TESTED | NOT TESTED | |
| mixed `这是 Rust 的 trait` | NOT TESTED | NOT TESTED | |
| candidate window near caret | NOT TESTED | NOT TESTED | |
| selection replaced by composition | NOT TESTED | NOT TESTED | |
| left/right during composition | NOT TESTED | NOT TESTED | |
| Backspace during composition | NOT TESTED | NOT TESTED | |
| Esc cancels without document change | NOT TESTED | NOT TESTED | |
| one Ctrl+Z removes full commit | NOT TESTED | NOT TESTED | |
| refocus then compose | NOT TESTED | NOT TESTED | |
| move/resize then compose | NOT TESTED | NOT TESTED | |
| 150% DPI candidate position | NOT TESTED | NOT TESTED | |
| 200% DPI candidate position | NOT TESTED | NOT TESTED | |

## Failure Receipt

For any failure, record exact steps, Windows/input-method versions, DPI, text before/after, whether
the error affects preedit, commit, caret, candidate position, or undo, and whether it reproduces
after restarting the development shell. Do not enable RichEdit as an automatic workaround.
