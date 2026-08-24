# Phase 14 Acceptance Matrix

本矩阵验证 release policy / qualification tooling；不新增产品功能。自动项由 std-only Rust CLI
持有并由 CI 调用。人工项始终保留 `NOT TESTED`，只有 ignored exact-candidate receipt 可改变
readiness，不能改写此 Markdown 状态。

## Automated qualification

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P14-A01 | Phase 14 stable PowerShell entry、task、report、matrix、guided manual guide 存在 | Automated | `stickymd-smoke phase 14 --ci` | AUTOMATED PASS |
| P14-A02 | Phase 14 加入 deduplicated headless CI graph | Automated | `stickymd-smoke all --ci --json` | AUTOMATED PASS |
| P14-A03 | cold/warm startup 同时报告 180/400/550 ms；仅 >550 ms hard fail | Automated | qualification/runtime unit tests | AUTOMATED PASS |
| P14-A04 | Performance ordinary failure 不跳过 Resources | Automated | campaign policy unit tests | AUTOMATED PASS |
| P14-A05 | Resources failure 不抹除 Performance receipt | Automated | campaign policy unit tests | AUTOMATED PASS |
| P14-A06 | Runtime ordinary failure 仍保留后续独立、安全 receipt | Automated | campaign policy unit tests | AUTOMATED PASS |
| P14-A07 | invalid environment、identity mismatch、P0/data-safety failure 全局停止 | Automated | campaign policy unit tests | AUTOMATED PASS |
| P14-A08 | startup attribution 使用 per-sample milestone intervals 并输出唯一 decision | Automated | attribution parser/classifier tests | AUTOMATED PASS |
| P14-A09 | Tier A `NOT TESTED` 阻断；PASS 或 explicit waiver 才 eligible | Automated | readiness tests | AUTOMATED PASS |
| P14-A10 | Tier B group waiver 必须绑定 version/source；Tier C NT 仅在 automation PASS 时 nonblocking | Automated | readiness tests | AUTOMATED PASS |
| P14-A11 | v0.1.0 waiver 不适用于 v0.1.1 | Automated | readiness version-binding test | AUTOMATED PASS |
| P14-A12 | manual receipt 绑定 exact source/EXE/ZIP/version/Windows build/session/case | Automated | manual receipt tests | AUTOMATED PASS |
| P14-A13 | unsigned package 明确记录且不伪造 Authenticode signed fields | Automated | release/package contract tests | AUTOMATED PASS |
| P14-A14 | GitHub-hosted CI 不执行 absolute 550 ms/resource qualification | Automated | workflow governance trace | AUTOMATED PASS |
| P14-A15 | product runtime/dependency delta 为零 | Automated | git path/dependency audit | AUTOMATED PASS |
| P14-A16 | exact candidate Release、headless、Runtime、Performance、Resources receipt 独立绑定；hidden-window stress 使用有界 reducer 与真实 shell/source projection ready gate | Automated | Phase 14 qualification campaign、window-stress parser/runtime contract tests；动态 receipt 不回写本表 | AUTOMATED PASS |

## Guided manual sessions

下列 session 是交互记录入口，不替代 `phase-12.md` 的 P12-M01..M44 authority。

| ID | Scope | Mode | Underlying cases / Evidence | Status |
| --- | --- | --- | --- | --- |
| P14-G1 | Editor / IME / rendering | Guided Manual | P12-M01,M02,M21,M22,M24..M27,M31 | NOT TESTED |
| P14-G2 | ToolWindow / tray / dock / theme | Guided Manual | P12-M03..M20,M23 | NOT TESTED |
| P14-G3 | clipboard / export / recovery / asset safety | Guided Manual | P12-M28..M30,M32,M33 | NOT TESTED |

## Readiness interpretation

- Tier A manual facts require exact PASS or explicit case/group waiver。
- Tier B requires exact PASS or version/source-bound USER disposition。
- Tier C `NOT TESTED` is nonblocking only while corresponding automated coverage is PASS；FAIL blocks。
- PUSH、TAG、DRAFT-RELEASE、PUBLISH 均不由本矩阵授权。
