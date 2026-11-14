# 提案：`$` 按钮先连接等号行，并扩到全部公式（2026-11-12）

## 结论

建议改动数学 `$` 按钮的两步顺序：

1. 第一步改为等号行连接。范围从只处理 `$$...$$` 扩到四种公式：`$$...$$`、`\[...\]`、`\(...\)`、`$...$`。
2. 第二步才是原来的 `\(`、`\[` 分隔符转换。

公式外的文本不动。等号行连接仍是独立的纯文本模块，Comrak 支持块级 display math 后可以整块移除。

这改的是 plan 07 [`semantic-math-delimiter-conversion`](../plan/07_editor_and_ime.md) 的契约，需要 USER 批准。Windows 与 macOS 共用这段代码，所以 Windows 上按钮的行为也会变。

## 背景

USER 2026-11-12 在 macOS 上点 `$` 按钮，下面这段文本没有任何变化：

```text
\[
\rho'(g^{op})(y)
=
\rho(g^{op})(y).
\]
```

原因（在当前代码上复现过，用的是共享流程 `crates/stickymd-app/src/flow/editor.rs` 的 `convert_math_delimiters`）：

- **第一步，分隔符转换。** 它按预览的 Markdown 规则解析。单独一行的 `=` 让上一行成了 Setext 标题，`\[` 因此不再算公式，转换跳过。
- **第二步，等号连接。** 它只处理 `$$` 块，所以也不动这段文本。

把等号行先手工连上，再点按钮，就能正确得到 `$$\n\rho'(g^{op})(y)=\rho(g^{op})(y).\n$$`（同样在当前代码上验证过）。

USER 的要求（原话）：

> “连接等号行”是第一步，所有的= 都这么处理，第二步才是识别公式\[ 与 \(……因为我们的目标是数学公式第一优先级

在问答中，USER 补充两点：先看上游 Comrak 的动作；若决定打补丁，就写一个独立的小模块，便于以后移除。之后 USER 选择“写补丁”，范围按建议只限公式内部。

## 上游 Comrak 现状（GitHub API，2026-11-12）

- **[kivikakk/comrak#880](https://github.com/kivikakk/comrak/issues/880)** “Optional block-level parsing for multiline display math”：
  - 由 USER 于 2026-10-22 提出，例子正是 `$$ 1 / = / 2 $$`。
  - 维护者回复愿意接受“display math 按块解析”的选项，标为 `good first issue`。
  - 目前没有关联 PR；最后一次活动是 2026-10-23。
- **[szdytom/markview#40](https://github.com/szdytom/markview/issues/40)** 引用了 #880，说明这是同一个问题。
- **上游版本：** 最新是 `v1.0.0-rc.1`（2026-11-12）。StickyMD 锁定在 `=0.54.0`。
  - 最近与数学有关的提交，都是 CommonMark 输出往返（roundtrip）的修复，不涉及块级解析。
- **结论：** 上游接受这个方向，但目前没有人在实现。
- **升级的代价：** 从 0.54 升到 1.0，要处理 `\r\n` 归一化和若干块的 sourcepos 变化。按钮依赖源码位置，所以升级需要另开任务。

## 方案：只在公式内部连接

### 为什么不连接全文的 `=`

上游的块级解析只会改变公式内部，公式外的文本不受影响。补丁与它对齐，以后移除补丁时，公式外文本的行为不变。

若连接全文，会改到上游永远不会改的地方，例如普通文本里用单个 `=` 作下划线的标题（`标题\n=\n正文`）。那样以后移除补丁时，行为会再变一次。

### 第一步：等号行连接

公式范围有两类：

1. **`$$...$$` 对。** 沿用现有的文本扫描：完整、未转义，可以含空行；行为不变。
2. **另外三种公式。** 在一次关闭 Setext 的临时 Comrak 扫描中，Comrak 识别为公式、且原始分隔符为 `\[...\]`、`\(...\)` 或 `$...$` 的节点。
   - 位于链接或图片文本内的不处理。
   - 被空行切断的，Comrak 本来就不认作公式，因此不处理。

范围内的连接规则不变：

- 某一行 trim 后恰为裸 `=`，且上下紧邻行都非空、也不是另一个孤立的 `=` 时，删除等号两侧连接处的换行和空白。
- 公式首尾单独的 `=`、连续的 `=` 行、`\=` 和其它运算符，都保持原文。
- 公式内含 `%` 注释、显式换行（`\\`、`\cr` 等）、反斜杠加空白，或 `\begin`/`\end` 环境时，整个公式跳过。

### 第二步：分隔符转换

规则不变，只是解析对象改为第一步的结果，即连接后的文本。如果第一步没有改动，解析对象就是当前 generation 的原文。

### 选区与事务

- 两步都只处理完整落在选区内的公式。第一步按原文选区判断，第二步按映射后的选区判断。
- 最终光标或选区映射到结果的 UTF-8 边界。
- 整个按钮仍只提交一次编辑：generation 最多加 1，只占一个撤销步骤。两步都没有改动时完全 no-op。

### 性能

只有出现裸 `=` 行和公式标记时，第一步才做一次 Comrak 扫描；第二步仍是一次解析。最坏情况与现在相同，都是两次解析。1 MiB / 1000 公式的 p95 `< 50 ms` 检查照常运行。

## 验证数据（当前代码，关闭 Setext 的 Comrak 0.54 探针）

| 输入 | Comrak 识别结果 |
| --- | --- |
| USER 的 `\[ … = … \]` | 一个 display 公式节点，覆盖第 1–5 行 |
| `文中 \(a\n=\nb\) 后` | 一个行内公式节点 |
| `$a\n=\nb$` | 一个行内公式节点 |
| `- \[a\n  =\n  b\]` | 列表项内的 display 公式 |
| `\[\na\n\n=\nb\n\]`（中间有空行） | 没有公式节点 |
| `\\[a\n=\nb\\]`（转义） | 没有公式节点 |
| `costs $5\n=\nfive$` | 一个行内公式节点（见下文“风险”） |

另外两点：

- Comrak 0.54 对 `\[`、`\(` 节点也报告 `dollar_math = true`，所以必须按源码开头的分隔符字节区分，不能依赖该字段。
- 引用块里的公式，每行带 `> ` 前缀，中间行 trim 后不是裸 `=`，因此不会连接。这与现有 `$$` 的行为一致。

## 影响

- **Windows：** 按钮在 Windows 上也会连接 `\[`、`\(`、`$` 公式里的等号行，需要在 Windows 机器上做一次桌面回归。另外两个变化：
  - `stickymd-render` 和 `stickymd-app` 的模块指纹会变。不过 `11` 在 `5e57b5a` 已经改过，Windows 模块收据本来就要重跑。
  - 现有测试中，有三条“保持原文”的断言会反转，改为期望连接：`math_text/tests.rs` 中的 `"\\(a\n=\nb\\)"`、`"\\[a\n=\nb\\]"`、`"$a\n=\nb$"`。其余断言保留。
- **macOS：** app 可执行文件的 SHA-256 会变，Phase 16 的行要在新身份下重新取得 PASS 或收据：
  - 自动化行在 VM 中重跑；
  - W12 重做脚本注销；
  - 尚未取得的人工收据，直接在新版本上做。
- **文档投影：**
  - `docs/features/00_v1_product_behavior.md` 第 75–80 行；
  - `docs/acceptance-cases/phase-11-b.md` 第 96–112 行：追加新日期一节，旧节保留为历史；
  - `docs/coverage-matrix.md` 第 17–21 行。
  - `docs/release-notes/0.1.3.md` 是历史，不改。

## 风险

- **货币写法。** 例如 `costs $5\n=\nfive$`：关闭 Setext 后，Comrak 把它当作行内公式，会连接成 `costs $5=five$`。这是“以 Comrak 判定的公式为准”的直接后果，罕见，不另加启发式。
- **只修不预览。** 补丁只改用户显式点按钮后的源码，不改预览。在上游支持之前，没点按钮的原文仍会被预览成标题。
- **上游的形态可能不同。** 上游实现后，按钮的第一步可能变得不必要，届时另行评估是否移除。

## plan 07 改动（待批准）

把 [`semantic-math-delimiter-conversion`](../plan/07_editor_and_ime.md) 中的第 2、4、5、6、7 条改为下文。其余条款不变。

- action 分两步，顺序固定：先等号行连接，后分隔符转换。分隔符转换必须从等号连接后的文本经现有 Comrak semantic pipeline 识别真正的 math node；未连接时，即从当前 generation 的 `DocumentSnapshot` 识别。只转换原始 delimiter 为 `\(...\)` 或 `\[...\]` 的节点；不得用 regex、全局 replace、自有 math parser、stale Preview AST 或 code/literal 猜测替代 Comrak。
- 第一步显式调用独立的纯文本等号行连接能力：
  - 它不进入 Markdown/RaTeX 或自动 Preview 管线，不与分隔符转换实现互相依赖，可独立移除或用于其它显式文本动作。
  - 它是 Comrak 按块解析 display math（kivikakk/comrak#880）之前的补丁；上游支持并升级后，重新评估是否保留。
  - 只处理两类公式范围：
    - 完整、未转义的 `$$...$$` 对；
    - 临时扫描中 Comrak 识别为公式、原始 delimiter 为 `\[...\]`、`\(...\)` 或 `$...$` 的节点（链接或图片文本内的除外）。
  - 范围内某一行 trim 后恰为裸 `=`，且上下紧邻行均非空、也不是另一个孤立 `=` 时，删除等号两侧连接处的换行及空白。`a\n=\nb\n=\nc` 可变为 `a=b=c`。
  - 公式外的文本、其它换行、块首尾孤立等号、连续等号行、`\=` 和其它运算符保持原文。
- 等号连接用 Comrak 识别代码、HTML、链接及非 display-dollar 数学的保护范围，并识别上述三种公式节点：
  - 仅这一次文本动作的临时扫描关闭 Setext 识别，以免独占的 `=` 抢先切断代码或数学范围。现有 parser 选项与 Preview 语义不变。
  - `$$` 对的规则：定位完整 `$$` 对后，不跨块或空行连接；未闭合块不改写。
  - 其它三种公式只在 Comrak 节点范围内连接。
  - 含未转义 `%` 注释、显式换行（`\\`、`\cr`、`\crcr`、`\newline`、`\linebreak`、`\displaybreak`）、反斜杠空白，或 `\begin`/`\end` 环境命令的公式整体跳过。
- 选区规则：
  - Source/Split 存在非空 Source selection 时，两步都只处理 source range 完全包含于 normalized selection 的公式，部分相交与 selection 外的不动。第一步按原 selection 判定，第二步按映射后的 selection 判定。
  - Source selection 为空或纯 Preview 模式时，处理整篇当前 canonical document。
  - 两步之间必须映射 selection/scope；最终 caret/selection 映射到结果的 UTF-8 byte boundary。
- 解析与提交：
  - 等号步骤只在存在裸 `=` 行与公式标记时做一次独立扫描；分隔符步骤一次 parse。
  - 每步收集互不重叠的修改，顺序构造结果，不逐公式修改 canonical document。
  - 最终只经单一 canonical mutation gateway 提交一次用户事务：generation 最多递增一次，两步的任意数量修改共用一个 Undo/Redo step，并正常触发 dirty/autosave/Preview。
  - 零匹配必须是完整 no-op（text、generation、undo、dirty 均不变）。

## 待 USER 决定

1. 是否批准上面的 plan 07 改动。
2. 提交位置：USER 未指定，默认放在 `feat/macos-host`，作为单独的提交。

## Resolution：USER 批准并实现（2026-11-12）

### 决定

- USER 批准上文的 plan 07 改动（“批准，开始实现”）。提交位置未指定，按默认在 `feat/macos-host` 上单独提交。
- 审查中加入了批准文本以外的规则。USER 在 [RISK 报告](RISK-comrak-054-inline-positions.md) 的提问中选择“方案 A”，并“接受”这些新增规则。全部写入 plan 07：
  1. 紧贴另一个 `$` 的 `$...$` 不连接（保留现有断言 `\$$a\n=\nb$$`）。
  2. 第一步的扫描使用 Preview 的 Markdown 方言；表格内不连接等号行，整张表格按行保护。
  3. Comrak 位置须与节点一致：opener 在所报起始行上且与节点类型相符；范围内除 ASCII 空白外恰为节点内容；多行公式的结束位置取自所报结束行上的 closer。
  4. 段首有链接引用定义、位置整体上移的段落，两步都不处理（RISK 报告方案 A）。
  5. 提交前对结果再做一次完整处理；仍会改变时整个 action 为 no-op。

### 更正上文

- 上文“第二步：分隔符转换”一节说规则不变：实际上第二步新增了第 4 条，跳过位置不可信的段落。这也修好了 HEAD 已有的错误转换。
- “最坏情况与现在相同，都是两次解析”不再成立：加上提交前的再次处理，最坏情况下两步各解析两次。基准已改为计入这部分。
- 两步的组合移到 `crates/stickymd-render/src/math_action.rs` 的 `run_math_action`。Flow（`crates/stickymd-app/src/flow/editor.rs` 的 `convert_math_delimiters`）调用它两次（处理与再次处理），基准测试调用的是同一个函数。

### 审查（codex-reviewer，六轮，均为 effort high、只读）

| 轮次 | 发现 | 处理 |
| --- | --- | --- |
| 1 | 错位的结束列让连接越过公式；表格边界被当作公式正文；`$$` 的错误坐标中断整个扫描；一次点击没做完，第二次还会改；性能数字缺少可核验输出 | 改为从结束行找 closer；使用 Preview 方言并按行保护表格；不再读取 `$$` 节点的结束位置；性能数字改为提交后绑定 SHA 记录 |
| 2 | 引用定义让位置整体错位，出现改写标题与 panic；lazy 引用块的 `>` 被并入公式；未闭合的 `$$` 与新写出的 `$$` 重新配对，第二次点击还会改；文档把“表格不连接”写成了“表格原文不动” | 加入位置与内容的核对和受检切片；加入提交前检查；修正文档 |
| 3 | 内容重复时能骗过核对；新顺序让第二步的旧缺陷在更多输入上触发；提交前检查只覆盖第一步；基准没有计入提交前检查 | 写 RISK 报告，USER 选方案 A，两步都识别错位段落；提交前改为完整的再次处理；基准改为调用 `run_math_action` 两次，并加入公式外的裸 `=` 行 |
| 4 | 第二步按字节反推行差，会被越过换行的列号绕过；没有字节位置的节点导致误判；第一步对错位段落的跳过是平方级 | 两步改用同一个行号判据（`preview/source_map.rs` 的 `inlines_shifted`），第二步只在含 `]:` 时另做一次解析；第一步只跟踪当前所在的错位段落；新增大量错位段落的线性基准 |
| 5（结论：PASS，无 BLOCKER） | 段末的跨行链接被误判为上移；深层引用仍是平方级；第二步多做一次解析，超出 plan 的解析次数 | 两步都只在含 `]:` 时检查，剩余误报写入 plan 与 RISK；第一步改为在进入、退出节点时维护状态；上移块改由 `PreviewParser` 在同一次解析中求出（`OwnedDocumentTree::shifted_blocks`） |
| 6（结论：PASS，无 BLOCKER） | 第 5 轮的三项：深层引用与解析次数已修好；跨行链接在含 `]:` 时仍被跳过，这是已写入契约的保守限制。另指出基准注释里的段落数与解析次数过时；原始运行输出尚未提供 | 更正注释（23,717 个段落，四次解析）；原始输出见下一节 |

各轮的完整输出在本机 scratchpad（会话临时目录，不入库）。每条发现都先在本机复现，再动手修改；对应输入均已加为回归测试。

### 验证（本机 macOS arm64，未提交的工作树）

- 测试：`phase11b_` 定向测试全部通过（`stickymd-app` 10 个、`stickymd-render` 20 个，另有 2 个仅限 Release 的基准）。
- 全量检查：
  - `cargo test --workspace --exclude stickymd-win --locked` 通过（`stickymd-win` 只能在 Windows 上编译）；
  - `cargo clippy --workspace --exclude stickymd-win --all-targets --locked -- -D warnings` 干净；
  - `cargo fmt --all --check` 干净；
  - `stickymd-smoke phase-entry 00` PASS。
  - 提交前会在最终工作树上重跑。
- 性能：Release 基准的结果在提交后，绑定提交 SHA 追加在下一节。提交前在工作树上的一次运行（Apple M6）：plan 规定的工作量 p95 21.4 ms；23,717 个错位段落时，整个按钮的中位数 41.0 ms，同一文档单次 Comrak 解析 9.0 ms。
- 小段落非常多的 1 MiB 文档，单次 Comrak 解析就要几十毫秒；在约 9 万个小段落时，HEAD 原有的转换单独约 36–57 ms，本按钮约 170 ms（本机）。这类文档超出 50 ms 预算，原因在解析本身，不在本次改动。

### 未验证

- Windows 桌面回归（按钮行为在 Windows 上同样改变）。
- Windows 上的 Phase 11-B Release 基线：本机 p95 21.4 ms，若 Windows 机器慢一倍以上，可能接近 50 ms。
- macOS 新版本的 VM 自动化行重跑，以及真实按钮的人工确认。
- Comrak 0.54 其余位置缺陷的穷尽排查。例如带缩进差异的多行行内代码，它的保护范围是旧行为，本次未改。

## Release 基线（提交 `2febab10f43a0324b365efd9210cf57ef3e10b55`，2026-11-12）

在该提交的干净工作树上运行（Apple M6，macOS 27.2.1，rustc 1.97.1）：

```text
cargo test --locked --release -p stickymd-render --lib phase11b_performance_ -- --ignored --nocapture
phase11b semantic_conversion_1mib_1000_math median=2.200291ms p95=3.767792ms max=3.799ms
phase11b math_action_1mib_1000_math median=21.950417ms p95=26.724916ms max=33.362625ms
phase11b math_action_1mib_shifted_paragraphs parse_median=13.21225ms median=68.7905ms p95=96.401083ms max=96.401083ms
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 216 filtered out; finished in 0.79s
```

退出码 0。各项结果：

- **plan 规定的工作量（`math_action_1mib_1000_math`）：** p95 26.7 ms，低于 50 ms。
- **线性检查（`math_action_1mib_shifted_paragraphs`）：** 中位数 68.8 ms，约为单次解析 13.2 ms 的 5.2 倍，低于 12 倍。

几次运行之间的波动来自本机负载：提交前的一次运行分别是 21.4 ms、41.0 ms 和 9.0 ms。同一提交上的工作区测试、clippy、fmt 与 Phase 00 都已通过（见 Resolution 中的命令）。

## 复审（2026-11-12，`baecb0a..995154e`）

USER 要求 codex 对已推送的六个提交整体复审。结论是不通过：2 项 BLOCKER、3 项 SHOULD_FIX、1 项待验证。每项都已核实。与本按钮有关的两项如下，处理都在复审之后的提交里。

- **BLOCKER：只含引用定义的段落里，标题会被连接。**
  - 现象：`[x]: u "$$a\n=\nb$$"` 后接 `[x]` 时，按钮把标题改成 `$$a=b$$`（本机复现）。
  - 原因：Comrak 会把只含引用定义的段落整段移除，这些行不属于任何节点；`$$` 的文本扫描又没有为它们设保护范围。这个缺陷在 `baecb0a` 已存在。
  - 修复：文中含 `]:` 时，第一步把不属于任何叶块（段落、标题、代码块、HTML 块、分隔线、表格）的非空行按整行保护（`math_text.rs` 的 `protect_detached_lines`）。列表项里只含引用定义的段落也被覆盖。
  - 已加回归测试，覆盖 `math_text` 与 `flow/editor`。
- **SHOULD_FIX：超大文档先被第一步完整解析。**
  - 现象：大于 5 MiB 的文档，第一步会先完整解析一遍，然后才报 `SourceTooLarge`。本机 6.3 MB 时多花了 148 ms；HEAD 的旧顺序在解析前就拒绝。
  - 修复：`run_math_action` 在第一步之前先做同一大小检查，并加了回归测试。

其余几项：
- plan 06 的方言核验见 [升级评估](2026-11-12-comrak-upgrade-evaluation.md) 的补充；
- 人工行状态与 W12 账本见 [VM 验收](2026-11-10-macos-vm-acceptance.md) 的更正；
- Windows 实机运行仍待验证。

### 复审第二轮（2026-11-12）

结论：PASS，无 BLOCKER；3 项 SHOULD_FIX、1 项待验证。逐项核实后处理如下：

- **保护范围过宽。** 文中含 `]:` 时，只有 `>`、`-`、`1.` 的空引用块或空列表项行也被保护，连带跳过跨越它们的合法 `$$` 连接。例如 `[x]: u` 之后的 `$$a\n=\nb\n\n>\n\nc$$`，本机复现为不连接，LF 与 CRLF 都一样。
  - 修复：只保护带文字的未覆盖行（`markup_only`），只含容器标记的行不保护。
  - 理由：任何连接都需要一个裸 `=` 行，而它不会是空行或纯标记，所以引用定义标题仍受保护。
  - 已为 `>`、`-`、`1.`、`> -` 各在 LF 与 CRLF 下加正向回归。
- **大小检查的测试区分不了新旧顺序。** 测试输入在连接后仍然超限，所以旧顺序也能通过。
  - 修复：改为超限 1 字节、连接后回到上限以内，只有第一步之前的检查才会拒绝它。
  - 本机核对：去掉这项检查，测试就失败。
- **验收表需要降级。** 修复改了产品代码，W07、W08、W09、W11、K02、E01 与 W12 的证据仍对应 `a9f4354b…`。
  - 已按 plan 11 改为 `NOT TESTED`，原证据保留为“此前”，等新构建在 VM 中重跑、W12 由 USER 重新确认。
- **Windows x64 实机运行：** 仍待验证。

### 复审第三轮（2026-11-12，`9088c57`）

结论：PASS，无 BLOCKER。1 项 SHOULD_FIX，1 项 Windows 待验证（既存）。

- **问题。** 嵌套的空容器标记行，例如 `- >`、`- -`、`1. >`、`> - >`，仍被当作保护范围，连带挡住合法的 `$$` 连接。原因是 `markup_only` 只认“若干 `>`，然后至多一个列表标记”这一种顺序。
- **修复。** `markup_only` 改为接受任意顺序、任意层数的 `>` 与列表标记；有序列表的数字限 1–9 位；`-`、`*`、`+` 与有序标记都必须后接空白或位于行尾，否则视为正文、继续保护。
- **回归测试。**
  - 正向：上述四种嵌套标记与 `>>`，在 LF 与 CRLF 下各一例。
  - 反向：`-x`、`1234567890.`、`-=`、`=` 出现在只含引用定义的段落的标题里时仍受保护。
