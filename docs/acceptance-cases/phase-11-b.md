# Phase 11-B Acceptance Matrix

## Purpose

本矩阵投影 Phase 11-B 的六项功能验收、46 项 Definition of Done 与五项真实环境人工验收。
自动化由 Rust CLI `stickymd-smoke` 持有，`tools/smoke/phase-11-b.ps1` 只是薄入口。
未执行的视觉、真实输入和真实 dock 行为不得由单元测试冒充，必须保持 `NOT TESTED`。

下表状态保留该阶段的 source baseline，不代表当前版本的动态 verdict。后续 exact-candidate
收据按 [plan 11](../plan/11_testing_and_release.md) 写入 ignored `dist/evidence/`，不得事后
回填这些历史行。各版本身份、处置与未验收项见[发布清单](../release-checklist.md)。

## Status Vocabulary

- `AUTOMATED PASS`: 对该行记录的源码或候选，自动检查已通过并有可复核证据。
- `MANUAL PASS`: 对该行记录的候选，在真实环境完成并保留正式 receipt；tracked 行必须有 `receipt:` 引用，不意味着 ignored 收据文件必须入库。
- `NOT TESTED`: 该行记录的候选缺少真实环境/人工证据。
- `BLOCKED`: 自动门尚未运行、未通过或依赖上游 Gate 决策。

## Functional Acceptance

| ID | Requirement | Mode | Evidence / entry | Status |
| --- | --- | --- | --- | --- |
| P11B-A01 | semantic inline delimiter conversion | Automated | named regression + exact all-CI receipt | AUTOMATED PASS |
| P11B-A02 | semantic display delimiter conversion | Automated | owned-AST regression + runtime receipt | AUTOMATED PASS |
| P11B-A03 | code/literal safety | Automated | code/fence/literal/malformed regressions | AUTOMATED PASS |
| P11B-A04 | selection-scoped conversion | Automated | fully-contained selection regression | AUTOMATED PASS |
| P11B-A05 | one-step undo | Automated | generation/Undo/Redo transaction regression | AUTOMATED PASS |
| P11B-A06 | Pin/auto-hide orthogonality | Automated | all-edge reducer transition-equivalence regression + copied-Release Right-edge Pin-ON focus-loss lifecycle | AUTOMATED PASS |

## Definition-of-Done Trace

| ID | Requirement | Mode | Required checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P11B-D001 | USER amendment 写入 plan | Automated | plan refs + exact governance PASS | AUTOMATED PASS |
| P11B-D002 | Math conversion button/action 实现 | Automated | toolbar flow tests + copied-runtime lifecycle | AUTOMATED PASS |
| P11B-D003 | 不使用 global regex 替换 | Automated | semantic module source audit | AUTOMATED PASS |
| P11B-D004 | Comrak 决定真实 math nodes | Automated | owned-AST semantic tests | AUTOMATED PASS |
| P11B-D005 | `\(...\)` 转换 `$...$` | Automated | inline conversion regression | AUTOMATED PASS |
| P11B-D006 | `\[...\]` 转换 `$$...$$` | Automated | display conversion regression | AUTOMATED PASS |
| P11B-D007 | dollar math 不变 | Automated | safety regression | AUTOMATED PASS |
| P11B-D008 | inline code 不误改 | Automated | safety regression | AUTOMATED PASS |
| P11B-D009 | fenced code 不误改 | Automated | safety regression | AUTOMATED PASS |
| P11B-D010 | non-math literal 不误改 | Automated | malformed/literal regression | AUTOMATED PASS |
| P11B-D011 | formula body byte-preserved | Automated | Unicode/multiline byte regression | AUTOMATED PASS |
| P11B-D012 | Source selection 只转换 fully-contained math | Automated | scoped conversion regression | AUTOMATED PASS |
| P11B-D013 | Preview-only 转换整篇 | Automated | typed intent contract + runtime | AUTOMATED PASS |
| P11B-D014 | Split 使用 Source selection | Automated | typed toolbar intent contract | AUTOMATED PASS |
| P11B-D015 | 整批转换一个 Undo step | Automated | document-flow transaction regression | AUTOMATED PASS |
| P11B-D016 | Redo 正确 | Automated | document-flow transaction regression | AUTOMATED PASS |
| P11B-D017 | 0 matches no-op | Automated | no-op snapshot regression | AUTOMATED PASS |
| P11B-D018 | conversion 正常触发 autosave/Source projection/Preview | Automated | ordinary `DocumentChanged` path、immediate source-layout regression、visible-mode relayout regression + copied-runtime lifecycle | AUTOMATED PASS |
| P11B-D019 | compact toolbar 适配 220 DIP | Automated | compact geometry/hit-test regression | AUTOMATED PASS |
| P11B-D020 | Pin 与 auto-hide 正交 | Automated | Left/Top/Right transition-equivalence regression + Right-edge Pin-ON runtime | AUTOMATED PASS |
| P11B-D021 | auto-hide predicate 不读取 configured topmost | Automated | reducer boundary source audit | AUTOMATED PASS |
| P11B-D022 | auto-hide predicate 不读取 effective topmost | Automated | reducer boundary source audit | AUTOMATED PASS |
| P11B-D023 | Pin ON focus loss 仍 700ms collapse | Automated | all-edge reducer timer/equivalence regression + Right-edge real focus-loss runtime | AUTOMATED PASS |
| P11B-D024 | Pin ON manual 仍 collapse | Automated | manual-collapse regression + boundary proof | AUTOMATED PASS |
| P11B-D025 | Pin ON Esc 仍 collapse | Automated | Escape regression + boundary proof | AUTOMATED PASS |
| P11B-D026 | Pin ON sensor 仍 100ms reveal | Automated | all-edge reducer timer/equivalence regression + Right-edge sensor runtime | AUTOMATED PASS |
| P11B-D027 | Pin ON hover leave 仍 500ms collapse | Automated | reducer timer/equivalence regression | AUTOMATED PASS |
| P11B-D028 | Floating Pin ON 不进行 edge auto-hide | Automated | floating-state regression | AUTOMATED PASS |
| P11B-D029 | temporary sensor topmost 逻辑保留 | Automated | sensor-topmost regressions | AUTOMATED PASS |
| P11B-D030 | Pin ON/OFF reducer transition property 测试 | Automated | named transition-equivalence test | AUTOMATED PASS |
| P11B-D031 | 无 architecture rewrite | Automated | final diff/boundary review | AUTOMATED PASS |
| P11B-D032 | new runtime deps = 0 | Automated | Cargo.lock/tree diff | AUTOMATED PASS |
| P11B-D033 | core unsafe = 0 | Automated | exact unsafe scan | AUTOMATED PASS |
| P11B-D034 | render unsafe = 0 | Automated | exact unsafe scan | AUTOMATED PASS |
| P11B-D035 | fmt PASS | Automated | `phase-11-release-final.json` | AUTOMATED PASS |
| P11B-D036 | clippy PASS | Automated | `phase-11-release-final.json` | AUTOMATED PASS |
| P11B-D037 | tests PASS | Automated | release + all-CI receipts | AUTOMATED PASS |
| P11B-D038 | Release build PASS | Automated | release/runtime/package receipts | AUTOMATED PASS |
| P11B-D039 | cargo deny PASS | Automated | release receipt | AUTOMATED PASS |
| P11B-D040 | full existing smoke 重新运行 | Automated | all-CI 16/16 PASS | AUTOMATED PASS |
| P11B-D041 | Phase 11 readiness 重新评估 | Automated | `phase-11-rc-readiness.md` | AUTOMATED PASS |
| P11B-D042 | 旧 artifact 标记 superseded | Automated | candidate identity ledger | AUTOMATED PASS |
| P11B-D043 | 新 artifact 重新生成 | Automated | exact package/hash/SBOM receipts | AUTOMATED PASS |
| P11B-D044 | 未 push | Automated | local Git audit | AUTOMATED PASS |
| P11B-D045 | 未 tag | Automated | local tag audit | AUTOMATED PASS |
| P11B-D046 | 未 release | Automated | release action not authorized/executed | AUTOMATED PASS |

## Manual Acceptance

| ID | Requirement | Mode | Required checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P11B-M01 | 真实 Source 点击按钮后 inline/display 文本正确 | Manual | Current-candidate interactive receipt unavailable | NOT TESTED |
| P11B-M02 | 一次 Ctrl+Z 恢复整批转换 | Manual | Current-candidate interactive Undo receipt unavailable | NOT TESTED |
| P11B-M03 | 真实 inline code 与 literal safety | Manual | Current-candidate visual/source receipt unavailable | NOT TESTED |
| P11B-M04 | Right Dock 下 Pin ON/OFF 失焦均约 700ms collapse | Manual | Current-candidate dock/pin receipt unavailable | NOT TESTED |
| P11B-M05 | Pin ON/OFF sensor 100ms reveal 与 leave 500ms collapse | Manual | Current-candidate hover timing receipt unavailable | NOT TESTED |

Shared entry compatibility (parameter scope, routing, failure and caller-state restoration)
is verified by [P00-A11](phase-00.md); this does not change this phase's manual status.

<a id="math-equals-lines"></a>
## 2026-10-08 数学按钮等号行连接

- Plan / feature：plan 07 `semantic-math-delimiter-conversion` → 数学公式 `$` 按钮；延续
  P11B-A01..A05 的范围、代码安全和单次事务，不改写上面的历史 evidence baseline。
- Preconditions：原有两种 LaTeX delimiter、完整 `$$` 块、LF/CRLF、Unicode/反向选区，以及
  代码、单美元公式、未闭合块、空行、边缘等号、`\=`、注释、显式换行和结构环境反例。
- Action：点击现有 `$` action；先执行分隔符转换，再执行独立纯文本等号行连接；重复操作，
  然后 Undo/Redo。
- Expected：只把符合条件的块内 `a\n=\nb` / `a\n=\nb\n=\nc` 变成 `a=b` / `a=b=c`；
  其余文本逐字保留。仅处理完整落在选区内的块，选区按两步映射，整个 action 只增加一次
  generation 和一个撤销记录；第二次操作为 no-op，stale generation 不修改文档。
- Failure signals：误改保护区、跨块/空行连接、自动预览改变源文本、光标落在非法 byte、
  多次撤销才能恢复或原分隔符功能回归。
- Automated entry：`tools/smoke/phase-11-b.ps1` 已有 Rust `phase11b_` selector 包含新增
  `math_text` 与 `flow/editor` 回归；无需改动薄 wrapper 或另建验证框架。
- Manual：真实按钮/IME/DPI 仍为 `NOT TESTED`；本次定向检查不提升人工或发布资格化状态。

<a id="math-equals-first"></a>
## 2026-11-12 数学按钮先连接等号行

- Plan / feature：plan 07 `semantic-math-delimiter-conversion`（USER 2026-11-12 批准，见
  [报告](../report/2026-11-12-math-button-equals-first.md)）→ 数学公式 `$` 按钮。上一节的
  步骤顺序与“仅 `$$` 块”范围由本节取代，其余预期不变。
- Preconditions：USER 的 `\[` / `\rho'(g^{op})(y)` / `=` / `\rho(g^{op})(y).` / `\]` 原文；多行
  `\(...\)` 与 `$...$`；列表内公式；公式外的 `标题\n=\n正文`；被空行切断的 `\[`；引用块、
  链接、代码、表格内的公式；紧贴另一个 `$` 的 `\$$a\n=\nb$$`；从块的第二行起、各行缩进不同的
  多行公式（Comrak 0.56 之前会报错结束列）；同一行的两个多行公式；段首有链接引用定义、位置
  整体错位的段落；引用块的 lazy continuation；未闭合 `$$` 之后的 `\[...\]`；反向选区与部分
  选区。
- Action：点击 `$` action；先执行等号行连接，再执行分隔符转换；重复操作，然后 Undo/Redo。
- Expected：四种公式内符合条件的等号行被连接，随后 `\[...\]` / `\(...\)` 转为 `$$...$$` /
  `$...$`；公式外文本与反例逐字保留，表格内不连接等号行（分隔符转换照旧）。仅处理完整
  落在选区内的公式，选区按两步映射；整个 action 只增加一次 generation 和一个撤销记录；
  第二次操作为 no-op；结果仍会被第一步改变时整个 action 为 no-op。
- Failure signals：公式外的 `=` 被连接；链接标题、代码、引用块标记、表格内的等号行被改写；
  Setext 仍使公式漏转；第二次点击仍改动文本；panic；光标落在非法 byte；多次撤销才能恢复。
- Automated entry：`phase11b_` selector 已包含新增回归：`math_text` 的
  `phase11b_equals_lines_join_inside_latex_and_single_dollar_formulas`、
  `phase11b_equals_lines_leave_text_outside_formulas_and_unsafe_formulas`、
  `phase11b_equals_lines_scope_requires_a_complete_latex_formula`、
  `phase11b_equals_lines_locate_misreported_multiline_formula_ends`、
  `phase11b_equals_lines_reject_positions_that_disagree_with_the_formula`、
  `phase11b_equals_lines_leave_tables_whole`，`semantic_conversion` 的
  `phase11b_conversion_skips_blocks_shifted_by_reference_definitions`，以及 `flow/editor` 的
  `phase11b_math_button_joins_equals_before_recognizing_latex_formulas`（含两次点击）、
  `phase11b_math_button_maps_scope_through_equals_then_delimiters`、
  `phase11b_math_button_refuses_a_result_it_would_change_again`、
  `phase11b_math_button_leaves_paragraphs_shifted_by_reference_definitions`。Release-only 的
  `math_action` 的 `phase11b_performance_math_action_one_mib`（1 MiB、1000 个需两步的公式，
  每个单元另含公式外的裸 `=` 行与一个段首有引用定义的段落；计时为 Flow 调用的同一函数：
  一次处理加提交前的再次处理，不含提交；p95 `< 50 ms`）与
  `phase11b_performance_math_action_stays_linear_in_shifted_paragraphs`（23,717 个段首有
  引用定义的段落；整个按钮须小于同一文档单次 Comrak 解析的 12 倍）由现有 Phase 11-B
  Release 基线任务运行；实测值连同所用提交与原始
  输出记录在[报告](../report/2026-11-12-math-button-equals-first.md)末节。
- Manual：真实按钮在 Windows 与 macOS 上仍为 `NOT TESTED`；Windows 需桌面回归。
