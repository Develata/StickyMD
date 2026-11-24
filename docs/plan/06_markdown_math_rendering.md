# 06_markdown_math_rendering.md - Markdown / 数学 / 预览合同

## Metadata

- `Layer`: Capability
- `Status`: Approved Contract
- `Version`: 0.3.0
- `Last Review`: 2026-11-24
- `Scope`: Markdown 语义权威、数学语义权威、Owned AST、Preview 调度与 generation、渲染桥接、缓存与资源限制

---

## Purpose

定义 StickyMD 预览管线的语义边界：

```text
parser semantics belongs to Comrak
math semantics belongs to RaTeX
StickyMD only owns projection/layout integration
```

StickyMD 不自行实现 Markdown parser，不自行实现 TeX parser/layout。

## Boundary

- 语义判定（什么是公式、什么是代码、delimiter 边界与转义）完全继承 Comrak/RaTeX。
- StickyMD 只拥有：AST 转换、布局集成、选择/复制映射、缓存、错误呈现。

## Owned Objects

`preview::owned_ast`、`preview::render_tree`、`math::display_list`。

---

<a id="markdown-semantics"></a>
## Markdown 权威：Comrak

### 方言

```text
CommonMark 0.31.2
+ GitHub Flavored Markdown
+ Comrak math_dollars
+ Comrak math_latex
```

启用：标题、段落、强调、粗体、删除线、引用、有序/无序列表、task list、
inline code、fenced/indented code block、链接、自动链接、图片、表格、
水平线、soft/hard break、转义、HTML entity、`$...$`、`$$...$$`、`\(...\)`、`\[...\]`。

### 规则

- 数学 delimiter 的边界与转义规则完全继承 Comrak：不自行写智能识别，
  不对 `$5` 等情况额外猜测。
- code span / code block 中的公式标记不得被解释。
- `\$` 按 parser 结果处理。
- 不改变 Markdown parser 语义；升级 Comrak 时方言选项需重新核验并更新本章。
- 当前版本 Comrak 1.2.0（2026-11-24 由 0.59.0 升级，USER 批准，见
  [升级评估](../report/2026-11-24-comrak-1.2-upgrade.md)；0.54.0 → 0.59.0 见
  [上一次评估](../report/2026-11-12-comrak-upgrade-evaluation.md)）。
  - 已核验：上述方言选项不变。`math_dollars_block`（独占一行的 `$$` 之间按块解析）与
    `math_code` 保持关闭，其它扩展也保持关闭（USER 2026-11-24）。
  - 随 1.x 改变、由 Comrak 决定的解析结果：
    - 单美元公式的 literal 保留换行，不再变成空格（`$$` 公式在 0.59 已保留）；其中 `%` 注释止于换行，
      `$a%c\nb$` 显示出 `b`，此前换行变成空格后 `b` 也在注释里；
    - 转义的反斜杠不再转义其后的 closer：`$a\\$ b$` 在 `a\\` 之后闭合，`$$a\$$b$$` 在最后的 `$$` 闭合，
      `\(a\\)b\)` 在最后的 `\)` 闭合；
    - task 标记须照原文出现，并后接空格、Tab 或行尾；
    - 表格单元格中的 `c\\|` 在 `c\` 之后结束单元格；单元格与分隔行的填充只认空格与 Tab；
    - 含实体的 email 不再自动链接，`_www.example.com_` 成为链接；
    - 删除线定界符不匹配时，其后的强调照常解析；
    - 嵌套超过约 50 层的列表标记读作文字；
    - 引用链接的展开额度改为 100 KB 与文档字节数中的较大者，补齐表格空单元格的额度改为全篇共享。
  - 源码位置改为正确的情形：段首链接引用定义之后的段落、表格中 `\|` 之后的内容、以嵌套列表
    结束的列表项、跨行的链接。表格中以 `\|` 开头的文字节点，Comrak 报出的位置从 `|` 开始，
    不含该反斜杠；Owned AST 把这个位置向前扩一个字节，见复制规则 6。
  - 0.59 对未闭合 `\(` 的平方级解析，在 1.2 修复。
  - 更早的 0.59 升级带来的变化：自动链接按 cmark-gfm 识别，`http://localhost/x` 这类短域名与
    大写 scheme（如 `HTTP://www.example.com/`）成为链接。

---

## Raw HTML：literal 呈现

Comrak 识别 raw HTML 节点，但 StickyMD：

```text
不执行 / 不构建 DOM / 不解析 CSS / 不加载 iframe
不运行 JavaScript / 不解释 <style> <script> / 不解释自定义元素
```

Preview 呈现：

- inline HTML → inline-code 风格显示原始文本。
- block HTML → code-block 风格显示原始文本。
- 必须保留用户原文。
- 例外（USER 2026-11-25 接受）：Comrak 1.x 把块内容中的孤立 `\r` 统一为 `\n`，所以 Raw HTML 原文块与
  代码块里的孤立 `\r` 在 Preview 中显示为换行，对象复制（含复制侧条）得到 `\n`；文档本身不变。

---

<a id="ratex-native-math"></a>
## 数学权威：RaTeX

### 支持范围

> RaTeX / KaTeX-compatible 数学语法。

它**不是** TeX Live、完整 LaTeX 文档系统、宏包管理器、`\usepackage` 环境、
任意 TeX 执行器或 LaTeX 编译器。

### 引擎与字体

- 使用 RaTeX 的 parser / layout / types / font crates；纯 Rust，无 JS/DOM/WebView。
- 数学字体使用 RaTeX 配套的 KaTeX-compatible 字体（OFL 1.1），不强制 Cambria Math。
- Release 必须包含第三方字体声明与 OFL 文本。
- Comrak 独占 delimiter 识别；RaTeX 只接收 delimiter 内部的数学 literal。
- Phase 6 采用方案 B：render 层内的薄 DisplayList → tiny-skia painter，覆盖
  `GlyphPath`、`Line`、`Rect`、`Path`，生产热路径不经过 PNG 编解码。
- painter 只解释 RaTeX 已完成的 DisplayList，不实现任何 TeX tokenizer、AST 或数学布局。

### 不得自行实现

TeX tokenizer、数学 AST、分数/根号/矩阵布局、可伸缩括号、上下标算法、数学字距算法。

---

## 公式错误处理

- 解析失败：不 panic；预览显示原始公式文本 + 轻微错误边框 + 简短错误图标；
  hover 显示简化错误信息。
- 不修改 `note.md`，不尝试自动修复。

## 资源限制（保护性）

| 项目 | v1 限制 |
| --- | --- |
| 单个公式源码 | 64 KiB |
| 单文档公式数量 | 2000 |
| 单公式位图 | RGBA ≤ 8 MiB、单边 ≤ 16384 px；独立于缓存预算 |
| 超限行为 | 显示原文及错误提示 |
| 公式后台任务 | 可丢弃过期 generation |
| 公式渲染 | 不得阻塞 UI 线程 |

限制只保护程序，不修改源文件。

---

<a id="owned-ast-projection"></a>
## Owned AST 投影

### 流程

```text
Arc<str> snapshot
→ Comrak parse_document（Arena AST）
→ 遍历转换为 OwnedDocumentTree
→ 释放 Arena
→ RenderTree → Block/Inline Layout → LaidOutDocument
```

### 规则

- **Comrak Arena 不得跨线程或长期保存在 AppState**；必须先转为项目自有 owned tree。
- 节点保存 source range，用于：preview selection、点击链接、错误定位、公式复制、调试。
- raw HTML 以 `HtmlLiteral(String)` 保留原文。

### v1 布局策略

- 不实现增量 Markdown parser；每次刷新全量 parse + 全量布局（后台）。
- 全量布局计算完整尺寸、基线和源码映射；公式位图只在绘制可见内容时生成，不能作为
  公式尺寸的长期持有对象。离屏公式不持有位图或额外长期 DisplayList 引用。
- UI 只绘制 viewport 范围内的 block。
- 旧 preview 在新结果完成前保持可用；generation 不匹配的结果立即释放。
- 这是用 debounce 换取架构简单与稳定性的明确决策。

### Preview 保护性上限

这些上限只阻止异常输入耗尽资源，不改变或截断 canonical source：

| 项目 | 上限 | 超限行为 |
| --- | --- | --- |
| Preview source snapshot | 5 MiB | 保留 Source 编辑，Preview 返回可见错误 |
| Owned AST 深度 | 256 | 丢弃该 generation 的 Preview 结果 |
| Owned AST 节点 | 200,000 | 丢弃该 generation 的 Preview 结果 |

Phase 5 foundation 曾在 RaTeX 正式接入前把 Comrak 已识别的四类公式节点保留为原文
placeholder；Phase 6 已用 RaTeX native projection 替代该数学 placeholder。图片解码仍留给
Assets Phase，当前 foundation 只显示安全 placeholder、alt 与路径/远程链接。

---

<a id="preview-scheduling"></a>
## Preview 调度与 Generation

```text
分栏已打开：停止输入 → 1000 ms debounce → 后台解析最新文本 → 构建预览
             → generation 仍为最新才提交（连续输入重置计时器）
切换纯 Preview：立即刷新；可暂显旧预览；新结果原子替换；过期结果丢弃
```

Preview 只读，但必须支持：鼠标选择文字、Ctrl+C、滚动、点击允许的链接、
公式错误提示、本地图片显示。

### Preview 文本选择

Preview 选择必须使用 shaping 产生的真实 cluster 几何；禁止用字符数、字素数或整段宽度比例
反推 byte boundary 或 selection rectangle。变宽 Latin、CJK、Emoji、组合字符、连字、换行和
BiDi 都必须经过同一份坐标—文本映射。

权威边界：

- `DocumentState` 仍是 canonical text authority；Preview 只持有不可变 projection。
- Cosmic Text 的 layout run / glyph cluster 是当前排版结果的几何事实，但不得作为长期文档
  authority，也不得越过 Render crate 暴露给 Interaction Shell。
- `PreviewDocumentProjection` 持有 generation、完整可复制显示文本 `Arc<str>` 与语义滚动 anchor；
  它随 semantic/layout generation 更新，不因选择或 hover 变化而复制全文。
- `PreviewFrameGeometry` 只持有当前 viewport 的行、cluster、原子对象与 action 几何；它是可丢弃
  的帧投影，不是第二份全文索引。
- 每个已排版文本 block 可以附带一个紧凑 visual-row locator（logical line、layout row、top、height、
  logical byte base）。它只定位 Cosmic Text 已经持有的 layout row，不复制 glyph、cluster、文本或
  action；其目的是让 viewport 投影通过 y 二分直接访问可见 row，避免滚动时重新线性遍历长代码块
  或超长换行段落的全部行。

每个可见文本 cluster 至少保存：

```text
selection byte start/end
leading x / trailing x
row y / height（由所在 visible row 持有）
```

规则：

- byte range 映射到 Preview 的 display/clipboard projection，不直接等同 Markdown source range；
  link action、diagnostic 与 scroll anchor 另持 source range。
- 同一 shaping cluster 产生多个 glyph 时，按相同 byte range 合并为一个 cluster 几何；不得为每个
  glyph 重复保存 byte range。`leading_x > trailing_x` 可以表达 RTL 视觉方向。同一 link/tooltip
  payload 跨多个可见 cluster 时必须共享所有权，不得为每个 cluster 复制 destination 字符串。
- hit-test 先按 y 二分找到 visible row，再在该行有序 cluster 中定位 boundary；cluster 内部不能
  安全细分时遵循 Cosmic Text 的 cluster boundary/fallback，不自行猜测 GDEF、ligature caret 或
  grapheme 比例。
- byte range → selection rectangles 与 point → byte boundary 必须消费同一份 cluster map；跨行、
  BiDi 或不连续视觉范围允许产生多个矩形。复制取什么文本由 #preview-copy-semantics 规定，
  不能从蓝框或 glyph 重新拼文本。
- 公式和图片是 atomic selectable object：选择公式时绘制公式矩形，选择图片时绘制图片矩形；
  复制结果见 #preview-copy-semantics，图片不复制 bitmap。它们不得伪造成普通等宽文本 cluster。
- `PreviewFrame` 只向 App 暴露 `hit_test`、`selection_rects`、`copy_text`、`action_at`、
  `tooltip_at` 等语义方法；App 不读取 cluster 数组，不依赖 Cosmic Text 类型。

生命周期与缓存：

- geometry cache key 至少包含 `generation + viewport width/height + scroll + block horizontal offsets + content scale`；影响
  shaping 的主题/字体 token 也必须使其失效。selection、hover、clipboard copy 不得触发 relayout
  或重建 cluster map。
- 构建新 Preview frame 时只投影与 viewport 相交的行和少量既有 overscan；滚动离开后旧 frame
  geometry 可直接释放。不得为整篇长文永久保存每个 glyph/cluster rectangle。
- stale generation 或不匹配 viewport key 的 frame 必须丢弃。任何非法 UTF-8 boundary、倒序范围
  或越界映射均 fail closed：跳过有问题的几何/拒绝提交该 frame，不 panic、不修改 Document。

复杂度与资源目标：

- visual-row locator 构建为每次 block layout 的 O(block visual rows)，viewport geometry 构建为
  O(log block visual rows + visible glyphs)；行命中 O(log visible rows)，行内命中可二分为
  O(log row clusters)，selection paint 为 O(intersecting visible clusters)。
- 额外长期内存为 O(document blocks + visual rows + clipboard text)，其中 row locator 是固定宽度
  小记录；精确 cluster 几何只为 O(viewport clusters)。典型窗口应为数十 KiB 量级，不得演变为
  全文 glyph cache。
- 不新增 DirectWrite、HarfBuzz 或另一套 shaping dependency；精确性来自保留现有 Cosmic Text
  已经计算出的 cluster 事实，而不是重复排版。

<a id="preview-copy-semantics"></a>
### Preview 复制语义（Phase 17，USER 批准 2026-11-14）

依据 [提议报告](../report/2026-11-14-copy-semantics-rail-notices.md) 与 `02` #copy-what-you-see。
上文“Preview 文本选择”与“块内横向滚动”中原先的复制条款（复制读取显示投影、公式复制带分隔符）已按本节改写（Phase 17 M3）。

**不变：** 选择几何、命中与蓝框；单独选中图片时复制 alt 文本。

**规则。** 选区先规范化。

1. **对象内容。** 选区完全落在一个对象的内容范围内（不含列表序号与容器前缀）时，只复制内容：
   - **公式**（块级与行内，原子对象）：不带分隔符的 TeX，即 Comrak 给出的 literal。
   - **代码块与 Raw HTML 原文块：** 选中的那部分代码文本，不带围栏，也不带引用或列表前缀。
   - **图片：** alt 文本（现行行为）。alt 为空的图片无法单独选中；改成复制源码需要新的选区表示，不在 Phase 17 范围内。
2. **源码切片。** 其他情况，把两个端点映射为该帧 generation 的源码字节位置，复制这段源码。
   - 端点位于块内可见文字的中间时，映射到对应的源码位置。
   - **起点**位于某个块首个可见位置时，沿祖先向上，找出首个可见位置与它相同的最外层块，取该块的源码起点。
     - 参与外扩的块：标题、段落、引用、列表、列表项、表格（整张）、代码块（第 1 条以外的情形）。
     - 不参与外扩的块：Document、表格行、单元格。表格内的端点按单元格内容映射；只有在表格首个或末个可见位置时，才取整张表的边界。
     - 列表项的首个可见位置是 Preview 生成的项目符号或序号（它们可以被选中）。
       所以从标题、引用的第一个字开始选，会带上 `# `、`> `；从列表的项目符号开始选，才带上 `- `；只从正文第一个字开始选，不带列表前缀。
   - **终点**位于某个块末个可见位置时同理，取最外层块的源码终点。
   - 源码中不存在的包装不会补出来。例如，复制引用式链接的段落时，不会补上链接定义。
3. **被切开的结构（第一期，只用于 Preview）。** 一个结构“被切开”，指恰好一个端点在它内部。
   恰好落在结构可见内容边缘的端点算作在结构外，映射到定界符外侧，与第 2 条的块边缘规则一致。
   例如在 `x **abcd** y` 中：选 `abcd` 得 `**abcd**`；选 `x abcd` 得 `x **abcd**`；选 `abcd y` 得 `**abcd** y`。
   行内代码按行内结构处理；第 1 条的对象只包括公式、代码块、Raw HTML 原文块与图片。
   两个端点都在同一个行内结构内部时，它没有被切开，按第 2 条只取里面的文字，例如在 `**abcd**` 中选 `bc`，得到 `bc`。
   - **围栏代码块：** 复制开围栏行、选中的行、闭围栏行，三者均取自源码。没有闭围栏的围栏代码块，按第 2 条原样切片。
   - **行内结构**（强调、粗体、删除线、行内代码、行内链接与图片）：复制时把在它内部的那个端点扩到该结构的源码边界；蓝框不变。
     嵌套时重复这一步，直到没有被切开的行内结构；端点只会向外移动，并停在所在块内，所以这个过程必然终止。
     - **外扩结果须校验：** 结构的源码范围必须以它自己的闭定界符结束（链接以 `)` 或 `]` 结束，强调以与开头相同的定界符串结束）。
       自动链接 `<…>` 以 `<` 开头、`>` 结束；裸 URL 与裸邮箱没有定界符，源码范围须恰好等于它显示的文字（USER 决定 2026-11-17）。
     - 校验失败时，端点吸附到所在块的整行范围。这是保守兜底：StickyMD 已知的 Comrak 1.2 输入都通过这项校验；0.59 曾对 URL 与 `)` 之间有换行的链接，报出停在 URL 末尾的终点。
   - **其余结构**（缩进代码、表格、引用、列表等）：按第 2 条原样切片。
   - 用结构自己的定界符只包住选中部分（行内），不在本期，另行批准。
4. **快照。** 复制读取该帧 generation 的源码快照，与 preview worker 的快照共享同一份 `Arc<str>`，不读当前 DocumentState。
   - 当前显示帧在新帧到达前可以落后于 DocumentState；debounce 随编辑重置，所以可以持续落后。
   - 在落后的显示帧上复制已有选区、点击复制侧条，都读该帧自己的快照。新的选区变化沿用现行规则。
   - **显示帧**指已装入 Preview 窗格的帧（USER 决定 2026-11-17）。新帧到达即装入，选区、复制、链接与侧条随即以它为准；
     它在下一次窗口呈现时出现在屏幕上。在此之前的点击按新帧处理：通常相隔一次事件循环，呈现被推迟时每 8 ms 重试，没有总时长上限。
   - 上文“stale generation 或不匹配 viewport key 的 frame 必须丢弃”针对与请求不符的新结果与几何，不针对当前显示帧。
5. **全选无特例。** Ctrl+A 仍是普通选区 `[0, 全文长度]`。只含一个对象的文档，全选按第 1 条处理。
6. **位置不可信时保守处理：**
   - 端点落在显示文本与源码不一一对应的 span 内（转义、实体、软换行、硬换行，以及经 Comrak 规范化的行内代码）：按 span 边界向外取整。
     行内代码的显示文字与源码内容逐字节相同时才算一一对应。取整后落在行内代码内容边缘的端点，按第 3 条算作在结构外。
     - 表格中（含 Comrak 从表格上方拆出的段落）的 `\|` 也是转义：以它开头的文字，Comrak 报出的位置不含反斜杠，Owned AST 向前扩一个字节，
       使文字与源码不再一一对应，端点照常向外取整，复制保留反斜杠；外扩校验也因此找到强调自己的开定界符（如 `**\|x**`）。
       “上方拆出的段落”指与表格之间没有空行的段落。文中第一个孤立 `\r` 之后不做这项扩展：那里的行号已与 Comrak 不一致。
   - 端点落在没有源码范围的生成文字上（列表序号等）：吸附到所属列表项或块的源码起点。
   - 映射得不到合法 UTF-8 边界时 fail closed：不写剪贴板，经 `09` #notices 报复制失败。

**执行顺序。** 第 1 条 → 第 6 条（取整与吸附）→ 第 3 条 → 第 2 条。第 4、5 条约束的是输入（快照与选区），不参与排序。

**复制映射。**

- Render 为每个 generation 生成，并随帧保留：
  - span 到源码的范围，以及该 span 是否与源码一一对应；
  - 行内结构与围栏代码块的源码范围；
  - 对象的内容范围。
- App 只经 `PreviewFrame` 的语义方法取复制文本与复制侧条目标，不读映射内部；Render 不反向依赖 App。
- 长期内存增加 O(span + 结构 + 对象)，外加帧持有的源码快照；两者随帧释放，不保留历史帧（`10`）。复制时每个端点做一次二分查找。
- 测试不变式：
  - 第 2、3 条的结果，每个字节都取自该帧源码快照，并保持源码顺序；
  - 第 1 条的结果取自对象自身的文本。

**Source 复制不变**，仍是原样切片（`07`）。引用中的代码这类情况下，Source 与 Preview 的结果可以不同（Source 带 `> `）。
这是两种视图各自的取文路径，不是缺陷。

<a id="preview-copy-rail"></a>
### 块复制侧条（Phase 17，USER 批准 2026-11-14）

- **对象。** 代码块（含 Raw HTML 原文块）与块级公式。行内公式、表格、图片没有侧条。
- **几何**（内容 DIP，随内容缩放）：
  - 侧条外接于块背景右缘，与背景同高，不包含横向滚动条轨道。
  - 宽度暂定 8，视觉验收后固定。
  - 只在整根侧条都落在 Preview 内容区内时绘制；高缩放、窄窗、深缩进时不画，选区复制照常可用。
  - 不改排版，不与正文、背景、滚动条重叠。
- **交互。**
  - 可点区域等于可见矩形；命中优先于开始选区。
  - 沿用 3 DIP 的点击/拖动阈值：在侧条上按下后拖动，既不复制也不开始选区。
  - 悬停时变亮并切换光标，只在进出侧条时重绘。
  - 常显、低对比，没有图标。40% 透明度下可能不易看清，USER 接受（2026-11-14）。
- **复制内容。** 该对象的完整内容（按上节第 1 条），结果经 `09` #notices 提示。
  - 只按第 1 条取，不退到第 2、3 条的源码切片，所以不会带出围栏或容器前缀。
  - 内容即 Comrak 给出的 literal：代码块含末行的换行，多行 `$$` 公式含定界符内侧的换行（Phase 17 M4 落地）。
  - 空代码块复制空字符串，剪贴板不改动，也不提示。
- **目标来源。** Render 每帧产出可见侧条目标，做法同横向滚动目标；侧条不加入排版，也不改变块的几何。
  - 侧条画在显示帧之上，不画进帧的像素：悬停只重绘窗口，不重绘帧（Phase 17 M4 落地）。
  - 命中、悬停与复制针对当前显示帧，帧落后于文档时也一样（上节第 4 条）。

<a id="split-scroll-sync"></a>
### Split 语义滚动同步

- Split 提供配置开关，默认开启；关闭时 Source 与 Preview 继续使用各自保存的滚动位置。
- 当前滚动手势所属面板是唯一 driver。它把 viewport 顶部映射为 `source byte + block 内比例`，
  另一侧通过当前 generation 的有序 source-range anchor index 做一次目标定位；同步更新不得反向
  触发第二轮滚动。
- 不得直接绑定滚动条百分比。标题、换行、表格、公式与图片会让两侧高度非线性变化。
- anchor index 随现有布局以 O(blocks) 构建；每次映射用二分查找 O(log blocks)、O(1) 临时空间，
  并合并为每次 redraw 最多一次 target update。
- Preview generation 落后于 Document 时暂停同步并保留两侧位置；没有精确 range 的节点使用最近
  稳定 source block 与 block 内比例，不得猜测或修改 canonical text。

---

<a id="preview-link-safety"></a>
## 链接安全

- 允许点击并交给系统 Shell：`http`、`https`、`mailto`、`file`。
- 自定义 URI scheme：可显示为链接、hover 显示目标，但不执行。
- 本地链接：相对路径以 note.md 所在目录为基准，交给系统处理；不在程序内建文件浏览器。

---

## Remote 图片零网络规则

```text
![alt](https://example.com/a.png)
```

- 不发起网络请求、不下载、不缓存。
- Preview 显示 alt text + 可点击链接。
- 导出时保留原 URL。
- 程序默认不需要网络权限或 HTTP client dependency。

---

## RaTeX 渲染桥接

### 原型阶段（spike）

允许 `DisplayList → render_to_png → decode → preview`，仅用于验证
parser/layout/字体/delimiter/正确性。

### 正式实现（beta 前禁止保留 PNG encode/decode 热路径）

优先级：

1. **方案 A**：向 RaTeX 上游贡献 `render_into_pixmap(display_list, pixmap, origin, options)` API（首选）。
2. **方案 B**：项目内维护很薄的 DisplayList painter（约 200–400 行，
   只绘制 GlyphPath/Line/Rect/Path；保留 MIT attribution；与 golden tests 对照）。

### 禁止

fork 整套 RaTeX、自行实现数学布局、运行外部 LaTeX、调用浏览器 KaTeX、
启动 Node.js、把公式交给 WebView2。

---

<a id="native-preview-layout"></a>
## 渲染与缓存

- 代码块：Consolas，不做语法高亮；fenced info string 可作顶部小标签；
  保留原始行，超长行在块内横向滚动，不得撑宽窗口或按普通正文自动换行。
- 表格：GFM alignment、单元格换行、基本边框、行背景轻微交替、宽度不足区域横向滚动；
  不支持列宽拖动与单元格编辑；task checkbox 只读。
- MathLayoutCache：source + display_mode → DisplayList；最多 512 entries 且计费容量 ≤ 8 MiB。
  继承正文的默认颜色保留为逻辑标记，theme foreground 在 raster 阶段解析；显式 TeX 颜色保留。
- MathRasterCache：layout key + effective font size（已折入 DPI 与 Content Zoom）+ theme foreground → raster；
  驻留预算 ≤ 8 MiB，与单公式分配上限分别定义、分别测试。预算值按性能合同实测评估。
  缓存不接纳时允许当前公式使用一个受单公式限制的临时位图，绘制完立即释放；不得把
  缓存压力转换为公式语法错误。峰值统计同时计入缓存、临时位图及 painter 分配。
- painter 的复用 glyph outline 采用独立 ≤ 4 MiB bounded cache；不使用上游无界 outline cache。
- DecodedImageCache：只缓存 viewport 附近图片；≤ 16 MiB；LRU 淘汰。
- 单次文档 layout 内允许复用等价文字 shaping；只在第二次出现后 admission，最多 1024 个、
  单 key 文本最多 1024 bytes，layout 完成即释放索引，不跨 generation 保存历史 Preview。
- 进入纯 Source 或隐藏（tray/dock collapsed）一段时间后：清理解码图片与公式 raster，
  保留小型 layout cache、文档与字体数据库，不保留无必要 framebuffer 副本。
- Content Zoom 改变时复用已解析的 OwnedDocumentTree/RenderTree，重新计算全文尺寸布局、
  只生成可见内容的位图；
  不重新运行 Comrak，也不推进 Document generation。数学只失效受有效字号影响的 raster，
  保留语义/layout authority；图片按缩放后的布局框请求受 16 MiB 预算约束的 viewport raster，
  不预生成 300% 全文大图。

<a id="frame-presentation"></a>
### 帧呈现与平台字体

- 绘制与命中由 `stickymd-render` 与 Shared Desktop 共用；呈现到窗口的方式与成本属于平台。共享绘制只给出 damage 区域，
  是否局部呈现由 Shared Desktop 依后端缓冲区年龄决定：只有缓冲区就是上一帧（softbuffer `age == 1`）时才只写
  damage 区，其他年龄（新缓冲 `0`、双缓冲取到更早帧 `≥ 2`）全量填充并完整呈现。不以 Windows 的局部呈现成本
  代表其他平台（例如 softbuffer 0.4.8 的 macOS 后端每帧新分配缓冲、忽略 damage，按全窗成本单独测量，见 `10`）。
- **宿主呈现（macOS，USER 批准 2026-11-14，[方案 B 报告](../report/2026-11-14-macos-plan-b-iosurface.md)）：**
  - **谁来提交。** `Host::presents_frames` 为真的宿主自己提交帧：Shared Desktop 只持有画布（`SoftwareSurface::for_host`），不创建 softbuffer 表面，把完成的整帧交给 `Host::present_frame`。
  - **macOS 的做法。** 宿主把整帧写进 IOSurface，设为子图层内容，CoreAnimation 不再复制。Windows 不实现这个能力，呈现不变。
  - **三种结果：**
    - **已提交：** 只有这种才算帧到达窗口，才走 `after_present` 与光标同步。
    - **暂缓：** 呈现缓冲都在用，窗口保留上一帧；整帧在 `PRESENT_RETRY`（8 ms）后重画。只保留一个等待中的唤醒，成功提交或隐藏时取消。
    - **失败：** 报告诊断，不重试，上一帧保留。
  - **不做局部提交。** 宿主呈现总是整帧，光标的局部提交在这时改走整帧。
- 字体选择算法共用；各平台的默认字体候选由 Platform Host 提供，Windows 现有默认（含代码块 Consolas）保持不变。

<a id="preview-horizontal-scroll"></a>
### 块内横向滚动

- 表格根据单元格内容测量列宽并保留合理换行；总宽度超过内容区时局部横向滚动。
- 独立公式在可用宽度内保持居中；超宽时从左端开始，使用块内横向滚动完整阅读。
  保留数学引擎给出的尺寸与断行，不以窗口宽度强制缩小公式或自行重排 TeX。
- 表格、代码块与独立公式共享最小的块视口、裁剪与坐标转换机制，各自仍拥有各自布局规则。
  代码与公式的块背景固定在视口内，不随内容横移。
- 横向偏移属于 Editor Session 的临时视图状态，按 generation 内的 block identity 定位；
  source、窗口宽度或有效字号变化后重置/校验，不持久化、不递增 Document generation。
- 支持指向块内的横向滚轮与 Shift+滚轮；可见的块内滚动条提供鼠标拖动和轨道点击。
- 渲染、selection、hit-test、link/tooltip 共用同一转换和裁剪；复制按 #preview-copy-semantics 读取该帧的源码快照，与横向偏移无关。
  App 只消费语义滚动目标/范围，不读取 Render crate 内部行、cluster 或布局数组。
- 视图快照和结果校验包含横向偏移；过期帧不得恢复旧偏移。普通滚动不触发全文重排。

<a id="math-raster-lifecycle"></a>
### 公式资源生命周期

- MathEngine 独占 RaTeX 布局与栅格化；尺寸测量与位图获取是独立能力。
- 数学布局驻留缓存同时限制 512 条目与 8 MiB，计费含 DisplayList 动态容量、源码及元数据估算；淘汰后按需重建。
- 全文布局保存公式源码共享引用、尺寸、基线及 source/selection 映射；DisplayList 复用
  只属于有界数学布局缓存，不把全部数学布局或位图固定在 LaidOutDocument 中。
- PreviewPipeline 的现有 worker 在 viewport 绘制时顺序获取公式位图、绘制、释放借用；
  不增加公式线程、异步任务队列、全局资源协调层或 UI 线程栅格化。
- 无论全文还是当前一屏的公式位图总和超过缓存预算，单个合法且未超限公式仍可显示；
  内存上界由驻留缓存加一个单公式临时工作集确定，不以全文公式数量乘位图大小增长。
- 回收或未命中不得改变文档高度、公式基线、源码复制及滚动锚点。真正的语法、单公式
  资源或 painter 错误单独传播；原文保持可复制，不通过反复重排重试缓存接纳。

---

## Inputs

`doc::snapshot`（带 generation）、主题、DPI 与 Content Zoom 上下文。

## Outputs

LaidOutDocument（带 generation，原子替换）、错误提示事件与选择映射；所有输出均是
projection，不得反写 source。

## State Changes

PreviewState 只按 `Dirty → Scheduled → Rendering → Clean/Failed` 推进；结果 generation
不等于当前 DocumentState generation 时直接丢弃，不改变当前预览或文档。

## Failure Paths

| 场景 | 行为 |
| --- | --- |
| 公式解析失败 | 显示原文 + 错误提示，不 panic |
| 公式超限 | 显示原文 + 提示 |
| 公式缓存不接纳 | 当前公式按需生成、绘制后释放；不产生公式错误占位 |
| stale 结果 | 丢弃 |
| malformed Markdown | 不 panic，按 parser 结果呈现 |
| remote 图片 | 不下载，显示 alt + link |
| 图片解码失败/超限 | 占位符，不修改源文件 |

## Configuration

Preview debounce 1000 ms 为固定内部参数；排版 token 固定。唯一用户排版比例是全局
Content Zoom（50–300%，默认 100%），它只缩放内容投影而不改变 Markdown 语义或 Shell。

## Lifecycle

Preview 结果随新 generation 替换；隐藏时按缓存策略清理。

## Extension / Replacement Points

渲染桥接方案 A/B（见上）；Comrak 版本升级需重新核验方言。

## Performance Critical Paths

全量 parse + layout（后台单线程）；viewport culling；缓存命中率。
量化目标见 `10_performance_reliability.md`。

## Verification

- fixture：全部 CommonMark block、GFM 表格、task list、strikethrough、autolink、
  四种 delimiter、转义 dollar、code 中公式标记、raw HTML literal、reference link/image、
  malformed input。
- 数学 fixture：分数、根式、上下标、积分、求和、极限、矩阵、cases、align、
  可伸缩括号、Greek、`\mathbb`、`\mathbf`、`\operatorname`、Unicode 数学字符、错误公式、超长公式。
- fuzz：`fuzz_markdown_to_owned_ast`、`fuzz_render_tree_builder`。
- golden：Light/Dark × 100/150/200% DPI（固定测试字体，小 AA 容差）。
- 验收：AC-013/014/015/016/017。

## Non-Goals

- 增量 parser、语法高亮、Mermaid/PlantUML、HTML/PDF 导出、可配置排版。
