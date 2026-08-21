# Phase 9 Release Readiness

## Executive Decision

**NOT RC READY**.

The Phase 9 implementation and automated release pipeline are implemented and locally exercised,
but the unchanged warm-startup gate fails and the release-critical manual matrix has no
current-artifact receipts. Independent-review findings for diagnostic file safety, CPU sampling,
leak coverage, runtime notices and package verification were repaired; the exact clean-source
package must still be regenerated. Cold startup passes the original 300 ms gate, so its
USER-authorized 400 ms fallback is not needed.

## Release Blockers

| ID | Severity | Result |
| --- | --- | --- |
| RB-001 warm startup p95 <=180 ms | P0 | **FAIL**, 267.094 ms; no waiver |
| RB-002 Microsoft Pinyin / WeChat IME | P0 | NOT TESTED |
| RB-003 Preview / math / image visual quality | P0 | NOT TESTED |
| RB-004 tray / docking / theme / opacity / physical displays | P0 | NOT TESTED |
| RB-005 clipboard producers / native export / crash / reparse / ACL / clean VM | P0 | NOT TESTED |
| RB-006 user asset / managed-looking fake full safety chain | P0 | NOT TESTED |
| RB-007 exact runtime license notices and package regeneration | P0 | notice generator PASS; exact package pending |
| RB-008 release workflow and supply-chain final static gates | P0 | implementation repaired; final verification pending |

Final performance/resource evidence is closed locally. Remote GitHub workflow execution is
deliberately not treated as a local PASS.

## Startup

| Cohort | Samples | p50 | p95 | max | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| Cold | 20 | 252.337 ms | 268.595 ms | 374.945 ms | PASS <=300 ms |
| Warm | 20 | 254.754 ms | 267.094 ms | 272.364 ms | FAIL >180 ms |

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

- Source / Preview / Split private working-set maxima: 7.758 / 18.238 / 19.504 MiB.
- Hidden-to-tray private working-set maximum: 7.578 MiB.
- Five independent 60-second samples per mode produced Source / Preview / Split / Hidden p95 idle
  CPU of 0.002604 / 0.001302 / 0.002604 / 0.001302%, all below 0.1%.
- 1 MiB source worst measured operation p95: full resync 37.446 ms, PASS <=50 ms.
- Preview p95: 20 KiB 37.242 ms; 100 KiB 263.444 ms; 1 MiB 1.785 s, all PASS.
- 1 MiB persistence end-to-end p95: 9.373 ms.
- 1000 window cycles, 100 autosave/external reloads, 100 dirty conflicts and 100 image cycles changed
  private bytes by +0.641 MiB without monotonic growth; GDI objects were unchanged.

Detailed medians/p95/maxima are in `phase-09-performance-final.md`.

## 4K Image Transient Peak

Peak working set fell from 93.93 to 83.46 MiB and peak private bytes from 79.93 to 65.33 MiB by
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
| Source commit | pending exact clean convergence commit |
| ZIP | pending regeneration |
| ZIP size | pending |
| ZIP SHA-256 | pending |
| EXE size | pending |
| EXE SHA-256 | pending |

The prior `d02f8a6` ZIP is superseded because it predates the exact runtime dependency notices and
stricter verifier. The replacement must contain exactly the EXE, README, MIT license, generated
third-party notice and two KaTeX font-license files under one `StickyMD/` directory. It must contain
no `note/`, user data, PDB or proprietary font.

## Reproducibility Audit

The superseded package proved deterministic archive generation for one EXE input. Determinism and
copied-runtime behavior must be rerun for the replacement clean-source package. Independent
Rust/linker build reproducibility remains **NOT TESTED** and is not claimed.

## SBOM

| Field | Value |
| --- | --- |
| Tool | Syft 1.50.0, pinned archive and checksum manifest |
| Format | SPDX 2.3 JSON |
| File | `SBOM.spdx.json` replacement pending |
| Coverage | pending exact package generation |
| SHA-256 | pending |

## Build and PE Identity

- rustc/cargo 1.97.1, `x86_64-pc-windows-msvc`, LLVM 22.1.6;
- Windows SDK version was not independently recorded;
- x64 PE32+ executable with PerMonitorV2 and asInvoker manifest;
- application icon and StickyMD product/file version are embedded;
- exact replacement EXE/ZIP identity pending.

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

1. Warm startup p95 is 267.094 ms versus the 180 ms gate. The USER must approve more engineering,
   explicitly waive/change this gate, or keep the release blocked.
2. Thirty-two frozen/manual Phase 9 rows remain `NOT TESTED`. Each needs a current-artifact receipt or
   an explicit per-gate USER waiver.
3. `ttf-parser 0.25.1` remains an upstream unmaintained notice, monitored in its risk report.
4. Remote release workflow execution, code signing and independent-build bit reproducibility are
   not claimed.
5. Exact runtime notices, five-sample idle CPU and the full leak-stress cycle implementation pass;
   the exact clean-source ZIP/SBOM and final static gates remain pending.
6. Version remains `0.1.0`; no stable version/tag decision has been taken.

## Stable Release Recommendation

**DO NOT TAG — BLOCKERS REMAIN.**

No tag, push, attestation or GitHub Release should be created until the USER resolves the warm
startup and manual acceptance blockers.
