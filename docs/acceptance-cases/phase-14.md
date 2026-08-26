# Phase 14 Acceptance Matrix

本矩阵验证 release policy / qualification tooling，以及 USER 在 candidate freeze 前明确批准的
Split 同步、Source 查找替换和转换控件标识。任何 runtime 变化都会作废旧候选并触发完整重新资格化。自动项由 std-only Rust CLI
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
| P14-A15 | USER 授权的 candidate defect correction 与资格化加固保持边界内、无新 runtime dependency 且有命名回归 | Automated | git path/dependency audit + source/preview multiline and grapheme selection isolation、source projection/stale-preview/hard-break/theme-math-cache、missing-canonical recovery/note-ack/Keep-Local force-receipt barrier、fixed-temp/recovery/export evidence ownership、native-drag/direct-redock、zoomed toolbar paint/hit alignment、线性 semantic conversion/search、USER rendering-stress Markdown/RaTeX/layout/lazy-image/source-safety regressions | AUTOMATED PASS |
| P14-A16 | exact candidate Release、headless、Runtime、Performance、Resources receipt 独立绑定；runtime smoke 使用有界 reducer、真实 shell/source projection ready gate，并以 fail-closed physical input 驱动快捷键、拖动、失焦、Left -> Top -> Left -> Right dock/auto-hide、Right Pin-ON 正交路径、顶角优先级与真实 compact resize | Automated | Phase 10 targeted Runtime、Phase 14 qualification campaign、window-stress parser/runtime contract tests；动态 receipt 不回写本表 | AUTOMATED PASS |
| P14-A17 | CI tests/performance 分片并发时，任务并集严格等于完整 `all --ci` 且失败日志同时保留 test stdout 与 Cargo stderr | Automated | smoke CLI shard-union/output-capture tests + `.github/workflows/ci.yml` | AUTOMATED PASS |
| P14-A18 | 日常 Resources 可按 source-preview/math/images/window/zoom 定向运行；完整候选仍要求全矩阵 | Automated | `phase-14.ps1 -Resources -ResourceModule <module>` + smoke task-plan tests | AUTOMATED PASS |
| P14-A19 | Portable Release 静态链接 MSVC CRT，Rust CLI 同时检查普通与 delay-load PE imports，CI 在打包前拒绝外置 developer runtime | Automated | `.cargo/config.toml` + `qualification native-runtime` + CI/release workflow trace | AUTOMATED PASS |
| P14-A20 | Split 语义同步默认开启、可持久关闭；双向手势单向映射、stale generation guard、无反馈环且保留独立位置 | Automated | render anchor-index + app reducer/config + Rust CLI regression | AUTOMATED PASS |
| P14-A21 | Source 纯文本查找/替换支持大小写开关、wrap、单次/全部替换、generation invalidation、Unicode boundary 与单事务 Undo；关闭后释放匹配投影且不再扫描，外部重载刷新打开会话，搜索焦点不吞掉全局保存/导出；不含正则 | Automated | interaction/flow/core tests + Rust CLI workspace-test shard regression | AUTOMATED PASS |
| P14-A22 | 数学分隔符转换控件以清晰的 `$` 标识，paint/hit geometry 在 50/100/300% 一致 | Automated | toolbar paint/hit contract + headless CI | AUTOMATED PASS |
| P14-A23 | Release 内存按 Source/Preview/Split/cache 分模块归因；优化决策有实测依据且不放宽既有 hard gate | Automated | targeted resource modules + `docs/report/phase-14-memory-attribution.md` | AUTOMATED PASS |

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
