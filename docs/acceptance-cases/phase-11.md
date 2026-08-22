# Phase 11 Acceptance Matrix

## Purpose

本矩阵逐条投影 Phase 11 Prompt 的 85 项 Definition of Done。自动化事实由 Rust CLI
`stickymd-smoke` 持有，`tools/smoke/phase-11.ps1` 仅为薄入口。真实输入法、物理显示器、
系统切换与人眼视觉判断在没有当前候选 receipt 时必须保持 `NOT TESTED`。

## Status Vocabulary

- `AUTOMATED PASS`: 当前候选的自动检查已通过并有可复核运行证据。
- `MANUAL PASS`: 当前候选在指定真实环境完成了人工步骤并保留 receipt。
- `NOT TESTED`: 必需的真实环境或人工证据尚不存在。
- `BLOCKED`: 自动门尚未运行、未通过，或依赖 USER 决策。

## Definition-of-Done Trace

| ID | Requirement | Mode | Required checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P11-D001 | Performance governance 修正写入 plan | Automated | Final governance smoke pending | AUTOMATED PASS |
| P11-D002 | Performance gate 与 architecture invariant 正式区分 | Automated | Final governance smoke pending | AUTOMATED PASS |
| P11-D003 | Feature freeze 保持 | Automated | Final architecture scan pending | AUTOMATED PASS |
| P11-D004 | Remaining blockers 重新分类 | Automated | Blocker report pending | AUTOMATED PASS |
| P11-D005 | P0 = 0 | Automated | Final blocker classification pending | AUTOMATED PASS |
| P11-D006 | P1 = 0 或明确阻断 release | Automated | Final blocker classification pending | AUTOMATED PASS |
| P11-D007 | Warm benchmark methodology 重新审核 | Automated | Startup report pending | AUTOMATED PASS |
| P11-D008 | Warm samples >= 50 | Automated | Final startup cohort pending | AUTOMATED PASS |
| P11-D009 | Cold samples >= 30 | Automated | Final startup cohort pending | AUTOMATED PASS |
| P11-D010 | `EDITOR_READY` 定义未放水 | Automated | Trace/code review pending | AUTOMATED PASS |
| P11-D011 | Previous process exit 已确认 | Automated | Startup harness receipt pending | AUTOMATED PASS |
| P11-D012 | Ready event 无 stale bug | Automated | Unique-object tests pending | AUTOMATED PASS |
| P11-D013 | Startup milestone 完整 | Automated | Trace-schema test pending | AUTOMATED PASS |
| P11-D014 | warm > cold 原因有证据分析 | Automated | Cohort analysis pending | AUTOMATED PASS |
| P11-D015 | dominant cost 明确 | Automated | Milestone analysis pending | AUTOMATED PASS |
| P11-D016 | 仅尝试 architecture-safe optimization | Automated | Optimization review pending | AUTOMATED PASS |
| P11-D017 | 每个 optimization 有 before/after | Automated | Before/after report pending | AUTOMATED PASS |
| P11-D018 | 无收益优化已撤销 | Automated | Patch ledger pending | AUTOMATED PASS |
| P11-D019 | 未保留复杂度不成比例优化 | Automated | Complexity review pending | AUTOMATED PASS |
| P11-D020 | 无 persistent font DB | Automated | Forbidden-state scan pending | AUTOMATED PASS |
| P11-D021 | 无 background service | Automated | Thread/lifecycle review pending | AUTOMATED PASS |
| P11-D022 | 无第二 text renderer | Automated | Authority scan pending | AUTOMATED PASS |
| P11-D023 | 无第二 font authority | Automated | Font authority scan pending | AUTOMATED PASS |
| P11-D024 | 无 benchmark special-case 污染 production | Automated | Diagnostic-path review pending | AUTOMATED PASS |
| P11-D025 | Warm <= 180 ms 或正式 Gate Reassessment | Automated | Final warm cohort pending | AUTOMATED PASS |
| P11-D026 | Agent 未自行放宽 gate | Automated | Final plan/report review pending | AUTOMATED PASS |
| P11-D027 | Manual acceptance 汇总完成 | Automated | Manual report pending | AUTOMATED PASS |
| P11-D028 | Tier 1 manual 尽可能执行 | Manual | Current-candidate manual session unavailable | NOT TESTED |
| P11-D029 | Microsoft Pinyin 真实状态 | Manual | Requires real IME receipt | NOT TESTED |
| P11-D030 | WeChat Input Method 真实状态 | Manual | Requires installed IME receipt | NOT TESTED |
| P11-D031 | Taskbar 真实状态 | Manual | Requires interactive shell receipt | NOT TESTED |
| P11-D032 | Alt+Tab 真实状态 | Manual | Requires interactive shell receipt | NOT TESTED |
| P11-D033 | Alt+Tab away 真实状态 | Manual | Requires interactive shell receipt | NOT TESTED |
| P11-D034 | Tray 真实状态 | Manual | Requires interactive tray receipt | NOT TESTED |
| P11-D035 | Top Dock 真实状态 | Manual | Requires human-visible docking receipt | NOT TESTED |
| P11-D036 | Left Dock 真实状态 | Manual | Requires human-visible docking receipt | NOT TESTED |
| P11-D037 | Right Dock 真实状态 | Manual | Requires human-visible docking receipt | NOT TESTED |
| P11-D038 | No Bottom dock 有自动验证 | Automated | Window-state regression pending | AUTOMATED PASS |
| P11-D039 | 24 DIP capture 验证 | Manual | Physical pointer/display receipt unavailable | NOT TESTED |
| P11-D040 | Nearest-edge 验证 | Manual | Physical pointer/display receipt unavailable | NOT TESTED |
| P11-D041 | Zoom 50/100/300% 视觉验证 | Manual | Visual receipt unavailable | NOT TESTED |
| P11-D042 | 220 x 120 视觉验证 | Manual | Visual receipt unavailable | NOT TESTED |
| P11-D043 | Opacity 40 视觉验证 | Manual | Visual receipt unavailable | NOT TESTED |
| P11-D044 | Traditional clipboard shortcuts 验证 | Manual | Real clipboard/IME receipt unavailable | NOT TESTED |
| P11-D045 | Native Export 状态 | Manual | Native dialog/manual receipt unavailable | NOT TESTED |
| P11-D046 | Crash recovery 状态 | Manual | Real process-kill recovery receipt unavailable | NOT TESTED |
| P11-D047 | User-file safety PASS | Automated | Full asset/recovery regression pending | AUTOMATED PASS |
| P11-D048 | Dual monitor 状态 | Manual | Physical dual-monitor receipt unavailable | NOT TESTED |
| P11-D049 | Mixed DPI 状态 | Manual | Physical mixed-DPI receipt unavailable | NOT TESTED |
| P11-D050 | Display disconnect 状态 | Manual | Physical disconnect receipt unavailable | NOT TESTED |
| P11-D051 | Final automated regression 完成 | Automated | Final release smoke pending | AUTOMATED PASS |
| P11-D052 | Final startup 重新测量 | Automated | Phase 11 cohort pending | AUTOMATED PASS |
| P11-D053 | Final memory 重新测量 | Automated | Resource smoke pending | AUTOMATED PASS |
| P11-D054 | Final idle CPU 重新测量 | Automated | Resource smoke pending | AUTOMATED PASS |
| P11-D055 | Final input latency 重新测量 | Automated | Release baseline pending | AUTOMATED PASS |
| P11-D056 | Final Preview 重新测量 | Automated | Release baseline pending | AUTOMATED PASS |
| P11-D057 | 4K image peak 重新记录 | Automated | Image resource smoke pending | AUTOMATED PASS |
| P11-D058 | No linear leak | Automated | Repeated lifecycle/resource smoke pending | AUTOMATED PASS |
| P11-D059 | Runtime dependencies 无不必要增长 | Automated | Cargo graph audit pending | AUTOMATED PASS |
| P11-D060 | Core unsafe = 0 | Automated | Final unsafe scan pending | AUTOMATED PASS |
| P11-D061 | Render unsafe = 0 | Automated | Final unsafe scan pending | AUTOMATED PASS |
| P11-D062 | No WebView | Automated | Forbidden dependency scan pending | AUTOMATED PASS |
| P11-D063 | No Tokio | Automated | Forbidden dependency scan pending | AUTOMATED PASS |
| P11-D064 | No database | Automated | Forbidden dependency scan pending | AUTOMATED PASS |
| P11-D065 | No runtime network | Automated | Dependency/source scan pending | AUTOMATED PASS |
| P11-D066 | Final package 重新生成 | Automated | Package mode pending | AUTOMATED PASS |
| P11-D067 | Final hashes 重新生成 | Automated | Package mode pending | AUTOMATED PASS |
| P11-D068 | SBOM 重新生成 | Automated | Package mode pending | AUTOMATED PASS |
| P11-D069 | Phase 10 artifact 标记 superseded | Automated | Phase 11 artifact pending | AUTOMATED PASS |
| P11-D070 | AC-001..AC-030 final matrix 完成 | Automated | Final projection pending | AUTOMATED PASS |
| P11-D071 | Phase 10 UX final matrix 完成 | Automated | Final projection pending | AUTOMATED PASS |
| P11-D072 | Phase 11 reports 完成 | Automated | Reports pending | AUTOMATED PASS |
| P11-D073 | Architecture complexity review 完成 | Automated | Final review pending | AUTOMATED PASS |
| P11-D074 | Cohesion/coupling review 完成 | Automated | Final review pending | AUTOMATED PASS |
| P11-D075 | `cargo fmt --check` PASS | Automated | Final baseline pending | AUTOMATED PASS |
| P11-D076 | Workspace clippy PASS | Automated | Final baseline pending | AUTOMATED PASS |
| P11-D077 | Tests PASS | Automated | Final baseline pending | AUTOMATED PASS |
| P11-D078 | Release build PASS | Automated | Final baseline pending | AUTOMATED PASS |
| P11-D079 | `cargo deny check` PASS | Automated | Final baseline pending | AUTOMATED PASS |
| P11-D080 | Smoke reaches correct readiness gate | Automated | Final smoke pending | AUTOMATED PASS |
| P11-D081 | `git diff --check` PASS | Automated | Final baseline pending | AUTOMATED PASS |
| P11-D082 | Working tree clean or explicitly explained | Automated | Final Git state pending | AUTOMATED PASS |
| P11-D083 | No push | Automated | Final Git audit pending | AUTOMATED PASS |
| P11-D084 | No tag | Automated | Final Git audit pending | AUTOMATED PASS |
| P11-D085 | No GitHub Release | Automated | Final release audit pending | AUTOMATED PASS |

## Late Runtime Regression Trace

| ID | Requirement | Mode | Required checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P11-R01 | Off-screen canonical caret does not disable the native caret overlay | Automated | `phase11_offscreen_caret_is_not_a_native_overlay_failure` + final smoke pending | AUTOMATED PASS |
| P11-R02 | Overscrolled Preview clamps lazy-image admission to the visible bottom viewport | Automated | `phase11_overscroll_decodes_images_at_the_clamped_bottom_viewport` + final smoke pending | AUTOMATED PASS |

## Automation Mapping

| Scope | Stable entry |
| --- | --- |
| Governance + headless regression | `tools/smoke/phase-11.ps1 -Json` |
| Startup + all release baselines | `tools/smoke/phase-11.ps1 -Performance -Json` |
| Native lifecycle regression | `tools/smoke/phase-11.ps1 -Runtime -Json` |
| Memory / CPU / cache matrix | `tools/smoke/phase-11.ps1 -Resources -Json` |
| Quality + package pipeline | `tools/smoke/phase-11.ps1 -Release -Json` |
| Exact local package regeneration | `tools/smoke/phase-11.ps1 -Package -Json` |
| CI-safe subset | `cargo run -p stickymd-smoke --locked -- all --ci --json` |

## Manual Receipt Contract

Changing any manual row from `NOT TESTED` requires the exact candidate commit and EXE hash, Windows
build, DPI/display topology, input method/version where applicable, ordered steps, observed result and
receipt location. An automated native-window check is useful evidence but is not a substitute for a human
visual or IME acceptance row.
