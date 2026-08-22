# Phase 9 Release Readiness

## Executive Decision

**NOT RC READY**.

The Phase 9 implementation and automated release pipeline are implemented and locally exercised,
but the unchanged warm-startup gate fails and the release-critical manual matrix has no
current-artifact receipts. Independent-review findings for diagnostic file safety, CPU sampling,
leak coverage, runtime notices and package verification were repaired and the exact clean-source
package was regenerated. Cold startup passes the original 300 ms gate, so its USER-authorized
400 ms fallback is not needed.

## Release Blockers

| ID | Severity | Result |
| --- | --- | --- |
| RB-001 warm startup p95 <=180 ms | P0 | **FAIL**, 342.891 ms; no waiver |
| RB-002 Microsoft Pinyin / WeChat IME | P0 | NOT TESTED |
| RB-003 Preview / math / image visual quality | P0 | NOT TESTED |
| RB-004 tray / docking / theme / opacity / physical displays | P0 | NOT TESTED |
| RB-005 clipboard producers / native export / crash / reparse / ACL / clean VM | P0 | NOT TESTED |
| RB-006 user asset / managed-looking fake full safety chain | P0 | NOT TESTED |
| RB-007 exact runtime license notices and package regeneration | P0 | CLOSED locally on exact clean package |
| RB-008 release workflow and supply-chain final static gates | P0 | CLOSED locally; remote execution NOT EXECUTED |

Package, supply-chain and final performance/resource evidence are closed locally. Remote GitHub
workflow execution is deliberately not treated as a local PASS.

## Startup

| Cohort | Samples | p50 | p95 | max | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| Cold | 20 | 258.771 ms | 277.205 ms | 404.996 ms | PASS <=300 ms |
| Warm | 20 | 325.975 ms | 342.891 ms | 356.433 ms | FAIL >180 ms |

Readiness is signalled after the first usable, visible and presented source frame with IME enabled.
The harness uses a graceful diagnostic exit, nearest-rank percentiles and no trimming. The full
milestone ledger is in `phase-09-startup-hardening.md`.

## Manual Acceptance

| Result | Count |
| --- | ---: |
| MANUAL PASS | 0 |
| NOT TESTED frozen/manual Phase 9 rows | 32 |
| USER WAIVED | 0 |
| Manual FAIL | 0 |

The absence of a manual failure is not evidence of success. Microsoft Pinyin, WeChat IME,
Preview/math/image appearance, themes, opacity, tray, three-edge docking, physical DPI/display
changes, clipboard producers, native export dialog, hard-kill recovery, reparse boundaries and a
clean Windows VM all remain `NOT TESTED`.

## Performance and Memory

- Source / Preview / Split private working-set maxima: 7.785 / 18.215 / 19.578 MiB.
- Hidden-to-tray private working-set maximum: 7.195 MiB.
- Five independent 60-second samples per mode produced Source / Preview / Split / Hidden p95 idle
  CPU of 0.002604 / 0.001302 / 0.005208 / 0.002604%, all below 0.1%.
- 1 MiB source worst measured operation p95: full resync 36.775 ms, PASS <=50 ms.
- Preview p95: 20 KiB 36.408 ms; 100 KiB 174.067 ms; 1 MiB 1.744 s, all PASS.
- 1 MiB persistence end-to-end p95: 8.091 ms.
- 1000 window cycles, 100 autosave/external reloads, 100 dirty conflicts and 100 image cycles changed
  private bytes by +0.527 MiB without monotonic growth; GDI objects were unchanged.

Detailed medians/p95/maxima are in `phase-09-performance-final.md`.

## 4K Image Transient Peak

Peak working set fell from 93.93 to 83.438 MiB and peak private bytes from 79.93 to 65.293 MiB by
dropping the owned encoded buffer before resize allocation. The change uses the existing safe image
API and introduces no custom decoder, unsafe block, GPU path or cache.

## Reliability and Security

Automated failure injection and integration coverage passes for atomic temporary write/flush/replace,
rare ReplaceFileW classification, guarded OCC, recovery evidence, read-only note authority,
missing/invalid note directory, disk-full injection, save/hide/quit barriers, managed-asset ownership,
hard-link export rejection, raw HTML literal projection, remote-image zero-network behavior, custom
URI rejection, malformed math and hostile image metadata.

There is no known data-loss defect in the automated coverage. This statement does not substitute for
the still-missing real hard-kill, ACL, reparse, external-editor and full user-asset manual receipts.

## Dependency, License and Supply Chain

- `Cargo.lock` is frozen; SHA-256:
  `0c44aa6811f0ef0226a3cc41bddcdebc497a2de7ea13b032f43134f28fabfa25`.
- `cargo deny 0.20.2`: advisories/licenses/bans/sources PASS; reviewed duplicate-version warnings
  remain.
- The only ignored advisory is the non-vulnerability unmaintained notice `RUSTSEC-2026-0192` for
  transitive `ttf-parser 0.25.1`; there is no compatible safe convergence.
- Approved runtime licenses include MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, BSL-1.0,
  CC0-1.0, Unicode-3.0, Unicode-DFS-2016 and Zlib. Embedded KaTeX-compatible fonts ship their
  separate OFL-1.1 text and notice.
- No proprietary system font, runtime network client, database, Tokio, Tauri or WebView is present.

## Portable Release Artifact

| Field | Value |
| --- | --- |
| Source commit | `eb687b2441a5816111c116ce30a01bb5b0fba8c6` |
| ZIP | `StickyMD-0.1.0-local-rc-eb687b2441a5-windows-x64-portable.zip` |
| ZIP size | 3,878,842 bytes |
| ZIP SHA-256 | `ef3b503d580fbd587239f9585eeb6195734703cd3abda59c6657f422766b05f9` |
| EXE size | 8,287,744 bytes |
| EXE SHA-256 | `84057a4322c965dbf48646274f2686464f060059a70aeebe1e72264d260c7831` |

The exact ZIP contains the EXE, README, MIT license, generated third-party notice and two KaTeX
font-license files under one `StickyMD/` directory. It contains no `note/`, user data, PDB or
proprietary font. The generated notice is 1,829,345 bytes and covers all 187 registry packages in
the locked normal Windows runtime graph, including Comrak and winit.

## Reproducibility Audit

Two generations from the same clean source commit and built EXE produced the identical ZIP digest.
Independent Rust/linker build reproducibility remains **NOT TESTED** and is not claimed.

## SBOM

| Field | Value |
| --- | --- |
| Tool | Syft 1.50.0, pinned archive and checksum manifest |
| Format | SPDX 2.3 JSON |
| File | `SBOM.spdx.json`, 677,966 bytes |
| Coverage | 337 packages, 12 file records |
| SHA-256 | `757163513bb80f89ee9c30437ca35f4dd3db1de294f64b80a0b50b1daf5343ce` |

## Build and PE Identity

- rustc/cargo 1.97.1, `x86_64-pc-windows-msvc`, LLVM 22.1.6;
- Windows SDK version was not independently recorded;
- x64 PE32+ executable with PerMonitorV2 and asInvoker manifest;
- application icon and StickyMD product/file version are embedded;
- ZIP 3.699 MiB, below the 30 MiB hard gate.

## GitHub Release Workflow

- local YAML syntax/static audit: PASS;
- actions pinned by full commit SHA: PASS;
- checked-in package/SBOM/checksum scripts reused: PASS;
- least-privilege tag-only attest/draft job: PASS by static audit;
- attestation subjects: `SHA256SUMS.txt`, with SPDX SBOM attached to the package;
- automatic stable publication: absent;
- remote workflow execution: **NOT EXECUTED**;
- tag, push and GitHub Release: none.

## Unsafe and Architecture Authority

- `stickymd-core` runtime unsafe: 0 (`#![forbid(unsafe_code)]`);
- `stickymd-render` runtime unsafe: 0 (`#![forbid(unsafe_code)]`);
- Win32 unsafe: 58 localized blocks in 13 platform-adapter files, with adjacent `SAFETY` invariants;
- canonical text owner remains `DocumentState`;
- source/preview/cosmic-text projections remain non-authoritative;
- `WindowShellState` remains the window lifecycle/geometry authority;
- config writes remain revisioned through `ConfigCoordinator`;
- asset deletion remains behind managed-name, full-digest, safe-root and durable-reference proof.

Architecture drift: **None identified**. Phase 9 added no product capability and no compatibility
layer. The startup and 4K changes remove duplicate work/allocation while preserving existing
boundaries.

## Acceptance Matrix

The final AC-001..AC-030 projection is in `docs/acceptance-cases/phase-09.md`. Automated contracts
have current-code evidence; any AC requiring real input methods, visual judgment, native OS UI,
physical displays or failure timing remains `NOT TESTED` at release level.

## Known Issues and USER Decisions Required

1. Warm startup p95 is 342.891 ms versus the 180 ms gate. The USER must approve more engineering,
   explicitly waive/change this gate, or keep the release blocked.
2. Thirty-two frozen/manual Phase 9 rows remain `NOT TESTED`. Each needs a current-artifact receipt or
   an explicit per-gate USER waiver.
3. `ttf-parser 0.25.1` remains an upstream unmaintained notice, monitored in its risk report.
4. Remote release workflow execution, code signing and independent-build bit reproducibility are
   not claimed.
5. Exact runtime notices, five-sample idle CPU, full leak stress, clean-source ZIP/SBOM and final
   local static gates pass. Remote release workflow execution remains unclaimed.
6. Version remains `0.1.0`; no stable version/tag decision has been taken.

## Stable Release Recommendation

**DO NOT TAG — BLOCKERS REMAIN.**

No tag, push, attestation or GitHub Release should be created until the USER resolves the warm
startup and manual acceptance blockers.
