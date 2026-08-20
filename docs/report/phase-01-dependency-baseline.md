# Phase 1 Dependency Baseline — Rebuilt

- `Date`: 2026-08-20
- `Status`: Current for the rebuilt Phase 1 experiments

## Direct Dependencies

| Crate | Locked version | Purpose | License | Runtime implication |
| --- | --- | --- | --- | --- |
| `comrak` | 0.54.0 | Markdown/GFM/math AST | BSD-2-Clause | `default-features = false`; no CLI/syntect |
| `ratex-parser` | 0.1.14 | math parsing | MIT | experimental only |
| `ratex-layout` | 0.1.14 | math layout/display list | MIT | experimental only |
| `ratex-render` | 0.1.14 | spike-only PNG proof | MIT | embeds fonts; not approved as production hot path |
| `sha2` | 0.10.9 | directory/disk identity | MIT OR Apache-2.0 | persistence experiment only |
| `windows` | 0.62.2 | replace + named objects | MIT OR Apache-2.0 | Windows-target-only adapter |

The Markdown/Math all-platform metadata graph contains 119 packages (98 unique names in the Windows
target tree); persistence contains 26 packages (25 unique names in the Windows target tree). This is
one reason these crates remain outside the production workspace.

## Dependency Health Finding

The original Phase 1 report incorrectly classified `arrayref 0.3.9` as yanked. A 2026-08-20 registry
index refresh shows `arrayref 0.3.6` through `0.3.9` as non-yanked, and an isolated fresh-lock check
for the RaTeX 0.1.14 spike resolves successfully. The direct production dependency that had been
added solely to force `arrayref =0.3.9` was therefore removed; `arrayref` remains only as a normal
transitive dependency of `tiny-skia`.

No dependency was silently upgraded and the math engine was not replaced. The spike still does not
vendor or fork RaTeX.

## Feature / Forbidden-Architecture Audit

- Comrak default features are disabled; `syntect` and CLI features are absent from the rebuilt tree.
- Windows-target `cargo tree` scans contain zero tauri, wry, WebView, CEF, Chromium, Tokio,
  async-std, wgpu, iced, egui, slint, GTK, Qt or SDL matches.
- The runtime has no HTTP/network client. Cargo registry access is build-time dependency retrieval.
- Embedded KaTeX-compatible font files still require their upstream OFL/notice review before release;
  crate metadata alone is not a substitute for asset-license verification.
