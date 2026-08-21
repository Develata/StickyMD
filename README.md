# StickyMD

StickyMD is a native, portable Windows 11 Markdown scratchpad: launch it, write, and let it save the single note beside the executable. It is deliberately not a general-purpose editor or knowledge-management system.

> Pre-release status: Phase 9 implementation is complete, but release validation is not. Warm startup and the manual IME, visual, tray, DPI, multi-monitor, recovery, and clean-VM acceptance rows remain open. No RC-ready or stable release is claimed.

## What it does

- Edits exactly `<program-dir>/note/note.md`.
- Uses a native Rust UI with no WebView, Electron, Tauri, JavaScript runtime, database, telemetry, updater, or runtime network client.
- Autosaves through guarded, same-directory atomic replacement and detects external edits.
- Renders CommonMark/GFM through Comrak and native RaTeX-compatible mathematics.
- Supports managed local-image paste/export, source/preview/split views, tray lifecycle, left/right/top docking, themes, topmost, and whole-window opacity.

## Portable use

1. Extract the package to a writable directory owned by the current user.
2. Run `StickyMD.exe` without administrator privileges.
3. Keep the generated `note/` directory with the executable when moving or copying the note.

One directory is one note identity. A second process from the same canonical directory wakes the existing instance; copies in different directories are independent. Do not install StickyMD under `Program Files`.

The current builds are unsigned. Windows may display a reputation warning. Verify `SHA256SUMS.txt` before running a release artifact; advanced users can also verify GitHub artifact attestations with `gh attestation verify`.

## Build from source

Requirements: Windows 11 x64, the MSVC C++/Windows SDK build tools, and the toolchain pinned by `rust-toolchain.toml`.

```powershell
cargo build --workspace --release --locked
```

Run the CI-safe automated matrix with:

```powershell
./tools/smoke/all.ps1 -Ci
```

Machine-dependent GUI, IME, display-topology, resource and visual checks are intentionally separate and remain `NOT TESTED` until a current-artifact receipt exists.

## Documentation

- [中文说明](README.zh-CN.md)
- [Architecture contract](docs/plan/)
- [Acceptance contract](docs/acceptance-cases/00_v1_acceptance.md)
- [Phase 9 matrix](docs/acceptance-cases/phase-09.md)
- [Release checklist](docs/release-checklist.md)
- [Third-party notices](THIRD_PARTY_NOTICES.md)
- [Security policy](SECURITY.md)
- [Contributing](CONTRIBUTING.md)

## License

StickyMD is MIT licensed. Embedded KaTeX-compatible font files retain the SIL Open Font License 1.1; see the packaged notices.
