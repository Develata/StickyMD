# Phase 3 Dependency Delta

- `Date`: 2026-08-20
- `Status`: Audited for the Phase 3 implementation scope; Release latency measured

| Crate | Version | License | Purpose | Runtime / transitive impact | Why required | Replaceability |
| --- | --- | --- | --- | --- | --- | --- |
| `winit` | 0.30.13 | Apache-2.0 | Windows event loop, keyboard, mouse, IME, DPI | Native window backend; no browser runtime | Frozen architecture direction and Phase 1 API spike | Platform shell adapter can be replaced without changing core |
| `softbuffer` | 0.4.8 | MIT OR Apache-2.0 | Present the CPU framebuffer | Native presentation backend; defaults disabled | Frozen software-rendering direction | Presentation adapter boundary |
| `raw-window-handle` | 0.6.2 | MIT OR Apache-2.0 OR Zlib | Safe handle interoperability between winit and softbuffer | Tiny trait-only bridge | Required by the two selected window crates | Replace with an upstream owned-handle API if one becomes available |
| `cosmic-text` | 0.19.0 | MIT OR Apache-2.0 | Native shaping, fallback, layout, glyph raster | Font database and shaping stack; largest Phase 3 cost to measure | Frozen text direction; hand-written shaping is forbidden | `SourceProjection` boundary |
| `tiny-skia` | 0.12.0 | BSD-3-Clause | CPU painting into one pixmap | PNG feature disabled; CPU paths only | Frozen renderer direction | Paint implementation boundary |
| `unicode-segmentation` | 1.13.3 | MIT OR Apache-2.0 | Unicode grapheme navigation/deletion | Small Unicode table; no runtime service | Correct UAX #29 behavior must not be hand-written | Navigation helper boundary |
| `unicode-script` | 0.5.8 | MIT OR Apache-2.0 | Script classification for font runs | Small generated Unicode table | Correct UAX #24 classification must not be hand-written | Font-run segmenter boundary |
| `arboard` | 3.6.1, default features off | MIT OR Apache-2.0 | Text-only Windows clipboard | Image support disabled; Windows target dependency only | Avoids a larger hand-written unsafe ownership surface | `ClipboardPort` adapter |
| `arrayref` | 0.3.9 (transitive) | BSD-2-Clause | Transitive primitive required by `tiny-skia` | No runtime service; macro-only utility | Registry index refresh confirms 0.3.9 is not yanked; no direct production constraint is needed. | Owned by the `tiny-skia` dependency boundary |

No dependency provides networking, a browser engine, a database, a general async runtime,
or a GPU UI framework. Exact resolved transitive dependencies are governed by `Cargo.lock`
and reviewed again with `cargo tree` at the Phase 3 gate.
