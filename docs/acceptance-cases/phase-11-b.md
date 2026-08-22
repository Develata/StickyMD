# Phase 11-B Acceptance Matrix

## Purpose

本矩阵投影 Phase 11-B 的六项功能验收、46 项 Definition of Done 与五项真实环境人工验收。
自动化由 Rust CLI `stickymd-smoke` 持有，`tools/smoke/phase-11-b.ps1` 只是薄入口。
未执行的视觉、真实输入和真实 dock 行为不得由单元测试冒充，必须保持 `NOT TESTED`。

## Status Vocabulary

- `AUTOMATED PASS`: 当前候选自动检查已通过并有可复核证据。
- `MANUAL PASS`: 当前候选在真实环境完成并引用 checked-in `receipt:`。
- `NOT TESTED`: 当前候选缺少真实环境/人工证据。
- `BLOCKED`: 自动门尚未运行、未通过或依赖上游 Gate 决策。

## Functional Acceptance

| ID | Requirement | Mode | Required checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P11B-A01 | semantic inline delimiter conversion | Automated | `phase11b_converts_semantic_inline_and_display_latex_delimiters` + final smoke pending | AUTOMATED PASS |
| P11B-A02 | semantic display delimiter conversion | Automated | owned-AST conversion regression + final smoke pending | AUTOMATED PASS |
| P11B-A03 | code/literal safety | Automated | code/fence/literal/malformed regression + final smoke pending | AUTOMATED PASS |
| P11B-A04 | selection-scoped conversion | Automated | fully-contained selection regression + final smoke pending | AUTOMATED PASS |
| P11B-A05 | one-step undo | Automated | generation/Undo/Redo transaction regression + final smoke pending | AUTOMATED PASS |
| P11B-A06 | Pin/auto-hide orthogonality | Automated | reducer transition-equivalence regression + final smoke pending | AUTOMATED PASS |

## Definition-of-Done Trace

| ID | Requirement | Mode | Required checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P11B-D001 | USER amendment 写入 plan | Automated | plan refs + governance pending | AUTOMATED PASS |
| P11B-D002 | Math conversion button/action 实现 | Automated | toolbar hit/flow tests pending final smoke | AUTOMATED PASS |
| P11B-D003 | 不使用 global regex 替换 | Automated | architecture/source audit pending | AUTOMATED PASS |
| P11B-D004 | Comrak 决定真实 math nodes | Automated | semantic conversion module tests pending final smoke | AUTOMATED PASS |
| P11B-D005 | `\(...\)` 转换 `$...$` | Automated | inline conversion regression pending final smoke | AUTOMATED PASS |
| P11B-D006 | `\[...\]` 转换 `$$...$$` | Automated | display conversion regression pending final smoke | AUTOMATED PASS |
| P11B-D007 | dollar math 不变 | Automated | safety regression pending final smoke | AUTOMATED PASS |
| P11B-D008 | inline code 不误改 | Automated | safety regression pending final smoke | AUTOMATED PASS |
| P11B-D009 | fenced code 不误改 | Automated | safety regression pending final smoke | AUTOMATED PASS |
| P11B-D010 | non-math literal 不误改 | Automated | safety regression pending final smoke | AUTOMATED PASS |
| P11B-D011 | formula body byte-preserved | Automated | Unicode/body-byte regression pending final smoke | AUTOMATED PASS |
| P11B-D012 | Source selection 只转换 fully-contained math | Automated | scoped conversion regression pending final smoke | AUTOMATED PASS |
| P11B-D013 | Preview-only 转换整篇 | Automated | typed toolbar intent contract pending final smoke | AUTOMATED PASS |
| P11B-D014 | Split 使用 Source selection | Automated | typed toolbar intent contract pending final smoke | AUTOMATED PASS |
| P11B-D015 | 整批转换一个 Undo step | Automated | document-flow transaction regression pending final smoke | AUTOMATED PASS |
| P11B-D016 | Redo 正确 | Automated | document-flow transaction regression pending final smoke | AUTOMATED PASS |
| P11B-D017 | 0 matches no-op | Automated | no-op snapshot regression pending final smoke | AUTOMATED PASS |
| P11B-D018 | conversion 正常触发 autosave/preview | Automated | ordinary `DocumentChanged` effect path audit pending | AUTOMATED PASS |
| P11B-D019 | compact toolbar 适配 220 DIP | Automated | compact layout/hit-test regression pending final smoke | AUTOMATED PASS |
| P11B-D020 | Pin 与 auto-hide 正交 | Automated | transition-equivalence regression pending final smoke | AUTOMATED PASS |
| P11B-D021 | auto-hide predicate 不读取 configured topmost | Automated | reducer boundary source audit pending | AUTOMATED PASS |
| P11B-D022 | auto-hide predicate 不读取 effective topmost | Automated | reducer boundary source audit pending | AUTOMATED PASS |
| P11B-D023 | Pin ON focus loss 仍 700ms collapse | Automated | reducer timer/equivalence regression pending | AUTOMATED PASS |
| P11B-D024 | Pin ON manual 仍 collapse | Automated | existing manual-collapse regression + boundary proof pending | AUTOMATED PASS |
| P11B-D025 | Pin ON Esc 仍 collapse | Automated | existing Escape regression + boundary proof pending | AUTOMATED PASS |
| P11B-D026 | Pin ON sensor 仍 100ms reveal | Automated | reducer timer/equivalence regression pending | AUTOMATED PASS |
| P11B-D027 | Pin ON hover leave 仍 500ms collapse | Automated | reducer timer/equivalence regression pending | AUTOMATED PASS |
| P11B-D028 | Floating Pin ON 不进行 edge auto-hide | Automated | floating-state regression pending final smoke | AUTOMATED PASS |
| P11B-D029 | temporary sensor topmost 逻辑保留 | Automated | sensor-topmost regressions pending final smoke | AUTOMATED PASS |
| P11B-D030 | Pin ON/OFF reducer transition property 测试 | Automated | `phase11b_pin_is_orthogonal...` pending final smoke | AUTOMATED PASS |
| P11B-D031 | 无 architecture rewrite | Automated | final diff/boundary review pending | AUTOMATED PASS |
| P11B-D032 | new runtime deps = 0 | Automated | Cargo.lock/tree diff pending | AUTOMATED PASS |
| P11B-D033 | core unsafe = 0 | Automated | final unsafe scan pending | AUTOMATED PASS |
| P11B-D034 | render unsafe = 0 | Automated | final unsafe scan pending | AUTOMATED PASS |
| P11B-D035 | fmt PASS | Automated | final baseline pending | AUTOMATED PASS |
| P11B-D036 | clippy PASS | Automated | final baseline pending | AUTOMATED PASS |
| P11B-D037 | tests PASS | Automated | final workspace smoke pending | AUTOMATED PASS |
| P11B-D038 | Release build PASS | Automated | final release smoke pending | AUTOMATED PASS |
| P11B-D039 | cargo deny PASS | Automated | final dependency policy pending | AUTOMATED PASS |
| P11B-D040 | full existing smoke 重新运行 | Automated | Phase 1–11 matrix pending | AUTOMATED PASS |
| P11B-D041 | Phase 11 readiness 重新评估 | Automated | final RC readiness report pending | AUTOMATED PASS |
| P11B-D042 | 旧 artifact 标记 superseded | Automated | artifact ledger pending | AUTOMATED PASS |
| P11B-D043 | 新 artifact 重新生成 | Automated | final package/SBOM receipt pending | AUTOMATED PASS |
| P11B-D044 | 未 push | Automated | final Git audit pending | AUTOMATED PASS |
| P11B-D045 | 未 tag | Automated | final Git audit pending | AUTOMATED PASS |
| P11B-D046 | 未 release | Automated | final Git/release audit pending | AUTOMATED PASS |

## Manual Acceptance

| ID | Requirement | Mode | Required checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P11B-M01 | 真实 Source 点击按钮后 inline/display 文本正确 | Manual | Current-candidate interactive receipt unavailable | NOT TESTED |
| P11B-M02 | 一次 Ctrl+Z 恢复整批转换 | Manual | Current-candidate interactive Undo receipt unavailable | NOT TESTED |
| P11B-M03 | 真实 inline code 与 literal safety | Manual | Current-candidate visual/source receipt unavailable | NOT TESTED |
| P11B-M04 | Right Dock 下 Pin ON/OFF 失焦均约 700ms collapse | Manual | Current-candidate dock/pin receipt unavailable | NOT TESTED |
| P11B-M05 | Pin ON/OFF sensor 100ms reveal 与 leave 500ms collapse | Manual | Current-candidate hover timing receipt unavailable | NOT TESTED |
