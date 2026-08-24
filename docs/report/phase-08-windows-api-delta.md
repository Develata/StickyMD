# Phase 8 Windows API Delta

## Scope

Phase 8 adds only the native capabilities needed for the desktop shell. All calls remain below
`apps/stickymd-win/src/platform/windows/` or in the smoke-only HWND observer.

| API / facility | Purpose | Runtime unsafe | Boundary |
| --- | --- | --- | --- |
| `QueryDisplayConfig` | enumerate active CCD paths | yes | `monitor.rs` |
| `DisplayConfigGetDeviceInfo` | map GDI source names to stable device paths | yes | `monitor.rs` |
| `GetMonitorInfoW` | obtain signed taskbar-excluded `rcWork` | yes | `monitor.rs` |
| winit Windows message hook | translate display/resume facts and repair winit's malformed synthetic non-client drag payload | yes | `native_message.rs` |
| `GetWindowLongPtrW` / `SetWindowLongPtrW` | preserve and update only `WS_EX_LAYERED` | yes | `window_opacity.rs` |
| `SetLayeredWindowAttributes` | apply whole-window alpha | yes | `window_opacity.rs` |
| `SetWindowPos` | apply configured or temporary topmost without activation | yes | `window_topmost.rs` |
| Windows notification-area/menu APIs through `tray-icon`/`muda` | own one icon and exactly three commands | dependency-owned | `tray.rs` |
| MSVC `/MANIFESTINPUT` linker integration | embed PerMonitorV2 and asInvoker manifest | build-time only | `build.rs` / `StickyMD.manifest` |

The product does not poll foreground state, tray events or display topology. Runtime topology refresh is
triggered only at startup, completed native move/resize, DPI change, display change or resume.

## Safety

Each handwritten unsafe block has an adjacent `SAFETY:` invariant covering handle validity, writable
output storage, integer-only message payloads and retained-resource ownership. Raw `HWND`/`HMONITOR`
values do not enter the pure `flow/window` state model or durable configuration.

## Phase 14 Native-drag Addendum

Phase 14 physical qualification confirmed that winit 0.30.13 posts a stack `POINTS` address where
`WM_NCLBUTTONDOWN` requires packed signed screen coordinates. The existing message-hook adapter now
uses the queued `MSG.pt` and same-thread `SendMessageW` to repair only that queued message before
ordinary dispatch. The adapter does not own window geometry or move-size state; those
remain Win32/winit lifecycle facts consumed by the shell reducer. No Windows feature or runtime crate
was added.
