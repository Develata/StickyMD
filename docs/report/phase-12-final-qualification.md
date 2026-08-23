# Phase 12 Final Qualification Report

## Executive Result

当前状态：**NOT RC READY — local qualification preparation in progress**。

Warm startup hard gate 已由 USER 校准为 400 ms；Phase 11 warm p95 311.353 ms 因而满足
v0.1.0 hard boundary，但没有满足 180 ms preferred target。mandatory manual evidence、release
version、unsigned policy、remote workflow 与 downloaded artifact evidence 尚未完成，不得 tag。

## Source Baseline

- starting commit: `d6ad84a126f218cb22cdcd4a93ff10e03102939c`
- starting branch: `main`
- starting tree: clean
- starting remote relation: `HEAD == origin/main`

最终 `RELEASE_SOURCE_COMMIT`、EXE/ZIP/SBOM/Cargo.lock SHA-256 与 Rust toolchain 由
`dist/evidence/release-candidate.json` 持有；该文件在 source freeze 后生成，不反向修改源码。

## Gate Calibration

| Metric | Preferred | v0.1.0 hard boundary | Latest measured p95 | Result |
| --- | ---: | ---: | ---: | --- |
| Cold startup | 180 ms | 400 ms | 300.692 ms | HARD PASS |
| Warm startup | 180 ms | 400 ms | 311.353 ms | HARD PASS; preferred missed |

Warm 400 ms 是 2026-08-23 USER-approved engineering gate recalibration，不是 waiver。

## Qualification Architecture

- Rust CLI owns task planning、receipt schema、exact-candidate USER decision projection、identity
  checking、manual recorder 与 readiness。
- PowerShell remains a thin stable entry and existing Windows package helper。
- receipts bind exact source commit、EXE、ZIP；stale or dirty evidence fails closed。
- manual recorder requires an interactive terminal and explicit `PASS` / `FAIL` / `NOT TESTED`。
- readiness has no `--force-ready` path。

## Known P0 / P1

- known product P0: 0。
- known automated product P1: 0。
- release blockers: mandatory human evidence and explicit USER/remote gates listed in the decision ledger。

## Automated Evidence

Freeze 后写入 `dist/evidence/automated-qualification.json`。Source-controlled report 不复制临时
receipt 的动态结果，以免制造 tested commit / report commit 循环。

## Manual Evidence

`docs/acceptance-cases/phase-12.md` 汇总 Tier A/B/C。当前全部保持 `NOT TESTED`；只有
`stickymd-smoke acceptance manual` 生成、且 SHA 与 candidate 一致的 receipt 才能参与 readiness。

## Remote / Downloaded Artifact

未授权 push，因而 `remote-workflow.json` 与 `downloaded-artifact-smoke.json` 当前不得伪造。

## Architecture Drift

None observed in Phase 12 preparation. Product runtime dependencies and authority boundaries are unchanged.

## Recommendation

**STOP — complete manual acceptance and pending USER decisions before remote/tag actions.**
