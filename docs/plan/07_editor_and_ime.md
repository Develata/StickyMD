# 07_editor_and_ime.md - 源码编辑器与输入法合同

## Metadata

- `Layer`: Capability
- `Status`: Approved Contract
- `Version`: 0.4.0
- `Last Review`: 2026-11-24
- `Scope`: Source 编辑器职责、IME preedit/commit 语义、字体 run、undo 分组、RichEdit fallback 治理

---

## Purpose

定义源码编辑区的行为契约与输入法正确性要求。中文输入法体验是 v1 一级需求。

## Boundary

- 编辑器后端只产生/消费 TextDelta 与选区状态；文本权威始终是 DocumentState。
- 本合约不覆盖 Preview 的选择/复制（见 `06`）。
- Editor、Search 与 preedit 会话属于 Shared Application；原生输入与输入法事件由 Shared Desktop 翻译为规范事件，
  修饰键与快捷键映射由 Platform Host 提供；候选窗定位留给平台接线。Search 组字期间的视图切换守卫、停放/恢复与
  替换边界只有一份实现。

## Owned Objects

`doc::text`（经 delta 修改）、caret/selection/preedit 呈现状态（非权威）。

---

<a id="source-editor"></a>
## Source 编辑器职责

- cosmic-text 布局 + 自绘 caret / selection / preedit。
- viewport 滚动（垂直；水平按需）。
- 垂直滚动条及其视口/交互边界遵守 `09_windows_shell.md#vertical-scrollbars`。
- 常规快捷键：编辑、Ctrl+C/X/V、Ctrl+Insert/Shift+Delete/Shift+Insert、
  Ctrl+Z / Ctrl+Y、Ctrl+S、Ctrl+Shift+S。传统剪贴板快捷键必须映射到与常规快捷键
  完全相同的 typed intents；Shift+Insert 必须保留文本/图片剪贴板的同一优先级路径。
- 剪贴板文本（文件列表剪贴板由平台层补充）。
- 复制成功显示“已复制”成功提示（3 s，`09` #notices；Phase 17，USER 批准 2026-11-14），不修改文档或选区。
  - 正在显示提醒、重要提醒或故障展示时，不显示“已复制”（`04` #notice-state 的入栈规则）；
    复制失败总会显示，同样不修改文档或选区。
  - Preview 复制经同一剪贴板写入回执时遵守同一反馈规则。Source 复制仍原样切片，
    Preview 复制按 `06` #preview-copy-semantics，两者共同遵守 `02` #copy-what-you-see。
- 字符/脚本级字体 run（见下）。
- 向 Flow Coordination 发出 dirty/autosave 事件。
- 不做完整语法高亮；Markdown marker 不额外着色；当前行可有极轻背景提示。
- Source 基准字号 16 DIP、基准行高 1.55；统一乘以全局 Content Zoom（50–300%）。
  不提供独立 Source 字号、行高或字体配置。
- caret、selection、IME preedit 必须明显可见。

<a id="semantic-math-delimiter-conversion"></a>
### AI 数学分隔符语义转换

- 顶部工具栏提供一个紧凑的 `Convert AI math delimiters` typed action；Interaction Shell
  使用紧凑、清晰的 `$` 标识；Interaction Shell 只能发出 intent，不得直接改写 `DocumentState`。
- action 分两步，顺序固定：先等号行连接，后分隔符转换。分隔符转换必须从等号连接后的文本
  （未连接时即当前 generation 的 `DocumentSnapshot`）经现有 Comrak semantic pipeline
  识别真正的 math node，只转换原始 delimiter 为 `\(...\)` 或 `\[...\]` 的节点；不得用
  regex、全局 replace、自有 math parser、stale Preview AST 或 code/literal 猜测替代 Comrak。
- `\(SOURCE\)` 转为 `$SOURCE$`；`\[SOURCE\]` 转为 `$$SOURCE$$`。按钮的职责是把公式写成 Comrak
  能读成同一公式的标准写法（USER 2026-11-25），所以除了替换 delimiter bytes，还去掉紧贴定界符
  内侧的空白；首尾空白不影响 TeX 数学排版，这仍是不改语义的改写：
  - `\(…\)` 写成 `$…$` 时，去掉开定界符之后、闭定界符之前的空格、Tab 与换行（含 `\r\n`），
    因为 Comrak 不把旁边有空白的单 `$` 当作定界符：`\( a \)`、`\(a \)`、`\( a\)` 都写成 `$a$`。
  - `\[…\]` 写成 `$$…$$`，以及 Comrak 已读成公式的 `$$…$$`，只去掉紧贴定界符内侧的空格与 Tab，
    换行保留（`$$\n…\n$$` 本身就是标准的块写法）：`\[ a \]`、`$$ a $$` 都写成 `$$a$$`。
  - 末尾的空白若紧跟在奇数个反斜杠之后，第一个空白字符是被转义的 TeX 控制空格（`\ `），保留它
    （否则 `\( a\ \)` 会变成 `$a\$`）。去掉空白后内容为空的公式（`\( \)`、`$$  $$`）不改写。
  - 其余 inner source（中间的空白与换行、Unicode、escape、全角空格）byte-for-byte 保持；inline/fenced
    code、普通讨论文本与 malformed delimiter 不变。Comrak 没有读成公式的单 `$` 文字（如 `$ a $`）
    不改写，见下文待批准的草案。此约束属于分隔符转换步骤。
- 第一步显式调用独立的纯文本等号行连接能力；它不进入 Markdown/RaTeX 或自动
  Preview 管线，不与分隔符转换实现互相依赖，可独立移除或用于其它显式文本动作。它起初是
  Comrak 按块解析 display math（kivikakk/comrak#880）之前的补丁；2026-11-24 升级到 Comrak 1.2
  时重新评估后保留：1.2 的可选块级公式（`math_dollars_block`，StickyMD 不开启）只认独占一行的
  `$$`，`\[...\]`、`$$a…b$$` 与多行行内公式中的裸 `=` 行仍会被读成 Setext 标题。只处理一类公式
  范围：临时扫描中 Comrak 识别为公式、原始 delimiter 为 `$$...$$`、`\[...\]`、`\(...\)` 或 `$...$`
  的节点（链接或图片文本内的、表格内的除外；紧贴另一个 `$` 的 `$...$` 视为残缺的 `$$` 对而跳过）。
  `$$` 也以 Comrak 的节点为准，不在原文中自行配对（USER 2026-11-25，见
  [Comrak 1.2 升级评估](../report/2026-11-24-comrak-1.2-upgrade.md) 的第四轮复审一节）：
  Comrak 从不在代码、HTML 或引用定义里识别公式，也不让公式跨过空行或另一个块，所以跨空行的
  `$$` 对不连接。范围内中间行 trim 后恰为裸 `=`，上下紧邻行均非空且不是另一个孤立 `=` 时，删除
  等号两侧连接处的换行及空白。`a\n=\nb\n=\nc` 可变为 `a=b=c`；公式外的文本、其它换行、块首尾
  孤立等号、连续等号行、`\=` 和其它运算符保持原文。
- 临时扫描使用 Preview 的 Markdown 方言，仅此文本动作关闭 Setext 识别，以免独占 `=` 抢先切断
  公式。现有 parser 选项与 Preview 语义不变。Comrak 报告的位置须与节点一致：opener 位于所报
  起始行且与节点类型相符，范围内除 ASCII 空白外恰为节点内容（所以原文里的引用标记 `>` 若落进
  范围就不一致）；多行公式的结束位置取自所报结束行上 Comrak 匹配的 closer，即该行第一个未被转义的
  closer（反斜杠转义其后一个字节，除非它本身已被转义，与 Comrak 1.2 的数学规格一致），不取所报
  列号。位置不一致的公式跳过。
  关闭 Setext 后，Setext 标题之后的引用定义、缩进代码、HTML 块或行内代码会被临时扫描读成段落
  文字，其中的公式也就能被连接。所以第一步有连接时，用 Preview 方言做两次核对（2026-11-24 跨模型
  复审发现，USER 批准为此多解析一次原文，见上述升级评估的复审各节）：
  - 解析原文：每个连接的上一行与 `=` 行属于同一个文字块（Setext 标题，或 `=` 不是下划线时的
    同一段落），下一行属于该块或紧接着的下一个叶块；三处都是段落、标题或独立公式里的文字，
    不在其中的行内代码、行内 HTML、链接或图片里，也不在代码块、Raw HTML 块、表格或任何块都
    不含的行（如引用定义）上。
  - 解析第一步的结果（即第二步要转换的树）：每个被连接的 `=` 同样落在文字里；每个被连接的
    公式经连接的位置映射后，恰是结果中一个公式节点的范围；并且两棵树的其余节点经位置映射逐个
    相同：种类，Preview 显示或第二步读取的字段（列表的有序性、起始编号与松紧，任务状态，标题
    级别，代码与 HTML 的原文，链接与图片的目标、标题、替代文字与类别，公式原文），在引用、列表项
    与表格中的位置，起点，以及引用、列表与列表项以外节点的终点。被连接公式之外的文字按相邻
    文字合成的段比较映射后的范围及其在引用与列表项中的位置，所以文字不会移进或移出引用与列表项。
    不比的只有：完全落在被连接公式之内的节点与文字；与被连接公式相交的段落或标题本身的种类与
    范围，因为撤销 Setext 对公式的切分正是连接的用意，所以与公式开头同一行、公式之前的文字可以
    从标题文字变成段落文字，块内的其余内容照常比较；容器的终点，连接补全的列表项随公式一起结束。
    与被连接公式相交的其他块（独立公式、代码、HTML、表格、分隔线）整体比较。所以公式两侧的强调
    若因连接而配对，连接也放弃。
    这样放弃的连接包括：列表项在公式之后继续开着，空行后的缩进代码变成该项的段落而被第二步改写，
    后面的列表并入而重新编号，或 closer 之后同一行的文字移进该项；closer 之后的反引号换一种配对，
    行内代码变成公式；Comrak 引用链接的展开额度取文档长度，连接使文档变短，公式外的最后一个
    引用链接退成文字；Setext 标题把 `[ ] ` 当作文字，连接后列表项变成任务（2026-11-25 第五至
    七轮复审）。两次遍历都按文档顺序；节点位置按文档顺序出现时（已测的输入都是如此），耗时与
    节点数、连接数及比较的原文字节数成正比，位置倒退时多一次二分查找与一次从头的位置映射。
    只保存原文一侧的比较记录，不多解析。
  - 回读最终文本（USER 2026-11-25 选定“A：本分支再解析”）：第二步改写了分隔符时，再对最终文本
    做一次 Preview 解析，与原文的解析按上一项的规则比较，位置映射经过两步。被转换的公式读回来
    须仍是一个公式节点，原文（不计首尾空白，被转义的末尾空格照计）与显示方式不变，范围是映射后
    的范围；被连接的公式恰是一个公式节点；
    其余节点与公式外的文字都不变。上两项只看第一步的结果，这一项补上第二步的改写带来的变化：
    写出的 `$` 后紧跟数字时 Comrak 不把它当作 closer，公式边界改变（如 `\(a\n=\nb\)2xyz$`；单独
    使用第二步的 `\(a=b\)2xyz$` 也是，`main` 上已有）；`\( a \)` 照原样写成的 `$ a $`
    不是公式（`main` 上如此，现在改写时去掉内侧空白，见上）；第二步使文档变短，可能与连接一起
    用尽 Comrak 按文档长度给出的引用链接展开额度。
    回读不通过而采用了第一步时，放弃第一步，第二步改为转换原文，再回读一次；仍不通过，或本来就
    没有采用第一步，整次不改（完整 no-op）。所以文中只要有一个改写后仍读不回同一公式的
    `\(…\)`，整次点击就不改任何东西。去掉内侧空白
    之后仍读不回的写法：闭定界符后紧跟数字（`\(a\)2` 写成的 `$a$2`）；末尾是被转义的空格
    （`\( a\ \)` 写成的 `$a\ $`）；公式内含 `$`；引用里的多行 `\(…\)` 而定界符独占一行（行首的
    `>` 留在了公式里）。是否改为只跳过这些公式、其余照转，由 USER 决定（2026-11-25 待定）。

  - **草案，待 USER 批准，未启用：单 `$` 内侧空白的规范化。** 目的：`$ a $`、`$a $`、`$ a$` 这类
    Comrak 读成普通文字的写法，改写成 `$a$`。改写的对象不是 Comrak 的公式节点，而是文字里的
    `$`，所以要先定配对规则与防误判办法：
    - 候选 `$`：Preview 解析中段落、标题与表格单元格的文字节点里，前面有偶数个反斜杠（未被转义）、
      前后都不是 `$` 的单个 `$`。行内代码、Raw HTML、链接与图片（含其文字）、公式里的 `$` 都不是
      候选，因为它们不在这些文字节点里。
    - 配对：只在同一行内配对，不跨行、不跨块；一行里恰好两个候选时才配对，三个及以上（无法唯一
      确定）整行不改。
    - 防误判：两个 `$` 之间去掉首尾空白后的内容非空；内容以数字开头（`$ 5 和 $ 10`、`$ 2 $`）
      不改，按货币处理；闭 `$` 之后紧跟数字的不改；配对后两侧必须至少有一侧带内侧空白（否则
      Comrak 本应读成公式，读不成另有原因，不猜）。
    - 改写后照常回读：新公式须恰是一个公式节点，原文等于去掉首尾空白的内容，其余不变；不通过按
      上面的回退处理。
    - 仍会误判的写法见 2026-11-24 升级报告的“单 `$` 草案”一节，由 USER 过目后再决定是否启用。

  任一处不满足，就整个放弃第一步，第二步改为用已有的原文解析转换原文。含未转义 `%`
  注释、显式换行（`\\`、`\cr`、`\crcr`、`\newline`、`\linebreak`、`\displaybreak`）、反斜杠空白
  或 `\begin`/`\end` 环境命令的公式整体跳过。
- 链接引用定义：Comrak 1.0 起正确报告段首引用定义之后的段落及其行内位置，两步照常处理其中的
  公式。0.59 不调整这类段落的起始行，行内位置随之上移，当时两步都跳过这类段落；2026-11-24
  升级到 1.2 后删除了这项跳过（USER 批准）。定义本身（含跨行标题）不是公式节点，两步都不处理。
- Source/Split 存在非空 Source selection 时，两步都只处理 source range 完全包含于
  normalized selection 的公式；部分相交与 selection 外的不动。第一步按原 selection 判定，
  第二步按映射后的 selection 判定。Source selection 为空或纯 Preview 模式时处理整篇当前
  canonical document。两步之间必须映射 selection/scope，最终 caret/selection 也映射到
  结果的 UTF-8 byte boundary。
- 一次处理为：等号步骤只在存在裸 `=` 行与公式标记时做一次独立扫描，分隔符步骤一次
  parse，改写了分隔符时回读一次；两步由 `stickymd-render` 的纯文本组合函数依次执行，Flow 只负责 generation、事务与
  selection。每步收集互不重叠修改，顺序构造结果，不逐公式修改 canonical document；
  最终只经单一 canonical mutation gateway 提交一次用户事务：generation 最多递增一次，
  两步的任意数量修改共用一个 Undo/Redo step，并正常触发 dirty/autosave/Preview。零匹配必须是
  完整 no-op（text/generation/undo/dirty 均不变）。提交前对结果按映射后的 scope 再做一次
  完整处理；若仍会改变文本，整个 action 保守拒绝，同样为完整 no-op，保证再次点击不再改动。
  原先的两个例子（未闭合的 `$$` 与第二步写出的 `$$` 重新配对；第二步在内层 closer 处结束外层
  公式而留下新公式）现在都由连接的核对放弃下一次的连接，所以第一次的结果直接提交；固定种子的
  400,000 个随机输入中只有 2 个仍需这项拒绝（`flow/editor` 的按钮测试）。源码字节超限、含孤立 `\r` 或 scope 无效时在解析前拒绝，不解析。其余每次
  处理：第一步只在有裸 `=` 行与公式标记时扫描一次；没有连接时，第二步解析原文一次；有连接时先
  解析原文核对，通过后再解析第一步的结果，第二步复用最后被采用的那一棵树。原文核对放弃第一步时
  只解析了原文；结果核对放弃时，原文与结果都已解析，回退只是不再解析原文。第二步改写了分隔符
  时再解析最终文本一次作回读；回读放弃第一步而回退时，第二步转换原文后再回读一次。原文超过
  Preview 的节点上限时报该解析错误，不做任何改动，即使连接后的文本不超限。第一次处理改动了
  文本，才有提交前的第二次处理。所以一次处理最多解析 5 次（扫描、原文、第一步的结果、两次回读），
  一次点击最多 10 次；常见的“有连接、有改写、都通过”是第一次处理 4 次，第二次处理 1–2 次。
- 1 MiB / 1000 math-node Release smoke（含提交前的再次处理）有两项 engineering check。先不计时
  运行 3 轮，再计时 100 轮；每轮先计时一次同一文档的单次 Comrak 解析，再计时一次按钮，使主机
  的短暂卡顿同时影响两者：
  - 防退化：按钮中位数不超过解析中位数的 10 倍。比值去掉了机器快慢，快机器上同样能发现浪费；
  - 流畅上界：按钮的 nearest-rank p95（排序后第 95 个样本）`< 100 ms`。最多 5 个样本可以超出，
    共享 CI 主机上 2–3 个样本的短暂卡顿不判失败。

  任一项超过时先审计
  重复 parse、全文 clone 与逐 formula mutation，不引入增量 Markdown parser 或后台 runtime。
- 已知限制（USER 2026-11-19 接受；第一项的整篇检查 USER 2026-11-24 批准）：
  - **含孤立 `\r` 的文档**：plan 05 把孤立 `\r` 保留为普通字符，编辑器与预览的源码映射只按
    `\n` 分行，Comrak 却把后面不接 `\n` 的 `\r` 读作换行。第一个这样的 `\r` 之后两边的位置对不上，
    替换可能落到别的字节上（例如混合换行时改写行内代码）。所以文本中只要有一个后面不接 `\n`
    的 `\r`，action 就在两步之前放弃，整篇为完整 no-op，字节不变，与 scope 无关。超过 Preview
    源码大小上限的文本仍先按大小拒绝并报编辑失败，这项检查在它之后。
  - **文本参数内的等号行**：等号连接不区分 `\text{...}`、`\mbox{...}` 等文本模式参数，其中
    符合条件的等号行同样被连接；删去的空白在文本模式下可见（`a = b` 变为 `a=b`）。USER 判定
    这类写法罕见、语义未变，可接受。

<a id="source-find-replace"></a>
### Source 纯文本查找与替换

- 查找与替换是同一个 `SearchSession`、同一个面板和同一套匹配 projection，不建立两套实现。
  `Ctrl+F` 在面板关闭时打开 Find-only；面板已打开时再次按 `Ctrl+F` 关闭。`Ctrl+H` 在关闭时
  打开并展开 replacement row，在 Find-only 状态下只展开同一 session，已经展开时聚焦 replacement
  输入框。范围永远是当前单一 canonical `note.md`，不做跨文件搜索、不支持正则。
- Find-only 状态在 state reducer 和 command boundary 都必须禁止 Replace Current / Replace All；
  不能仅隐藏按钮后仍让快捷键以空 replacement 修改文档。替换面板提供 query、replacement、
  大小写开关、上一个/下一个、替换与全部替换；窄 Source pane 的紧凑档省略上一个/下一个按钮（见下文
  面板分档），导航仍由键盘完成。
- query、replacement、active match 与 match ranges 属 Editor Session projection。match ranges 必须绑定
  Document generation；任何 canonical mutation 后先重新扫描，再允许导航或替换 stale range。大小写
  敏感开关默认关闭，关闭 session 后不写入配置。
- 查找以 UTF-8 byte range 表达。大小写敏感路径使用标准库 substring search；大小写不敏感路径
  使用确定性的 Unicode lowercase token stream + KMP prefix table，并以 query 长度的边界环形缓冲
  映射回 source byte，禁止产生 lowercase expansion 中间或非 char-boundary range。扫描 O(n+m+matches)、
  辅助空间 O(m)，其中 `m` 为 query 长度；导航在有序结果中 O(1)。
- Replace Current 通过一个普通 typed `EditRequest` 提交；Replace All 先验证不重叠 range，再一次
  顺序复制 unchanged slices 与 replacement，最终只提交一个 canonical mutation/Undo entry，复杂度
  O(n + output bytes)，不得对每个 match 重复 `String::replace_range`。
- Replace Current 只替换 Source 当前选中的 active match。外部重载、工具栏转换或在正文中点击后，
  选区可能不再是 active match；此时第一次 Replace Current 只选中并显示 active match，不修改文档，
  再次执行才替换。替换并重新扫描后，active match 取插入内容末尾（edit 留下的 caret；replacement 与
  原文相同的 no-op 取原 match 末尾）处或之后起始的第一个 match，没有则回绕到第一个，并选中它；
  之后还有 match 时，replacement 含 query 也不会在原处反复替换；回绕后可能再次命中先前插入的内容。
  match projection 有上限（超出时计数带 `+`），导航与替换后的下一个 match 只在保留的 match 中前进与回绕。
- replace 成功后正常触发 dirty/autosave/Preview；零匹配是完整 no-op。IME preedit 期间禁止替换，
  打开控件不得隐式提交、取消或污染 composition。
- Search 属于 Source pane。切到没有 Source pane 的视图（Preview）时，打开的 Search 被停放：保留
  query、replacement、大小写开关、replacement row、当前字段、各字段 cursor 与 active match 序号，
  释放 match projection；停放期间它不绘制、不接收键盘/IME/鼠标、不观察 canonical mutation，Esc
  顺序与自动收起判定都按 Search 未打开处理。切回 Source/Split 时原样恢复并按当前 generation 重新
  扫描（文档未变时 active match 不变），Source selection 与打开、导航时一样跟随 active match；Source
  与 Split 之间切换保留打开的 Search。Search 字段 IME composition 进行中时拒绝切到 Preview 并给出诊断，与打开查找
  时的拒绝一致。Preview 中 `Ctrl+F`/`Ctrl+H` 先切回 Source：有停放的 Search 时原样恢复（`Ctrl+H`
  再展开或聚焦 replacement row），不按选区改写 query；切换未发生时不打开 Search。
- 视图切换 `Ctrl+1/2/3` 与键盘内容缩放是窗口命令，与保存/导出一样不被面板吞掉；Search 字段
  composition 期间它们与其它按键一样留给 composition。

输入、caret 与导航合同：

- query/replacement 输入框各持有独立 UTF-8 byte cursor。Left/Right 只在当前输入框移动 cursor；
  Up/Down 分别导航上一个/下一个 match。Enter=下一个，Shift+Enter=上一个，Esc=关闭，Tab 在字段、
  大小写与动作间循环。全局 Ctrl+S/导出等既有命令不得被面板吞掉。
- IME composition 期间，方向键、Backspace、commit/cancel 优先交给 composition；不得把 preedit 内
  导航误解释为 match navigation。preedit 不写入 query/replacement authority，只在当前 byte cursor
  处临时投影；commit 后一次性插入，cancel 后字段不变。
- 输入框 paint、mouse hit、caret 和 `set_ime_cursor_area` 必须来自同一次 Cosmic Text 单行 layout。
  不得用固定 x、文本尾部或源码 editor caret 代替查找框 caret。面板打开时源码原生 caret 停止闪烁/
  绘制；当前字段绘制自己的 caret，并在内容过宽时只做水平 viewport 偏移与裁剪，保证 caret 可见。
- 面板是 Source pane 右上角的窄条（同 VS Code 查找框）：宽度上限 300 DIP，pane 更窄时占满 pane 宽度；
  Split 中位于 Source pane（分隔线左侧）的右上角，不越过 Source 滚动条 gutter 与分隔线上的同步按钮。
  面板表面（背景、输入框、按钮）整体使用同一个固定不透明度 40%，各部分只以颜色区分、焦点切换不改变透明度；
  文字与图标不透明以保证可读；该透明度相对正文、与窗口透明度无关。
  面板覆盖处的按下归面板（被覆盖的正文可见但不可点），窗口原生 resize 边框与更高层的透明度弹层除外。
  （USER 决定，2026-10-21。）
- 面板分两档：完整档在 query 旁排 `Aa < > x`；完整档留给 query 文字的宽度不足 40 DIP 时（如 220×120
  最小窗口或 330 DIP 宽窗口的 Split）改用紧凑档，收紧边距、间距与文字内距，只保留 `Aa` 与 `x`，
  replacement 行保留替换与全部替换，省略上一个/下一个按钮。导航的合同是 Enter/Shift+Enter、Up/Down
  与 F3/Shift+F3，按钮只是它的投影。任一档下每个输入框至少留 20 DIP 文字宽度，按钮标签在任一 CJK/UI 字体候选下都完整落在按钮内；
  被省略的按钮既不绘制也不响应按下。（USER 决定，2026-10-21。）
- 鼠标点击输入框必须按该字段的真实 cluster geometry 设置 cursor；找不到安全 cluster boundary 时
  取最近合法 UTF-8 boundary。selection 当前不属于 v1 输入框合同，不得为此引入第二套编辑器。
- 真实 Microsoft Pinyin / WeChat Input Method 的 composition、commit、cancel、Undo、selection replace、
  refocus 和 Search 字段链路属于可客观观测的功能事实，必须由 exact-candidate 物理键盘自动化覆盖；
  synthetic `Ime` event 只证明 reducer，不得替代真实 profile。候选窗位置、遮挡、字体和动画属于视觉
  验收，继续由人工判定。

<a id="font-runs"></a>
## 字体 Run 规则

| 内容 | 首选 | fallback |
| --- | --- | --- |
| 中文/CJK 正文 | 仿宋_GB2312 | 仿宋 / FangSong / 系统 CJK |
| Latin 正文 | Times New Roman | 系统 serif |
| 代码 | Consolas | 系统 monospace |
| 数学 | RaTeX KaTeX fonts | RaTeX 内置 fallback |
| Emoji/特殊字符 | 系统 fallback | 系统 emoji/CJK fallback |

- 按字符脚本分段形成多个 run；Markdown 标点跟随相邻正文 run。
- 示例：`这是 Rust 的 trait 示例` → 仿宋 / Times / 仿宋 / Times / 仿宋。

---

<a id="ime-semantics"></a>
## IME 语义

### preedit vs commit

| 项 | preedit（composition 中） | commit |
| --- | --- | --- |
| 进入规范文档 | 否 | 是（一次性 TextDelta） |
| 触发 autosave | 否 | 是 |
| 进入 undo | 否 | 是（整次提交一个 undo entry） |
| 触发资产 reconcile | 否 | 是 |
| 呈现 | 带下划线的临时 run | 正常文本 |
| cancel | 文档保持不变 | — |

- 候选框位置必须通过当前 caret 屏幕坐标更新：`window.set_ime_cursor_area(...)`。
- 上表是普通组字流程。输入法的范围替换（如 macOS `replacementRange`）与删除周边文本（如 Wayland
  `DeleteSurroundingText`）若需支持，另成一条路径：adapter 把同一输入法事务（如 Wayland 一次 `done` 内的删除与提交）
  合成一个带范围的 typed intent，形成一次 canonical edit 与一个 undo entry；只有删除、没有字符串提交的事务同样产生
  一次编辑。通用 preedit/commit 事件不承诺覆盖这些操作；锁定的 winit 0.30.13 在 macOS 与 Wayland 上丢弃这两类信息，
  对应平台的输入法验收矩阵须包含它们与“只有删除、没有提交”的场景。
- IME composition 期间：窗口**绝不自动收起**（typing guard，见 `09`）。

### 一级验收输入法

Windows：微软拼音、微信输入法。macOS：系统简体拼音、微信输入法（`docs/acceptance-cases/phase-16.md`）。验证项至少包括：

1. 中文连续输入。
2. 中英文混输。
3. 候选框位于 caret 附近。
4. selection 状态下开始 composition。
5. composition 中按左右键。
6. composition 中按 Backspace。
7. commit 后一次 Ctrl+Z 撤销整次提交。
8. composition 取消不污染 undo。
9. 高 DPI 候选框位置正确。
10. 分栏 / 源码 / 吸附展开后都可输入。
11. 整窗透明度 40–100 时输入正常。
12. 失焦/重新聚焦后 composition 状态正确。
13. 输入期间绝不自动收起。

<a id="macos-input"></a>
### macOS 输入（USER 批准 2026-11-10）

- 锁定的 winit 0.30.13 丢弃 `replacementRange`，不提供上下文读取。v1 不支持依赖它们的功能（听写改写、文本替换、输入法修改已提交文本）。一级输入法基本组字失败时停止实现，按 `AGENTS.md` 写 RISK 报告。
- 宿主在本进程的参数域关闭 `ApplePressAndHoldEnabled`（优先于用户与全局设置，不写盘）：长按按系统键盘设置连发，不弹重音菜单，也就不会因替换范围丢失而多出字符；代价是没有重音弹窗。
- 键表（语义相同，只换按键）：保存 / 导出 Cmd+S / Cmd+Shift+S；复制、剪切、粘贴、全选 Cmd+C/X/V/A；撤销 / 重做 Cmd+Z / Cmd+Shift+Z；查找 / 替换 Cmd+F / Cmd+Option+F；视图与缩放 Cmd+1/2/3、Cmd+=/-/0；行首行尾 Cmd+←/→ 与 Home/End；文首文末 Cmd+↑/↓；退出 Cmd+Q（`09`）。Ctrl 组合键不绑定，没有 Insert 别名，Windows 也没有的按词移动不做。

- 验收（USER 批准 2026-11-19）：macOS 的一级输入法验收按 `11` Phase 16 的验收阶梯判定。候选窗的位置、遮挡、字体与动画在 macOS 上先由程序与 agent 判定，做不到的再交 USER 看截图或录像，最后才由 USER 亲手验收。上文“候选窗位置、遮挡、字体和动画属于视觉验收，继续由人工判定”与人工视觉矩阵对 Windows 不变。
---

<a id="undo-grouping"></a>
## Undo 分组

### 范围与限制

- 仅当前进程；重启清空；不写磁盘；不与 autosave 绑定。
- 最多 256 entries 或 4 MiB undo memory，先达到者淘汰最老 entry。

### 合并条件（连续输入可合并为一条）

相邻位置 + 同一输入类型 + 间隔 < 750 ms + 中间无 selection 替换、
无换行、无粘贴、无 IME commit。

### 必须独立成 entry

IME commit、粘贴、图片粘贴、删除 selection、Enter。
外部 reload 不进 undo（清空 undo）；程序化恢复不进普通 undo。

### 图片联动

图片引用变化的 AssetEffect 与文本 delta 同属一个 UndoEntry：
Ctrl+Z 恢复文本并把图片从 `.trash` 恢复；Ctrl+Y 重新移入 `.trash`（见 `08`）。

---

## EditorBackend 抽象与 RichEdit Fallback 治理

```text
trait EditorBackend {
    set_text / apply_delta / selection / set_selection / handle_event / draw
}
实现：CosmicEditorBackend（默认）、RichEditBackend（受控 fallback）
```

### RichEdit fallback 的定位

RichEdit fallback 是 **approved contingency，not default architecture**。

启用审批条件（全部满足才允许）：

1. 已完成至少两轮纯 Rust IME 修复。
2. 微软拼音或微信输入法仍存在阻塞性问题。
3. 问题有可复现步骤。
4. `docs/report/DESIGN_RISK_IME.md` 已记录。
5. fallback 被 feature flag 隔离：`richedit-fallback`。

### fallback 启用后仍必须保持 Rust 实现

DocumentState、Undo/Redo 外层事务、Markdown、数学、Preview、文件系统、
图片、窗口、托盘、Docking。RichEdit 只负责源码输入区。

---

<a id="selection-caret"></a>
## Selection / Caret

selection/caret 是 Editor Session 的非权威 byte-position projection；移动它们不递增
Document generation。grapheme/visual navigation 在 editor 层计算，DocumentState 只校验
最终 mutation range 的 UTF-8 char boundary。

## Inputs

键盘/IME/鼠标事件、DocumentState read projection、主题与 DPI。

## Outputs

EditText/Undo/Redo typed intent、dirty 调度结果、caret 坐标（供 IME 候选框）；不得输出
UI 自称的 deleted text 或 mutable document buffer。

## State Changes

preedit 只改变 ImeState；commit 通过单一 mutation gateway 产生一个 TextDelta。selection/
caret 变化只改变 Editor Session；成功 canonical edit 才推进 generation 和 dirty 状态。

## Failure Paths

| 场景 | 行为 |
| --- | --- |
| delta 非 char boundary | 拒绝并保持状态不变 |
| composition cancel | 文档与 undo 不变 |
| 输入法候选框定位失败 | 回退到窗口左上角附近，不阻断输入 |
| IME 阻塞性问题 | 走 fallback 审批流程，不私自切换架构 |

## Configuration

基准字号/行高为固定 token；全局 `content_zoom_percent` 是唯一内容缩放配置。
键盘 `Ctrl++`/`Ctrl+=`/数字键盘 `+` 与 `Ctrl+-`/数字键盘 `-` 每次 ±10%，
`Ctrl+0` 恢复 100%；`Ctrl+Wheel` 每个完整 notch ±5%，高分辨率滚轮增量先在
Interaction Session 中累计。缩放提交经 ConfigCoordinator 合并写入；普通滚轮仍只滚动。

## Lifecycle

随 Source/Split 视图存在；纯 Preview 模式下编辑器不接收输入。

## Extension / Replacement Points

EditorBackend 平级实现；TextStore String→rope（见 `04`）。

## Performance Critical Paths

按键 → delta apply → 局部重绘；100 KiB / 1 MiB 文档输入延迟见 `10`。

## Verification

- exact-candidate 自动化：微软拼音 + 微信输入法的连续/混输、selection composition、composition
  导航与 Backspace、commit/cancel/Undo、Source/Split/Docked-expanded、40% opacity、失焦重聚焦、
  typing auto-hide guard 与 Search 字段链路。synthetic event 只补充 deterministic reducer 边界。
- 人工视觉矩阵：微软拼音 + 微信输入法 × 100/150/200% DPI，只观察候选窗是否真实出现、是否位于
  caret 附近，以及遮挡、字体、动画和透明度观感；不得重复承担上述客观功能判定。
- 单元/property：UTF-8 byte range、CJK、emoji、combining mark、selection 替换、
  undo grouping、256/4 MiB 限制、commit 一次撤销。
- 验收：AC-002/003/004/009/022。

## Non-Goals

语法高亮、多光标、宏、命令面板、Vim/Emacs 模式、WYSIWYG/Typora 模式、LSP。
