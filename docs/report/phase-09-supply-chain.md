# Phase 9 Dependency, Security, License and SBOM Report

## Result

- Dependency graph: **PASS with reviewed upstream duplicates**.
- Advisory policy: **PASS** (`cargo deny 0.20.2`; no unresolved security advisory).
- Licenses and sources: **PASS**.
- Runtime network/database/async-framework boundary: **PASS**.
- SPDX generation pipeline: implemented; replacement exact-package run pending.

## Frozen inputs

`Cargo.lock` is the release dependency graph. Phase 9 did not run a blanket `cargo update`. The only dependency delta is `winresource 0.1.31`, used only by the Windows build script to compile one manifest, icon and version resource; it is absent from the product runtime graph.

`cargo deny check` reports duplicate-version warnings arising from independently pinned upstream families: RaTeX/rust-embed versus the application SHA stack, Comrak versus RaTeX PHF, cosmic-text versus swash font crates, and Windows crates selected by winit/arboard/notify/tray/softbuffer. Removing them would require upstream-changing upgrades or forks immediately before release and is not justified by measured runtime cost.

The only advisory exception remains `RUSTSEC-2026-0192` for transitive `ttf-parser 0.25.1`; it is an unmaintained-status notice with no safe compatible upgrade and is tracked in `docs/report/RISK-ttf-parser-unmaintained.md`.

## License boundary

Runtime dependency licenses pass `deny.toml`. StickyMD code is MIT. RaTeX code is MIT. Embedded KaTeX-compatible font files remain OFL-1.1 and the portable allowlist includes both the OFL text and font notice. The package contains no Times New Roman, FangSong/仿宋, Consolas or other proprietary system font file.

## SBOM tool

The checked-in script pins Syft 1.50.0 and the Windows amd64 archive SHA-256 `815ee6973ec5dff6a671d7f41b0e78835a8c45b91d5a39f4743ea1cee833d3be`. It also pins and verifies the upstream checksum manifest SHA-256 `bb8824a06c27c625fc103db5d7e9d7131ba2cc6e7c7a79318ee71686ede3c3f0`, then verifies that manifest contains the archive digest. No installer pipe or mutable latest URL is used. The selected version and published assets were checked against the [official Syft v1.50.0 release](https://github.com/anchore/syft/releases/tag/v1.50.0).

The scan context contains the exact `Cargo.lock` plus the extracted portable staging tree. `SYFT_FILE_METADATA_SELECTION=all` ensures the generated SPDX 2.3 document covers the packaged EXE, third-party notice and both font-license files instead of relying on stripped-binary Rust detection alone. Syft itself is not shipped.

## Exact Local RC Evidence

The previous `d02f8a6` SBOM/package evidence is superseded because the runtime notice boundary and
package verifier changed. The replacement SPDX package/file counts, size and checksum are recorded
after the exact clean-source package run. The verifier requires both the SBOM and ZIP as the only
two entries in `SHA256SUMS.txt`.

`Cargo.lock` SHA-256 is
`0c44aa6811f0ef0226a3cc41bddcdebc497a2de7ea13b032f43134f28fabfa25`.
