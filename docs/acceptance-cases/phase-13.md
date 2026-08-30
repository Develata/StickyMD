# Phase 13 Acceptance Matrix

本矩阵只验证 exact-candidate 资格化过程，不重新定义或复制产品验收正文。产品行为仍由
[`phase-12.md`](phase-12.md) 的 44 个 exact-artifact 人工 case 与全局 AC 约束。动态结果写入
gitignored `dist/evidence/`；本文件在 candidate freeze 后不回填运行数字。

因此下表是 qualification contract/source baseline。`v0.1.0` 的最终 exact identity、USER
disposition 与 `READY` 结论见
[`../release-notes/0.1.0.md`](../release-notes/0.1.0.md)，不通过改写历史行表达。

## Automated qualification process

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P13-A01 | Phase 13 prompt、task、plan、matrix 与薄 smoke 入口完整存在 | Automated | `cargo run -p stickymd-smoke --locked -- phase 13 --ci` | AUTOMATED PASS |
| P13-A02 | Phase 13 无界面任务加入去重后的 `all --ci` 图 | Automated | `.github/workflows/ci.yml` → `stickymd-smoke all --ci --json` | AUTOMATED PASS |
| P13-A03 | qualification environment 使用 Windows 会话、输入桌面、shell、foreground 与 display 事实 | Automated | `qualification_environment` classification/adapter tests | AUTOMATED PASS |
| P13-A04 | 锁屏/非交互环境被记录为 `NOT_TESTED` / `ENVIRONMENT_BLOCKED`，非产品 FAIL 且非 PASS | Automated | environment status mapping tests + nonzero CLI contract | AUTOMATED PASS |
| P13-A05 | Runtime、Performance、Resources 与人工 recorder 都在有效 GUI 证据前执行环境门 | Automated | Rust task-plan tests and manual coordinator | AUTOMATED PASS |
| P13-A06 | 环境收据只含布尔事实和显示器数量，不含标题、用户名或路径 | Automated | evidence schema serialization test | AUTOMATED PASS |
| P13-A07 | Resources 每个主要场景前重新检查环境 | Automated | resource coordinator task tests | AUTOMATED PASS |
| P13-A08 | Resources 每个已完成场景覆盖写入 partial receipt，未完成状态为 `INCOMPLETE` | Automated | partial evidence serializer/coordinator tests | AUTOMATED PASS |
| P13-A09 | Local Release/package preflight 绑定 Source Freeze；最终 EXE/ZIP/SBOM 仅由 remote artifact promotion 建立 | Automated | Phase 13 `-Release` + `qualification source-freeze` + Phase 14 promotion contract | AUTOMATED PASS |
| P13-A10 | Headless CI receipt 只绑定 Source Freeze 且不运行 GUI 项；不得宣称验证了 final EXE | Automated | Phase 13 local/source campaign `all --ci` stage | AUTOMATED PASS |
| P13-A11 | Local campaign 只产生 Source/Preflight evidence；Promote 后 Runtime、Performance、Resources、G3/G4/G5 作为独立 artifact-bound channel 收集，普通失败不抹除其它安全收据 | Automated | campaign/task-plan + readiness tests | AUTOMATED PASS |
| P13-A12 | Cold/Warm startup hard gate 都是 550 ms；180 ms 为 preferred、400 ms 为 diagnostic engineering target | Automated | startup runtime gate + approved plan/decision projection | AUTOMATED PASS |
| P13-A13 | 所有必需 source-bound 与 artifact-bound 收据必须 identity 匹配、完整且所有 result 为 `PASSED` | Automated | readiness fail-closed receipt validation tests | AUTOMATED PASS |
| P13-A14 | stale source、EXE、ZIP、SBOM 或 incomplete receipt 不参与 readiness | Automated | candidate/readiness identity tests | AUTOMATED PASS |
| P13-A15 | Manual recorder 只接受显式 `MANUAL_PASS` / `MANUAL_FAIL` / `NOT_TESTED` | Automated | manual parser/interactive-token tests | AUTOMATED PASS |
| P13-A16 | M1..M5 只共享 setup；原 P12-M01..P12-M44 每项恰好映射一个 session 并单独记录 | Automated | manual session mapping test | AUTOMATED PASS |
| P13-A17 | Agent 无法产生 blanket waiver；每个 `NOT_TESTED` 仍需具体 USER waiver | Automated | readiness decision contract tests | AUTOMATED PASS |
| P13-A18 | 所有新代码只位于 verification tooling，产品 runtime/dependencies 不变 | Automated | workspace diff/dependency/governance review | AUTOMATED PASS |

## Manual campaign sessions

以下 session 只是 Phase 12 44 个 case 的执行分组；它们不会自动把组内 case 标成 PASS。

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P13-M01 | M1 Editor / IME / Zoom / Math：逐项记录 P12-M01、02、21、24、25、27、31 | Manual | exact candidate `manual-acceptance.json`; session M1 | NOT TESTED |
| P13-M02 | M2 Shell / Tool Window / Dock：逐项记录 P12-M03..20、22、23 | Manual | exact candidate `manual-acceptance.json`; session M2 | NOT TESTED |
| P13-M03 | M3 Clipboard / Images / Export / Recovery：逐项记录 P12-M26、28..30、32、33 | Manual | exact candidate `manual-acceptance.json`; session M3 | NOT TESTED |
| P13-M04 | M4 Multi-Monitor / DPI：逐项记录 P12-M35..40、43 | Manual | exact candidate `manual-acceptance.json`; session M4 | NOT TESTED |
| P13-M05 | M5 Platform / Clean VM：逐项记录 P12-M34、41、42、44 | Manual | exact candidate `manual-acceptance.json`; session M5 | NOT TESTED |

## Current qualification state

Tracked preparation is in progress. `AUTOMATED PASS` 表示仓库内重复入口与约束已由 pre-freeze
baseline 验证；它不等于 exact frozen candidate 的动态收据已经完成。所有人工 session 保持
`NOT TESTED`，直到 USER 在 exact candidate 上逐项观察并由 recorder 写入 ignored receipt。
