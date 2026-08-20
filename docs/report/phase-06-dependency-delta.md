# Phase 6 Dependency Delta

## Decision

RaTeX 0.1.14 is integrated only in `stickymd-render`. Versions are exact-pinned. No renderer crate,
PNG codec, browser engine, async runtime, network client or GPU framework was added.

| Crate | Version | License | Purpose | Runtime implication | Replaceability |
| --- | --- | --- | --- | --- | --- |
| `ab_glyph` | 0.2.32 | Apache-2.0 | Read font outlines for the native tiny-skia painter | small CPU-side outline dependency | isolated in `math/painter.rs` |
| `ratex-parser` | 0.1.14 | MIT | KaTeX-compatible math parser | formula work on preview worker | exact-pinned semantic authority |
| `ratex-layout` | 0.1.14 | MIT | math layout and DisplayList conversion | formula work on preview worker | exact-pinned adapter boundary |
| `ratex-types` | 0.1.14 | MIT | DisplayList/color/path data | value types in render crate only | hidden from core/app public state |
| `ratex-font` | 0.1.14 | MIT | RaTeX font IDs/metrics | formula-only | hidden behind painter |
| `ratex-font-loader` | 0.1.14 | MIT | lazy embedded KaTeX font loading | first-formula initialization; fixed font set | replaceable behind painter |
| `ratex-unicode-font` | 0.1.14 | MIT | CJK/emoji fallback faces | system fallback loaded only when requested | replaceable behind painter |
| `ratex-katex-fonts` (transitive) | 0.1.14 | MIT crate / OFL-1.1 fonts | embedded KaTeX TTF assets | main binary size increase | notices checked in |

## Feature Audit

- `ratex-font-loader`: `default-features = false`, only `embed-fonts` enabled.
- `ratex-render` is absent. Therefore its `png` and tiny-skia 0.11 path are absent.
- `ratex-svg`, `ratex-pdf`, `ratex-wasm`, `ratex-cairo`, `ratex-gtk4`, `ratex-ffi` are absent.
- `stickymd-core` has no RaTeX dependency.
- RaTeX's transitive graph adds `sha2` 0.11 beside StickyMD's existing 0.10.9. Cargo-deny treats
  multiple versions as a warning; this is transitive and is not exposed as a durable hash format.

## Font Licensing

The Rust crates report MIT. Embedded `KaTeX_*.ttf` assets are under SIL OFL 1.1. Distribution
evidence is checked in at:

- `THIRD_PARTY_NOTICES.md`
- `assets/licenses/SIL-OFL-1.1.txt`
- `assets/licenses/KaTeX-fonts-NOTICE.txt`

## Rejected Alternatives

- `ratex-render`: public PNG output and tiny-skia 0.11 would create an encode/decode hot path and a
  second raster stack.
- Forking RaTeX or implementing TeX layout: violates semantic authority and increases maintenance.
- Upstream unbounded outline cache: replaced by a small StickyMD-owned 4 MiB bounded cache.

## Binary Baseline

The pre-RaTeX Phase 5 copied Release executable measured **3,495,424 bytes**. The final Phase 6
value and delta are recorded after the final Release build in the Phase 6 report.
