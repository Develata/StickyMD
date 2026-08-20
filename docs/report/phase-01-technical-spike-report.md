# Phase 1 Technical Spike Report — Rebuilt

- `Date`: 2026-08-20
- `Starting commit`: `3094dbb1f48f`
- `Worktree`: uncommitted takeover rebuild

## Executive Decision

| Path | Decision | Evidence boundary |
| --- | --- | --- |
| Workspace / crate boundaries | **PASS** | core/render Windows-free; experiments outside workspace |
| Window / software framebuffer | **CONDITIONAL** | current dev shell builds and idles within exploratory budget; DPI/opacity/DWM not validated |
| Source / IME | **CONDITIONAL — blocking manual gate** | automated editor pipeline passes; Microsoft Pinyin and WeChat IME NOT TESTED |
| Comrak | **PASS for semantic spike** | 6 automated tests; production Preview not implemented |
| RaTeX | **CONDITIONAL** | formula baseline passes; PNG is spike-only; production painter/API path remains unresolved |
| Portable persistence | **CONDITIONAL** | 9 logic/failure/writability tests + Release adapter smoke pass; junction/non-ASCII Windows case/ACL/kill/power loss not tested |

## Environment

- Microsoft Windows 11 家庭版中文版, version 10.0.26200, build 26200.
- Intel Core i7-12700H, 20 logical processors, 15.8 GiB RAM.
- GPU enumeration was headed by a virtual display adapter; no GPU path is used.
- Rust/Cargo 1.97.1 MSVC.

## Rebuild Scope

The original four experiment crates and stale result files were removed after a verified
repository-external backup. Current retained experiments are:

- `experiments/phase-01/markdown-math`: safe owned projection + automated semantics/math tests;
- `experiments/phase-01/persistence`: mtime-aware recovery, injected transaction failures and a
  conservative Windows adapter.

Window and text duplicate programs were intentionally not rebuilt. The current production dev shell
already supplies the stronger window/source harness; keeping a second 774-line editor with snapshot undo
would create duplicate authority and maintenance burden.

## Automated Results

- Markdown/Math: 6 passed, 0 failed.
- Persistence: 9 passed, 0 failed.
- Persistence Release smoke: canonical directory, atomic first create/replace and cross-process
  mutex/event wake all PASS.
- Forbidden Windows-target dependency scan: 0 matches.
- `cargo deny check`: advisories/bans/licenses/sources PASS under the committed Windows-target policy;
  the exact unmaintained `ttf-parser` advisory is documented and temporarily acknowledged.
- Current production Source shell: five-run memory/idle CPU sample recorded in the performance report.

## Unsafe Baseline

Production `stickymd-core` and `stickymd-render` remain `forbid(unsafe_code)`. Rebuilt Phase 1 unsafe is
limited to `experiments/phase-01/persistence/src/windows_adapter.rs` for Win32 calls; every block states
its pointer/handle/lifetime invariant. No unsafe exists in the Markdown/Math experiment.

## Architecture Findings

- No duplicate document authority is introduced by the retained experiments.
- Comrak Arena and RaTeX DisplayList remain transient projections.
- Unknown atomic-replace errors now fail closed instead of triggering an unconditional overwrite path.
- The frozen architecture is not disproved, but the RaTeX production painter path and real IME behavior are open.

## Blocking Risks

1. Microsoft Pinyin and WeChat IME must be tested with composition, candidate position, selection,
   cancellation, one-step undo, DPI and opacity. Automated winit event tests cannot substitute for this.
2. The original `arrayref` yanked-package finding was disproved by a refreshed registry index and
   isolated fresh-lock check. RaTeX remains conditional because the production hot-path painter
   (without PNG encode/decode) has not yet been selected and verified.
3. `RUSTSEC-2026-0192` marks transitive `ttf-parser 0.25.1` unmaintained with no safe upgrade. The
   exact exception and its release-time exit criteria are recorded in
   `RISK-ttf-parser-unmaintained.md`; it is not treated as a silent clean bill of health.

## Recommendation

**B. APPROVE Phase 2 WITH CONDITIONS** is the only supportable retrospective decision: Phase 2's
platform-independent document model can proceed, but no editor/release gate may interpret Phase 1 as
real-IME PASS, and no production persistence or RaTeX code may copy the spike without closing its stated
conditions.
