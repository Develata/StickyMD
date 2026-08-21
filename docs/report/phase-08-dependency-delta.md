# Phase 8 Dependency Delta

## Production additions

| Crate | Resolved version | License | MSRV | Purpose | Runtime implication |
| --- | --- | --- | --- | --- | --- |
| `tray-icon` | 0.24.2 | MIT OR Apache-2.0 | 1.73 | Native Windows notification-area icon | One native icon plus the project-owned event dispatcher; default features are disabled |
| `crossbeam-channel` | 0.5.16 | MIT OR Apache-2.0 | 1.60 | Blocking tray event hand-off and deterministic shutdown | One bounded/event-driven channel path; no polling and no async runtime |
| `muda` | 0.19.3 | Apache-2.0 OR MIT | 1.73 | Transitive native three-item tray menu | Windows menu objects only; it reuses `crossbeam-channel` |

`tray-icon` and `crossbeam-channel` are Windows target-specific dependencies of `stickymd-win`.
They do not enter `stickymd-core`, `stickymd-render`, or the portable Linux build gate.

## Feature and transitive audit

- `tray-icon` uses `default-features = false`.
- `crossbeam-channel` uses only its `std` feature.
- The Windows target dependency tree contains no GTK, libappindicator, xdo, WebView, Tauri,
  Tokio, WGPU, network client, or database.
- The relevant native transitive set is `muda`, `dpi`, `keyboard-types`, `once_cell`,
  `windows-sys`, and Windows support crates already compatible with the target.
- `windows` remains feature-scoped; Phase 8 adds display/GDI and window-message capabilities, not
  a blanket `Win32` feature.

## License disposition

All direct and transitive additions above are compatible with StickyMD's MIT license. They are
ordinary Rust dependencies recorded by `Cargo.lock` and the release dependency/license report;
Phase 8 adds no bundled binary asset that requires a new standalone notice file.

## Decision

Accepted for the Windows shell. The implementation remains event-driven, target-specific, and
does not weaken the repository's forbidden-architecture boundary.
