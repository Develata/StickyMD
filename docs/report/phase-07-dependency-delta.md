# Phase 7 Dependency Delta

## Decision

Phase 7 adds one exact-pinned image codec surface and enables the already approved clipboard crate's
image path. No network client, browser engine, async runtime, GPU UI framework, database or general
attachment framework enters the production graph.

| Crate | Version | License | Purpose | Runtime implication | Replaceability |
| --- | --- | --- | --- | --- | --- |
| `image` | 0.25.10 exact | MIT OR Apache-2.0 | trusted format inspection, bounded decode, PNG normalization | preview worker and image-paste I/O CPU/memory only | isolated behind `stickymd-render::image` |
| `arboard` | 3.6.1 | MIT OR Apache-2.0 | CF_BITMAP/RGBA fallback after native encoded paths | lazy clipboard initialization; Windows-only | clipboard adapter boundary |
| `sha2` | 0.10.9 existing | MIT OR Apache-2.0 | streaming export/managed-file hash | linear file read, no resident cache | existing durable hash authority |
| `windows` features | 0.62.2 resolved | MIT OR Apache-2.0 | clipboard HGLOBAL/HDROP/GDI and `IFileSaveDialog` | thin UI/platform adapters only | Windows API boundary |

## Image Feature Audit

`image` uses `default-features = false` and enables only:

```text
png jpeg webp gif bmp ico
```

PNG/JPEG/WebP/GIF are approved stable inputs. BMP/ICO exist only so non-stable file/bitmap inputs
can be validated and normalized to PNG. AVIF, EXR, TIFF, HDR, QOI, DDS, rayon and encoder/decoder
families outside this list are absent from the Windows runtime feature graph. GIF rendering is
first-frame only and creates no animation timer.

## Arboard Feature Audit

`arboard` disables defaults and enables only `image-data`. On Windows this resolves through
`clipboard-win`, `windows-sys` and `image` BMP/PNG support. StickyMD still reads CF_HDROP,
registered encoded formats and DIB/V5 directly; arboard is reached only for CF_BITMAP because it
provides a small, audited pixel-copy fallback.

## Windows Feature Delta

The `windows` dependency adds only the namespaces required by the approved adapters:

- `Win32_System_Com`
- `Win32_System_DataExchange`
- `Win32_System_Memory`
- `Win32_System_Ole`
- `Win32_Graphics_Gdi`
- `Win32_UI_Shell_Common`

The project does not enable the broad `Win32` umbrella feature.

## Runtime and Cross-Platform Boundary

- `image` lives in `stickymd-render`, which stays safe Rust and cross-platform buildable.
- `arboard` and the new `windows` features are target-specific dependencies of `stickymd-win`.
- `stickymd-core` adds no runtime dependency and remains Windows-free.
- `tools/stickymd-smoke` stays std-only.

## Graph Audit

The normal Windows graph contains no `tauri`, `wry`, `webview`, `tokio`, `async-std`, `wgpu`,
`reqwest`, `hyper`, `ureq`, `curl` or database crate. `Cargo.lock` may retain target-specific
packages selected by non-Windows arboard configurations, but they do not appear in the Windows
product dependency tree.

## Binary Impact

Phase 6 Release EXE: 6,930,944 bytes. Phase 7 Release EXE: 8,072,192 bytes. Delta: +1,141,248 bytes
(+1.088 MiB, +16.47%). This is below the Phase 7 +5 MiB dependency review trigger and remains far
below the v1 portable-package hard limit.
