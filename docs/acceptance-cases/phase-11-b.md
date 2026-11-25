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
  `\(...\)` 与 `$...$`；列表内公式；公式外的 `标题\n=\n正文`；被空行切断的 `\[` 与 `$$`（`$$` 以 Comrak 的公式节点为准，USER 2026-11-25）；引用块、
  链接、代码、表格内的公式；紧贴另一个 `$` 的 `\$$a\n=\nb$$`；从块的第二行起、各行缩进不同的
  多行公式（Comrak 0.56 之前会报错结束列）；同一行的两个多行公式；段首有链接引用定义的段落
  （Comrak 0.59 时位置整体错位，1.0 起正确）；结束行上的 closer 前有转义的反斜杠（`\\$`、
  `\\)`）；引用块的 lazy continuation；未闭合 `$$` 之后的 `\[...\]`；反向选区与部分
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
  `crates/stickymd-render/src/math_text/tests.rs:327` `phase11b_equals_lines_join_formulas_after_reference_definitions`、
  `crates/stickymd-render/src/math_text/tests.rs:288` `phase11b_equals_lines_locate_closers_after_escaped_backslashes`、
  `crates/stickymd-render/src/math_text/tests.rs:423` `phase11b_equals_lines_name_each_joined_equals_in_the_result`、
  `phase11b_equals_lines_leave_tables_whole`，`semantic_conversion` 的
  `crates/stickymd-render/src/preview/semantic_conversion.rs:946` `phase11b_conversion_after_reference_definitions_leaves_titles`，`math_action` 的 `crates/stickymd-render/src/math_action.rs:422` `phase11b_math_action_drops_joins_the_preview_reads_outside_text`（Setext 标题之后的引用定义、缩进代码、HTML 块与行内代码中的 `=`，`=` 的下一行开始定义、缩进代码或 HTML 块，跨过 ATX 标题的 `$$` 配对，使列表项继续开着而吞下其后缩进代码、下一个列表或 closer 之后同一行文字的连接，使 closer 之后的反引号换一种配对的连接，以及使 `[ ] ` 变成任务的连接，都不连接，第二步仍转换原文；引用定义之后的公式、连续等号、懒续行、独占段落的 `$…$` 与 `\(…\)`（含同批的其他公式），以及公式前有文字的 Setext 形状仍连接）、`crates/stickymd-render/src/math_action.rs:510` `phase11b_math_action_keeps_links_a_shorter_document_cannot_expand`（恰为 120,000 字节的文档里，连接会让第 1,200 个引用链接超出 Comrak 的展开额度，所以不连接）、`crates/stickymd-render/src/math_action.rs:550` `phase11b_math_action_reads_the_final_text_back`（回读与乙：`\(a\n=\nb\)2xyz$` 保留连接、不转换该公式；`\(a=b\)2xyz$`、末尾是转义空格的 `\( a\ \)`、含 `$` 的 `\(a$b\)`，以及引用里定界符独占一行的 `\(…\)` 读不回同一公式，只把它们保留原样，同一处的其他公式照转；全角空格与空格一样去掉；`$` 后是空格或字母时照常转换；`\( a \)`、`\(\na\n\)`、列表里的多行 `\(…\)`、`$$ a $$` 与引用里的 `\[…\]` 按标准写法改写并读回；单 `$` 文字按规则改成的公式单独、连接之后与转换的公式旁都读回，`$ npm test and check $` 不改）、`crates/stickymd-render/src/preview/semantic_conversion.rs:756` `phase11b_conversion_writes_the_standard_form`（标准写法：`\(…\)` 去掉内侧空格、Tab、全角空格与换行，`\[…\]` 与 `$$…$$` 只去掉内侧空格与 Tab、换行保留，独占一行的闭定界符前的缩进与 `> ` 保留，被转义的空格保留，只有空白的公式与已是标准写法的 `$$…$$` 不改）、`:805` `phase11b_conversion_makes_formulas_of_single_dollar_text_only_by_the_rule`（单 `$` 规则 b：语料里的转义说明行、价格、`$ USD 5 and $`、`$ npm test and check $`、`$ symbol and the $` 不改；`$ a $`、`$a $`、`$ a$`、强调里的 `$ a $`、`$a$` 之后的 `$ b $`、含 `^` 或 `\` 的内容、标题与表格单元格里的照改；代码、链接、以数字开头或闭 `$` 后跟数字、一行三个以上、中间有其他节点、跨行、多个普通词、末尾转义空格都不改；选区须含整对）、`:865` `phase11b_conversion_names_the_rewrites_that_do_not_read_back`（读回同一公式的四个条件逐一成立：起点、终点、去掉首尾空白后的原文、显示方式）、`:908` `phase11b_conversion_maps_dropped_whitespace_to_the_kept_body`（被去掉的空白里的位置映射到保留部分的边界）、`crates/stickymd-render/src/math_action.rs:618` `phase11b_math_action_reads_links_back_after_both_steps`（120,003 字节时连接加转换会让第 1,200 个引用变成文字，于是放弃连接、只转换 `\(z\)`；120,001 字节时单独转换也会如此，于是不改）、`crates/stickymd-render/src/preview/semantic_conversion.rs:729` `phase11b_conversion_forward_position_map_matches_map_position`（回读用的前向位置映射与逐次映射逐位置一致）、`crates/stickymd-render/src/math_action.rs:397` `phase11b_math_action_refuses_an_invalid_scope_before_parsing`（无效选区在解析前拒绝）、`crates/stickymd-render/src/math_action.rs:379` `phase11b_math_action_reports_an_original_over_the_node_limit`（原文超过 Preview 节点上限时报错，不改动）、`crates/stickymd-render/src/math_action.rs:1088` `phase11b_math_action_joins_touch_nothing_but_formula_text`（7 种公式形状，含独占段落的行内公式 × Setext 标题后可开始的块 × 16 种容器写法 × 定义前缀 × 4 种后缀的组合，每个输入一次；凡被采用的连接：连接后与第二步转换后，Preview 读出的引用、列表的编号与松紧、任务状态、ATX 标题、代码与 HTML 块、表格的对齐与表头行、分隔线，以及公式之外的文字源码（相邻文字节点合并）、链接的目标与标题、图片的目标标题与替代文字、行内代码、HTML、公式和三种强调都不变，按文档顺序与引用、列表项的嵌套比较；公式之外的文字与第二步单独处理原文的结果逐字节相同；不带定义前缀时，只选中公式的选区采用同样的连接，且不改公式之外的字节），`crates/stickymd-render/src/math_action/surroundings.rs:496` `surroundings_require_each_joined_formula_to_be_one_math_node`、`:518` `surroundings_compare_a_display_formula_that_meets_a_joined_one`、`:540` `surroundings_map_positions_in_document_order` 与 `:566` `span_cursor_places_ranges_out_of_order_as_in_order`（结果核对的直接测试：被连接的公式须是一个公式节点；与之相交的独立公式块整体比较；遍历按文档顺序映射位置；公式游标的结果与访问顺序无关），`crates/stickymd-render/src/math_text/tests.rs:305` `phase11b_equals_lines_locate_each_closer_kind`（每种分隔符的多行 closer，含 `\$$` 与 `\\$$`），以及 `flow/editor` 的
  `phase11b_math_button_joins_equals_before_recognizing_latex_formulas`（含两次点击）、
  `phase11b_math_button_maps_scope_through_equals_then_delimiters`、
  `phase11b_math_button_refuses_a_result_it_would_change_again`、
  `crates/stickymd-app/src/flow/editor.rs:1079` `phase11b_math_button_leaves_reference_definition_titles`。Release-only 的
  `math_action` 的 `phase11b_performance_math_action_one_mib`（1 MiB、1000 个需两步的公式，
  每个单元另含公式外的裸 `=` 行与一个段首有引用定义的段落；计时为 Flow 调用的同一函数：
  一次处理加提交前的再次处理，不含提交；不计时运行 3 轮后计时 100 轮，每轮先解析同一文档一次：
  按钮中位数不超过解析中位数的 10 倍，nearest-rank p95 `< 100 ms`，即最多 5 个样本可超出）与
  `crates/stickymd-render/src/math_action.rs:889` `phase11b_performance_math_action_stays_linear_after_reference_definitions`（附加诊断，
  不属于 plan 07 的两项检查：23,717 个段首有引用定义的段落，按钮小于同一文档单次 Comrak 解析的 12 倍）由现有 Phase 11-B
  Release 基线任务运行；实测值连同所用提交与原始
  输出记录在[报告](../report/2026-11-12-math-button-equals-first.md)末节；Comrak 1.2 上的复测见
  [升级评估](../report/2026-11-24-comrak-1.2-upgrade.md)。
- Manual：真实按钮在 Windows 与 macOS 上仍为 `NOT TESTED`；Windows 需桌面回归。

<a id="math-known-limits"></a>
## 2026-11-19 数学按钮已知限制

- Plan / feature：plan 07 `semantic-math-delimiter-conversion` 的“已知限制”（USER 2026-11-19
  接受；含孤立 `\r` 即整篇放弃，USER 2026-11-24 批准）→ 数学公式 `$` 按钮。
- Preconditions：含后面不接 `\n` 的 `\r` 的文档：第一个 `\r` 之前的普通 `\(...\)`；混合换行下
  标题中的公式加下一行行内代码中的 `\(y\)`；以孤立 `\r` 分行、含等号行的 `\[...\]` 与
  `$$...$$`；普通 `\(...\)`；普通 `\[...\]`；其余全为 `\n`、只在文末有一个孤立 `\r` 的文档；
  选区只含 `\(x\)`、孤立 `\r` 在选区外（`\r\r\n`）。对照：同样内容改用 `\n` 或 `\r\n`。
- Action：对整篇或所给选区执行 `$` action 的同一纯函数 `run_math_action`。
- Expected：含孤立 `\r` 的输入均为完整 no-op，字节不变；对照输入照常转换，行内代码不变。超过
  大小上限的文本即使含孤立 `\r` 也照旧报 `SourceTooLarge`（由同文件的超限测试锁定）。
- Failure signals：任何字节被改写；对照输入不转换；panic。
- Automated entry：`phase11b_` selector 包含 `math_action` 的
  `phase11b_math_action_leaves_documents_with_an_isolated_cr_unchanged`；文本参数内的等号行
  （`$$\text{a`、`=`、`b}$$` 三行连接为 `$$\text{a=b}$$`）由
  `phase11b_math_action_joins_an_equals_line_inside_a_text_argument` 锁定。
- Manual：不需要。
