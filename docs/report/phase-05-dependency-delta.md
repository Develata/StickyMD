# Phase 5 Dependency Delta

## Direct dependency

| Crate | Locked version | License | Rust requirement | Purpose | Runtime implication |
| --- | --- | --- | --- | --- | --- |
| `comrak` | `0.54.0` (exact) | BSD-2-Clause | Rust 1.85 | CommonMark/GFM/math delimiter semantic authority | One transient Arena and full parse per preview generation on the single preview worker |

Workspace Rust is pinned to 1.97.1, so Comrak's declared Rust 1.85 requirement is within the
approved toolchain. `default-features = false` is mandatory: the production tree contains neither
Comrak's CLI nor syntax-highlighting/oniguruma paths.

## Resolved Comrak subtree

| Package | Locked version | License |
| --- | --- | --- |
| `caseless` | 0.2.2 | MIT |
| `finl_unicode` | 1.4.0 | `(MIT OR Apache-2.0) AND Unicode-DFS-2016` |
| `jetscii` | 0.5.3 | MIT OR Apache-2.0 |
| `phf` / `phf_shared` | 0.13.1 | MIT |
| `rustc-hash` | 2.1.3 | Apache-2.0 OR MIT |
| `siphasher` | 1.0.3 | MIT/Apache-2.0 |
| `smallvec` | 1.15.2 | MIT OR Apache-2.0 |
| `typed-arena` | 2.0.2 | MIT |
| `unicode-normalization` | 0.1.25 | MIT OR Apache-2.0 |

`Unicode-DFS-2016` is explicitly allowlisted in `deny.toml`; it is not hidden behind a broad
exception. The final `cargo deny check` accepts licenses, advisories, bans and sources. Existing
duplicate-version findings remain warnings and are unrelated to the Comrak feature choice.

## Boundary and replaceability

- Comrak exists only in `stickymd-render`; `stickymd-core` remains parser- and platform-free.
- Arena nodes never leave `PreviewParser::parse`; application state receives only project-owned,
  generation-tagged projections.
- No network, browser, JavaScript, async runtime, HTML DOM or syntax-highlighting dependency was
  introduced.
- Replacing or upgrading Comrak requires re-running the exact dialect, golden owned-tree,
  10,000-case robustness and Release preview gates. It is not an incidental version bump.
