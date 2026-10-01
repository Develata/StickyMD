# Preview 渲染规则、优先级与实现差异

日期：2026-09-30。审查源码：`be1322e43624908b7ad04c7c7942d2c3ecb9ed09`。

本文集中说明该源码中 **实际存在** 的 Markdown / 数学 Preview 行为，包括解析边界、样式
叠加、布局参数、缓存、刷新顺序、复制和失败处理。它是源码审查报告，不是新工程合同，
也不声明 `v0.1.1` 发布 ZIP 已按此源码重建。实现不符合合同的地方在文末单列。

现有 [渲染合同](../plan/06_markdown_math_rendering.md) 已规定架构与应有行为；
[Phase 5](phase-05-markdown-native-preview.md)、[Phase 6](phase-06-ratex-native-math.md)
记录阶段实现。本报告补充的是分散在当前实现中的具体规则，不复制一份平级规范。

## 目录

1. [范围与依据](#scope)
2. [整个处理链](#pipeline)
3. [Markdown 方言与语法优先级](#syntax)
4. [表格中的竖线、公式与源码坐标](#table-pipes)
5. [块结构怎样投影](#blocks)
6. [行内样式怎样叠加](#inline-style)
7. [数学公式](#math)
8. [图片](#images)
9. [链接与原始 HTML](#links-html)
10. [字体、尺寸、颜色和绘制顺序](#appearance)
11. [刷新、任务合并与过期结果](#scheduling)
12. [滚动、缩放与缓存](#cache)
13. [选择、复制与 Split 同步](#selection)
14. [限制与失败路径](#failures)
15. [实现与合同的差异、后续路径](#differences)
16. [验证结果与未验证项](#verification)

<a id="scope"></a>
## 1. 范围与依据

### 1.1 三种结论要分开

| 种类 | 本文含义 |
| --- | --- |
| 合同要求 | `docs/plan/` 已批准的行为；链接到原章节 |
| 当前实现 | 上述 source 中实际调用、条件和参数；有源码入口 |
| 验证证据 | 对指定输入实际执行的测试；不扩展为全部语法、字体或真实桌面验收 |

以下像素、颜色、缓存键与内部优先级均属于当前实现的观察值。它们不是用户可配置接口，
也不是允许绕过合同的新承诺。报告发现的差异没有通过本次文档更新获得批准。

### 1.2 主要依据

| 主题 | 合同 | 当前实现入口 |
| --- | --- | --- |
| Markdown 语义、owned AST | [plan 06](../plan/06_markdown_math_rendering.md#markdown-semantics) | [parser.rs](../../crates/stickymd-render/src/preview/parser.rs)、[source_map.rs](../../crates/stickymd-render/src/preview/source_map.rs) |
| 视觉语义、文字与表格布局 | [native Preview](../plan/06_markdown_math_rendering.md#native-preview-layout) | [render_tree.rs](../../crates/stickymd-render/src/preview/render_tree.rs)、[layout.rs](../../crates/stickymd-render/src/preview/layout.rs)、[table_layout.rs](../../crates/stickymd-render/src/preview/table_layout.rs) |
| 数学解析与绘制 | [RaTeX](../plan/06_markdown_math_rendering.md#ratex-native-math) | [math/engine.rs](../../crates/stickymd-render/src/math/engine.rs)、[math_layout.rs](../../crates/stickymd-render/src/preview/math_layout.rs) |
| 图片读取与安全限制 | [plan 08](../plan/08_assets_and_export.md#local-image-read-boundary) | [image_layout.rs](../../crates/stickymd-render/src/preview/image_layout.rs)、[image.rs](../../crates/stickymd-render/src/image.rs)、[assets/path.rs](../../apps/stickymd-win/src/assets/path.rs) |
| 调度与完成结果 | [Preview scheduling](../plan/06_markdown_math_rendering.md#preview-scheduling) | [flow/preview.rs](../../apps/stickymd-win/src/flow/preview.rs)、[preview/worker.rs](../../apps/stickymd-win/src/preview/worker.rs)、[pipeline.rs](../../crates/stickymd-render/src/preview/pipeline.rs) |
| 选择与滚动 | [owned projection](../plan/06_markdown_math_rendering.md#owned-ast-projection)、[Split 同步](../plan/06_markdown_math_rendering.md#split-scroll-sync) | [text_layout.rs](../../crates/stickymd-render/src/preview/text_layout.rs)、[scroll.rs](../../crates/stickymd-render/src/scroll.rs) |

审查时 [Cargo.lock](../../Cargo.lock) 固定 Comrak 0.54.0、cosmic-text 0.19.0、
RaTeX parser/layout 0.1.14、tiny-skia 0.12.0 和 image 0.25.10。本文不把其他版本或浏览器
Markdown 的表现当作当前行为；依赖升级后需重新核验方言和投影。

<a id="pipeline"></a>
## 2. 整个处理链

```text
DocumentState 的不可变 snapshot（文本 + generation）
    ↓ 一个 Preview worker
Comrak：识别 Markdown / GFM / 数学分隔符
    ↓ 只在本次 parse 中存活的 Arena
OwnedDocumentTree：保留结构、源码范围和公式原文
    ↓ RenderTreeBuilder
RenderTree：块种类、行内样式、复制文本、链接动作、数学/图片对象
    ↓ 布局
cosmic-text 文字 + RaTeX 公式 + 本地图片投影 → LaidOutDocument
    ↓ 可见块/行筛选、选择几何、tiny-skia 绘制
带 generation 的 PreviewFrame → UI 检查是否仍适用 → 呈现
```

- `DocumentState` 仍是运行时文本的唯一权威。Preview 的树、布局、位图、选区文本都可丢弃。
- Comrak 负责语义，StickyMD 不用正则再实现一套 Markdown / 数学分隔符解析器。
- RaTeX 只接收已识别公式的 delimiter-free literal，不决定哪里开始或结束公式。
- 当前文本更新是后台全量 parse 与全量 layout；只绘制 viewport 内需要的内容。
- 普通 resize / scroll 复用已有语义树。图片懒加载、缩放和主题变化的额外布局见第 12 节。
- 没有 HTML 中间页面、DOM、CSS cascade、WebView 或 JavaScript 执行阶段。

这里有四种不同的“优先级”：**语法上下文、样式属性选择、对象布局、任务新旧顺序**。
不能用一条“公式高于代码、代码高于链接”的总排序概括它们。

<a id="syntax"></a>
## 3. Markdown 方言与语法优先级

### 3.1 开关与不支持的扩展

`markdown_options()` 从 Comrak 默认选项开始，只显式开启以下六个扩展：

| 选项 | 作用 |
| --- | --- |
| `table` | GFM 表格 |
| `autolink` | GFM 自动链接 |
| `tasklist` | 任务项标记 |
| `strikethrough` | 删除线 |
| `math_dollars` | 美元分隔符公式 |
| `math_latex` | LaTeX 风格分隔符公式 |

基础块和行内语义沿用 Comrak 的 CommonMark 实现；契约目标为 CommonMark 0.31.2 + GFM。
选项锁定测试也检查 footnotes、math-code、WikiLink、description lists 等扩展未开启。
这不是整套上游规范测试已在本轮穷尽通过的声明。

Mermaid / PlantUML 没有解释器；标为 `mermaid` 的 fenced code 仍是代码文字。WikiLink、
脚注和其他未启用扩展没有 StickyMD 专属语义；相关字符仍按启用的 Markdown 方言处理，
不能笼统保证每一种未启用语法都会逐字呈现。

### 3.2 应按上下文理解的优先关系

| 输入所处位置 | 实际决定规则的阶段 | 结果 |
| --- | --- | --- |
| fenced / indented code block 内 | Comrak 块结构 | 内容保留为 code literal，不递归识别里面的公式、图片或 HTML |
| inline code 内 | Comrak 行内解析 | code literal 不进入 RaTeX，不建立图片读取动作 |
| GFM 表格行 | Comrak 表格识别与分列 | 竖线先影响表格结构；公式书写不能自行保护未转义的竖线 |
| Comrak Math 节点内 | RaTeX | `*`、`_`、反斜线等交给数学语义，不再次作为 Markdown emphasis 解析 |
| ordinary text / emphasis / link label | Comrak 行内结构，再递归投影 | 已识别的 strong、emphasis、strike、link 属性可以叠加 |
| raw HTML 节点 | StickyMD literal 投影 | 标签文字显示出来，不应用标签、CSS 或脚本 |
| image destination | 图片分类与 execution-domain resolver | 决定占位、只读本地加载或 HTTP/HTTPS 链接，不由 alt text 决定 |

反斜线转义、实体解码、反引号长度、括号匹配和未闭合 delimiter 都由 Comrak 处理。
StickyMD 不按“看到 `$` 就渲染”的启发式工作，也不追加 `$5` 等货币特判。
文本节点可能已被 Comrak 规范化；Preview 的普通文字复制不是 Markdown 原始 bytes 的副本。

### 3.3 公式的四种入口

| 写法 | Comrak 识别后保留的信息 | 常见投影 |
| --- | --- | --- |
| `$x$` | delimiter-free `x` 与原始 `$x$` | inline math |
| `\(x\)` | delimiter-free `x` 与原始 `\(x\)` | inline math |
| `$$x$$` | display 标志、literal 与完整原文 | 独立段落内单一公式可升为 display block |
| `\[x\]` | display 标志、literal 与完整原文 | 同上 |

真正的 display block 还要求 **整个 paragraph 恰好只有一个、带 display 标志的 Math 子节点**。
混合文字、emphasis 或表格单元格中的公式走 inline span 通路；当前该通路使用
`MathKind::Inline`。因此 delimiter 形式、语义节点位置和最终块布局不能混为一谈。

反引号中的 `$x$` 仍是代码。原始 HTML 属性中的公式字符不会被 StickyMD 单独提取执行。
独立公式、容器内公式、四种 delimiter 的复制与过宽公式已有针对性测试，见第 16 节。

<a id="table-pipes"></a>
## 4. 表格中的竖线、公式与源码坐标

这是当前最容易误判为“公式优先级错误”的输入之一。

```markdown
| 对象 | 大小 |
| --- | --- |
| 邻居集合 | $\lvert\mathcal N_i\rvert$ |
```

- 表格里的裸竖线是 GFM 分列字符。把绝对值直接写成 `$|\mathcal N_i|$`，不会因为外层
  有美元符号就自动成为完整单元格；还可能使表头与分隔行列数不匹配，整段失去表格身份。
- 仓库用例覆盖 `\lvert…\rvert`、`\left\lvert…\right\rvert` 以及 Markdown 转义竖线
  的写法。选择 LaTeX 命令时也应注意其数学含义，不能把单竖线和双竖线记号随意互换。
- code span 也不能被当成 GFM 表格分列的通用豁免；具体识别继续服从 Comrak。
- StickyMD 对表格转义维护的是 **坐标映射**，不是另写语法修复器。Comrak 消费转义后，
  `source_map` 把节点范围对应回 canonical UTF-8 source。
- 正确的范围供公式原文复制、显式 delimiter conversion 和图片导出局部替换共用，避免
  中文、Emoji、连续反斜线和 CRLF 使后续节点偏移。
- 无效或越界的坐标不能用来截取任意文本；不能为了“显示像表格”而重写 canonical note。

证据：[table_math_pipes.rs](../../crates/stickymd-render/tests/table_math_pipes.rs) 的七项回归，
以及 [2026-09-08 源码范围报告](2026-09-08-table-math-source-ranges.md)。

<a id="blocks"></a>
## 5. 块结构怎样投影

| Owned 结构 | 当前 RenderTree / layout 行为 |
| --- | --- |
| Paragraph | 普通文字块；在 quote 容器中成为 Quote 块 |
| Heading | level 限于 1–6，使用该等级的字号倍率；不自动给全部文字加 strong |
| BlockQuote | 递归展开子块并增加缩进；Quote 段落绘制竖线，不是包住所有嵌套块的一整条连续边框 |
| List | 递归展开 item；增加一级缩进，并在首个结果块前插入 marker |
| Task item | checked / unchecked 分别显示 `☑` / `☐`；Preview 中不是可修改文档的复选框控件 |
| Ordered item | 按解析的 start（至少 1）和 item index 生成编号 |
| Unordered item | 统一用 `•`，不保留源文件中 `-`、`+`、`*` 的视觉差别 |
| CodeBlock | literal 文字与 code 样式；保留 info string 数据，但当前 painter 没有语言标签绘制 |
| HtmlBlock | code + HTML literal 样式，显示原文 |
| ThematicBreak | 独立 rule 装饰 |
| Table | 独立表格布局器；等宽分列，单元格可以有文字、样式、公式和图片 |
| DisplayMath | 独立公式块，水平居中，有数学背景装饰 |

列表 marker 的选择顺序是：**任务状态 → 有序编号 → 普通 bullet**。
当前列表投影还会把 item 的首个输出块种类统一设为 `ListItem`；若首块原来是 heading、
code 或 table，其块级种类会被覆盖。这是需要继续检查的投影局限，不能从 AST 已支持容器
推断所有复杂嵌套视觉都已正确。`tight` 虽保留在 owned list 中，当前没有独立间距分支。

### 表格的具体布局

- 列数取所有行最大 cell 数与 alignment 数的较大者，至少为 1。
- 扣除外边距与缩进后，将可用宽度均分给所有列，没有按内容估算列宽。
- 单元格四周 padding 为 `8 × scale`；正文使用 body metrics。
- 左/中/右 alignment 传给单元格布局；未指定时左对齐。
- 行高取该行最高 cell 内容加上下 padding，同时保留最小行高。
- header 有不同背景，但没有隐式 strong；需要粗体时由单元格内容本身产生 strong。
- 普通 body 行目前使用同一种背景，没有交替底色。
- 目前没有表格独立横向滚动状态；窄窗口通过收窄等宽列和文字换行处理。
- 原子公式或图片不能像普通文字一样拆开，过宽对象仍受 viewport 裁剪约束。

其中横向滚动与交替底色是明确的合同差异，详见第 15 节，不能把上表理解为修改了合同。

<a id="inline-style"></a>
## 6. 行内样式怎样叠加

`RenderStyle` 是一组独立标志，不是 CSS 层叠规则。递归进入节点时在 inherited style 上
设置当前属性，其余属性保留。

| 语义 | 投影属性 | 对普通文字的影响 |
| --- | --- | --- |
| Strong | `strong = true` | `Weight::BOLD` |
| Emphasis | `emphasis = true` | italic |
| Strikethrough | `strikethrough = true` | 删除线 |
| Code | `code = true` | monospace family；不解析代码中的子语法 |
| Link | `link = true` | underline，并给子 span 设置打开目标动作 |
| HtmlLiteral | `code = true` + `html_literal = true` | monospace literal |
| Math | math 对象与原文复制信息 | 进入 RaTeX 对象布局；失败时用等宽原文 |
| Image | image 对象、alt 复制信息和占位文本 | 尝试图片通路；不成功则保留占位 |

### 6.1 字体选择优先级

1. `code`、`math_placeholder`、`html_literal` 任一为真：使用 cosmic-text 的
   `Family::Monospace`。
2. 否则按文本脚本片段选择 CJK / Latin family。
3. strong、italic、underline、strikethrough 独立应用，不由选了 monospace 自动清除。
4. 无相应字体或字符时，实际字形仍受字体库与 shaping fallback 影响。

例如 link 内的 strong 文字可同时粗体和下划线；link 内的 code 文字可以同时等宽和带
链接动作。链接文字当前沿用主题前景，没有额外指定统一蓝色。

行内 code 当前没有单独的胶囊/背景矩形；block code 的背景来自块级 decoration。
标题也主要通过字号区分，不能套用浏览器默认的“标题一定粗体”。

### 6.2 对象不等同于带特殊字体的文字

公式对象传给 MathEngine 的关键输入是公式 literal、Inline/Display kind、有效字号和
主题前景。外层 Markdown strong / emphasis 不自动变成 RaTeX 的数学粗体/斜体命令；
数学字形应由公式语义决定，例如公式里的 `\mathbf`。

link 节点会给递归生成的子 span 写入同一个 `OpenLink` 动作；因此外层 link 的目标可以
覆盖子 image 自身的 remote-image 动作。样式、动作、视觉对象和复制文本是不同字段，
不能从“看起来有下划线”反推复制内容或目标。

<a id="math"></a>
## 7. 数学公式

### 7.1 识别、布局与字形来源

1. Comrak 产出 Math 节点。
2. parser 同时保存 delimiter-free `literal` 和含原始 delimiter 的 `source_literal`。
3. RenderTree 选择 Inline 或 Display kind。
4. RaTeX parser/layout 使用 Text 或 Display math style 生成 DisplayList。
5. 受控 native painter 绘制 glyph path、line、rect、path；不经过 PNG renderer。
6. 公式 raster 作为原子对象与相邻文本合成。

`source_literal` 优先从有效 source range 截取；当前代码在范围不可用时退回 Comrak 的
`math.literal`，该 fallback 不能保证含原 delimiter。已有四种 delimiter 与表格坐标用例
验证的是有效映射下的精确复制，不能把 fallback 也称为 byte-exact 原文恢复。

支持范围是锁定 RaTeX 版本实际能够解析和布局的数学语法；不是完整 TeX 引擎，不加载
宏包，不执行 `\usepackage` 或外部命令。嵌入数学字体与正文选用的 Times/CJK 字体是两条路径。
某个公式成功 rasterize 也不等于它的每个字形已获人工视觉认可。

代表性输入见 [phase6_formulas.txt](../../crates/stickymd-render/tests/fixtures/phase6_formulas.txt)
与 [rendering-stress.md](../../crates/stickymd-render/tests/fixtures/rendering-stress.md)：包含分数、
根式、上下标、求和积分、矩阵、cases、括号、字体命令、中文与错误公式等。
它们是回归语料，不是完整命令支持列表。

### 7.2 inline / display 几何

- inline math 是一块不可从中间换行的矩形；宽度不足时与相邻文字协商换到下一行。
- 混排每行使用所有对象的最大 baseline 与 descent，行高至少达到正文 metrics。
- display math 在可用块宽度内居中；过宽时居中偏移不会变成负数。
- 不为过宽公式自动修改 TeX、拆成多行或无限缩小字号；viewport 负责裁剪。
- formula 背景、error border 和复制范围不是公式源文件的一部分。

### 7.3 失败隔离

无法解析、超长、超过公式数量上限或 raster 资源不能满足时，当前公式显示原始 delimiter
文本，附错误边框、角标和可供 hover 查询的错误信息。它仍是原子复制对象。
其他 block 继续排版，不通过删除公式、自动修补源码或改写 `note.md` 来掩盖错误。

是否易读、边框/角标在不同 DPI 下是否合适、hover 的实际桌面体验仍属于人工验证。
headless 中“存在错误对象与 tooltip 数据”不能替代这些判断。

### 7.4 颜色与缓存

布局阶段用默认前景 sentinel 保持 DisplayList 与普通主题前景解耦；raster 阶段再代入主题色，
公式自身的显式颜色不应被默认前景替换。当前 layout key 为公式 source + math kind；raster
key 另含字号与前景。与 plan 中 layout key 文字描述的差异见第 15 节。

<a id="images"></a>
## 8. 图片

### 8.1 语义分类先于加载

只有 Comrak 真正识别的 Image 节点才进入图片投影。code 或 raw HTML 中看似图片的文本
不调用 Markdown 图片加载通路。alt 由节点内部的文字/code 等内容收集，换行转为空格。

| destination | 当前分类与行为 |
| --- | --- |
| `http:` / `https:`，大小写不敏感 | Remote：占位文字 + 可点击链接；不调用 HTTP 下载 |
| Windows drive 绝对路径、以斜线/反斜线开头的路径、`file:` | LocalAbsolute：交给本地 resolver |
| 无 URI scheme 的其他路径 | LocalRelative：以 canonical note 目录为基准 |
| 其他 URI scheme | Unsupported：不做本地图片解码，保留占位 |

通用占位视觉文本是 `[image: alt] destination`；空 alt 时视觉标签用 `image`，复制仍使用
原 alt（可以为空），不把占位文案或路径自动写进剪贴板。

### 8.2 本地路径与读取边界

- 相对路径以 `<program-dir>/note/` 为基准；`../` 和显式绝对路径用于读取，不授予写入或
  managed ownership。GC 的授权不是由“Preview 能显示”决定。
- `file:///C:/...` 转为 Windows 路径；`file://server/share/...` 当前会转为 UNC 路径。
- `file:` URL 分支对 percent encoding 失败返回错误；普通路径尝试解码，失败则保留字面值，
  因而普通文件名中的裸 `%` 不一定被拒绝。
- worker 的 inspect 用 seekable buffered reader 读取格式和尺寸；load 才读取有上限的
  encoded bytes。格式和尺寸验证与后续完整解码仍是不同步骤。

**已发现的边界风险：** resolver 没有拒绝 UNC，后续直接交给 Windows `File::open`。
所以“没有 HTTP client”不能证明没有通过系统文件服务访问网络共享。本轮未访问任何网络
共享；目前确认的是代码路径与路径解析用例，实际 SMB 流量未测量。详见第 15 节。

### 8.3 显示尺寸与懒加载

- 独占一个块的本地图片：最大宽度为可用内容宽度，最大高度为 `900 × scale`。
- 文字/表格混排中的图片：最大宽度为可用宽度，最大高度为当前文字行高的 4 倍。
- 宽高保持比例，缩放比不大于 1，不放大到超过源图片像素尺寸；目标尺寸至少 1 px。
- 支持 PNG、JPEG、WebP、GIF、BMP、ICO 的当前 image 解码路径；这里没有动画播放调度。
- JPEG orientation 同时影响 metadata 与 raster 的尺寸/方向。
- 不在加载带内的有效图片可保留已知尺寸的矩形占位，以维持布局位置。
- 文件丢失、损坏、格式不支持、尺寸/预算超限时退回占位，不修改图片或 note。
- 缓存按 encoded 内容哈希和目标尺寸识别；不是仅凭路径名认定图片 bytes 永远不变。

当前加载带在 viewport 上下各扩展 `max(300 × scale, viewport height)`，并非固定只扩展
300 DIP。滚动超出已有加载带时可能重做整篇布局，详见缓存章节与差异清单。

<a id="links-html"></a>
## 9. 链接与原始 HTML

### 链接动作

parser 和 Windows Shell adapter 都对目标分类。允许 http、https、mailto、file，以及
相对路径；scheme 大小写不敏感。其他有效 URI scheme（例如 `javascript:`）被拒绝。

点击经过 typed intent 与 coordinator；Shell adapter 会重新分类并要求与传入 kind 一致，
不能仅靠伪造一个 `Https` kind 绕过检查。相对路径由 note 目录解释，然后交给系统 Shell；
Windows 拒绝打开也会成为错误。参考 [shell.rs](../../apps/stickymd-win/src/platform/windows/shell.rs)。

“可点击”表示允许显式调用系统打开目标，不表示 Preview 会在后台下载目标，也不表示
链接目标内容已被 StickyMD 校验为可信文件。图片自动读取与链接显式打开需分别审查。

### raw HTML

- HtmlInline / raw literal span 按等宽文字呈现。
- HtmlBlock 走代码块式 literal 投影。
- `<script>` 不执行，`style=` 不建立样式，`<img>` 不被当成 Markdown Image 加载。
- 不使用 HTML sanitizer 构造一个可执行页面，因为不存在 HTML 渲染层。
- HTML 标记附近的普通 Markdown 是否仍识别，取决于 Comrak 产出的具体节点，不能把
  所有尖括号内容一概视为相同的 block HTML 上下文。

<a id="appearance"></a>
## 10. 字体、尺寸、颜色和绘制顺序

### 10.1 正文字体选择

[fonts.rs](../../crates/stickymd-render/src/source/fonts.rs) 中候选顺序如下：

| 类别 | 从前到后选择首个可用 family |
| --- | --- |
| CJK | `仿宋_GB2312` → `FangSong_GB2312` → `仿宋` → `FangSong` → `Microsoft YaHei` |
| Latin | `Times New Roman` → `Georgia` |
| code / HTML literal / 错误公式文字 | `Family::Monospace`；当前 Preview 没有显式绑定 Consolas |

脚本切分把 Han、Hangul、Hiragana、Katakana、Bopomofo 归为 CJK，Latin 脚本归为 Latin。
中性字符通常继承前一个明确脚本；开头中性字符使用后续第一个明确脚本，无明确脚本时
使用 Latin。Emoji 等字符最终仍依赖字体 fallback，不是这里单独指定一套 Emoji family。

没有候选 family 时仍可进入字体系统 fallback，不能据此保证所有机器字形完全相同。
固定字体 raster golden 与用户桌面的实际字体发现是不同的证据。

### 10.2 布局 token

下面是 `scale = 1` 的基础值。正常 UI 输入的 scale 结合 DPI 与 Content Zoom；布局内部
下限保护为 0.5。用户 Content Zoom 范围仍由既有配置合同限定为 50–300%。

| 项目 | 基础值 / 算法 |
| --- | --- |
| body 字号 / 行高 | 17 / 26.35 |
| code / HTML block 字号、行高 | 15.3 / 23 |
| H1 | body 字号与行高 × 1.75 |
| H2 | body 字号与行高 × 1.45 |
| H3 | body 字号与行高 × 1.25 |
| H4–H6 | body 字号与行高 × 1.1 |
| 内容外边距 | 24 |
| 块间距 | 12 |
| 每级缩进 | 16 |
| Quote 文字附加内缩 / 竖线宽 | 8 / 3 |
| 表格 cell padding | 8，四边 |
| code 背景扩展 | 文字区域左右各 7、上下各 4 |
| display math 背景扩展 | 左右各 7、上下各 3 |

上表 token 随 scale 参与布局；不能据此声称每个装饰都按同一比例缩放。当前 paint 中
边框 stroke 为 1 个物理像素、数学错误 marker 为 4×4 个物理像素。
实际行高还会受公式/图片 baseline 与 descent 影响。

### 10.3 普通文字与代码换行

没有数学/图片对象的 chunk 使用 cosmic-text advanced shaping 和 `Wrap::WordOrGlyph`。
这一路径也覆盖当前 code / HTML block，所以代码长行实际会自动换行。
混排通路把文字 pieces 与不可拆的公式/图片排入行；遇到 hard break 显式结束当前行。
没有根据字符数量线性估算 glyph 的选择位置。

SoftBreak 当前投影为一个空格；HardBreak 为换行。源文件里一个 ordinary soft newline
不等于 Preview 的强制换行，这与代码 literal 中保留的换行也不同。

### 10.4 调色板

来源：[paint.rs](../../crates/stickymd-render/src/preview/paint.rs)。数值为 sRGB RGBA；没有
列出的 alpha 均为 255。这是 Preview 内容色，不是整窗透明度设置。

| 用途 | Light | Dark |
| --- | --- | --- |
| 背景 / 普通 table cell | `(248,246,239)` | `(35,35,33)` |
| 文字 / 默认数学前景 | `(40,38,34)` | `(226,223,214)` |
| 选区 | `(176,207,243,210)` | `(68,101,142,220)` |
| quote 竖线 | `(148,142,126)` | `(130,130,124)` |
| code 背景 | `(238,235,226)` | `(48,48,45)` |
| display math 背景 | `(243,239,228)` | `(52,50,44)` |
| 分隔线 | `(180,176,164)` | `(86,85,80)` |
| table header | `(235,232,222)` | `(49,49,46)` |
| table border | `(186,181,168)` | `(87,86,80)` |
| math error | `(190,73,55)` | `(232,120,102)` |

字形显式颜色可覆盖默认文字前景；公式中的显式颜色亦通过数学路径保留。
“全部文字始终同色”并不是这张默认调色板能证明的结论。

### 10.5 绘制顺序

1. 分配 viewport 大小的 frame 并填充背景。
2. 将 scroll clamp 到当前文档可滚动范围，找到可见块和文字行。
3. 对可见块画 code、quote、table、math 等 decoration。
4. 画该块的选择矩形。
5. 画该块的文字、公式 raster、图片 raster 或矩形占位。

因此选择色在内容下面，原子图片的不透明像素可能遮住其内部背景。文字 hit-test 使用真实
shaped cluster 与 visual row locator，不靠“每个字等宽”的近似。
viewport culling 也不能等同于整篇布局已被省略。

<a id="scheduling"></a>
## 11. 刷新、任务合并与过期结果

### 11.1 什么触发工作

| 状态 / 事件 | 当前调度 |
| --- | --- |
| Preview 不可见时文本变化 | 标记 dirty，取消计划时点，不立即构建 |
| Split 中文本变化 | 从最新编辑重新计算 1000 ms debounce |
| Preview 可见且要求新内容 | 立即请求 build |
| 显示 Preview，已有同 generation 的干净投影 | 可请求 relayout / paint |
| 显示 Preview，投影缺失或 dirty | 以当前 snapshot 请求 build |
| completion generation 不是当前值 | 丢弃，不改变当前文档与 Preview 状态 |

1000 ms 是触发工作的调度参数，不是“输入后恰好 1000 ms 一定看到新图”的时延保证；还需
考虑 worker 正在执行的工作与布局/绘制耗时。旧结果未被合格新结果替代前仍可保留显示。

### 11.2 有界 mailbox 的优先规则

- 单个 Preview worker；pending 槽位有界，合并待执行工作，不无限排队。
- 更新 generation 的请求覆盖较旧的 pending 请求，旧 generation 不能反向覆盖新请求。
- 同 generation 的 Build 与后来的 viewport 更新合并时，保留要解析的 snapshot，使用
  较新的 viewport，不能只剩 Relayout 导致该 generation 从未建立语义树。
- 每个请求带完整 viewport 上下文；仅 scroll 更新也不能恢复旧 width / zoom / theme。
- release 请求清除 pending；同时存在 release 级别时 Document 高于 Rasters。
- worker 下一轮先处理 release。它不意味着可在任意 RaTeX 指令中间强行终止线程；过期
  completion 仍必须由 generation / viewport 适用性检查拦截。

这些规则节省无效排队，但不构成多 worker 并行渲染。UI 与 worker 的职责隔离也不允许
worker 直接写 `DocumentState`。

<a id="cache"></a>
## 12. 滚动、缩放与缓存

### 12.1 哪些动作会重复工作

| 动作 | Comrak parse | 当前布局 / 绘制 |
| --- | --- | --- |
| 新 snapshot build | 是 | 新 RenderTree、整篇 layout、viewport paint |
| 同 generation 调整宽度 | 否 | 复用 RenderTree，重做 layout |
| 普通滚动 / 选区变化 | 否 | 几何可复用时只 paint |
| 有本地图片且 viewport 离开加载带 | 否 | 为新图片带重做 layout 后 paint |
| Content Zoom 或主题变化 | 否 | 复用 RenderTree，当前实现重做整篇 layout |
| 清除 raster 后恢复显示 | 依剩余 document projection 而定 | raster / layout 按需重建 |
| 丢弃 document projection 后再次显示 | 是 | 从当前 snapshot build |

滚动先按当前文档高度 clamp，再计算图片带，避免 `f32::MAX` 等过量滚动使底部可见图片
仍被判断为带外。纯文本 note 不因 production 中始终存在 image adapter 就触发图片重布局。

Content Zoom 改变内容比例，不改变 source、generation 或 Markdown 语义；不建立另一个
用户排版配置系统。目前“整篇重布局”与 plan 的“只重新布局可见内容”存在差异。

### 12.2 缓存身份、容量与生命周期

| 缓存 | 当前身份 / admission | 上限 / 释放规则 |
| --- | --- | --- |
| Math layout | source + Inline/Display kind | 最多 512 entries；保留可复用 DisplayList |
| Math raster | layout 身份 + 有效字号 bits + foreground | 8 MiB；比例/主题变化需适用 raster，清理路径可释放 |
| Math glyph outline | glyph outline 数据 | 4 MiB |
| Decoded image | encoded 内容哈希 + target width/height | 16 MiB 与 512 entries 双上限 |
| Text layout（单次 layout 内） | 文字、style、width、字号、行高、align、wrap | 只缓存文字长度至多 1024 bytes 的项；首次见仅登记，重复后才缓存；有界 1024 项状态 |
| cosmic-text / Swash glyph raster | 字体系统内部有效字号等 key | 改变 scale 或清理 raster 时换掉旧 Swash cache，避免累计旧比例 |

Math / image live budget 包含当前 layout 仍通过 `Arc` 持有的 raster。仅从 map 中移除 key
不代表内存已经释放；不能据此继续 admission 而让真实 live bytes 越界。替换 layout 前
先丢弃旧 layout 的 leases，使可淘汰项真正可释放。

text shape cache 复用的是几何；每次出现的 source range、复制 offset 和 action 都重新投影，
不能因为两段文字相同就复用另一处链接目标。它在本次 layout 后结束，不是永久文档缓存。

`release_raster_caches()` 丢弃 layout、Swash、math raster 与 decoded images。
`release_document_projection()` 还丢弃 RenderTree 和图片带状态，保留字体数据库和有界
math layout 以供复用。不能把这个动作描述为“全部渲染相关内存归零”。

这些是算法和计账规则；本次没有测量其时延收益、整进程峰值或完整资源 campaign。

<a id="selection"></a>
## 13. 选择、复制与 Split 同步

### 13.1 两套坐标不要混用

- canonical source range：UTF-8 source 的字节区间，服务于源码定位、公式原文和导出替换。
- Preview display/copy range：布局后可选择文本的区间，可能去掉 Markdown 标记或加入
  渲染生成的 marker、块间换行与表格分隔符。
- visual geometry：shaped cluster、逻辑行与自动折行后的 visual row，用于鼠标命中和高亮。

多字节中文、组合字符、Emoji 和 BiDi 使三者不能用简单字符数比例互换。框选和 hit-test
使用真实 shaping 数据；长期文档布局保留行定位信息，viewport 才投影可见 cluster 几何。

### 13.2 复制内容

| 内容 | 当前复制语义 |
| --- | --- |
| 普通文字、emphasis、strong、link label | 复制显示文字；不自动附 Markdown 样式或 URL |
| code | 复制 code literal；不是 fenced marker / info string 的完整源片段 |
| SoftBreak / HardBreak | 分别复制空格 / 换行 |
| 相邻 render blocks | 用换行分隔 |
| 列表 | 投影插入的 marker 也是显示/复制文本的一部分 |
| 表格 | cell 之间 Tab，row 之间换行 |
| 公式 | 原子选择，复制含原始 delimiter 的完整 source literal |
| 图片 | 原子选择，复制 alt text；不复制图片 bytes 或自动附 destination |
| raw HTML literal | 复制所显示的 literal |

所以 Preview 的“全选复制”是只读文字投影，不是 canonical Markdown 导出。
公式 copy 的 byte-exact 原文承诺不能推广到整篇普通文字复制。

### 13.3 Split 与显式转换

Split 同步使用 source-byte anchor 与块内相对位置建立跨 Source / Preview 的定位关系，
不直接按两边总高度百分比映射。Preview generation 过期时不能把旧范围当作当前映射。
源侧与预览侧仍有各自的滚动状态，resize / zoom 不会成为文本编辑。

工具栏的数学 delimiter conversion 是另一条**显式编辑**路径：读取 Comrak 确认的公式
范围，对受支持的 `\(...\)` / `\[...\]` 做受控替换；code、未闭合片段和不相关文本
不自动转换。它通过编辑事务改变 source，与 Preview 日常渲染不会改 source 的承诺不冲突。
实现见 [semantic_conversion.rs](../../crates/stickymd-render/src/preview/semantic_conversion.rs)。

<a id="failures"></a>
## 14. 限制与失败路径

### 14.1 有界输入

| 限制 | 当前数值 | 超出后的层级 |
| --- | --- | --- |
| Preview snapshot | 5 MiB UTF-8 bytes | 整次 Preview parse 拒绝，Source 文本保留 |
| owned 转换深度 | 256 | owned conversion 拒绝 |
| owned 节点数 | 200,000 | owned conversion 拒绝 |
| 单公式 source | 64 KiB | 该公式错误占位 |
| 一篇布局中公式数量 | 2,000 | 后续超限公式错误占位 |
| 图片 encoded bytes | 64 MiB | 图片解码拒绝 / 占位 |
| 图片最大单边 | 16,384 px | 同上 |
| 图片最大像素数 | 40,000,000 | 同上 |
| 数学 / 图片缓存 | 见第 12 节 | admission / 淘汰受 live budget 约束 |

深度与节点上限在 Owned AST 转换中检查，不能表述成 Comrak 在遇到第 257 层前就一定
已经停止解析。5 MiB snapshot gate 则在创建本次 Comrak parse 前检查。
这些也不是整进程内存上限；资源 hard gates 另见 [plan 10](../plan/10_performance_reliability.md)。

### 14.2 失败不能互相冒充

| 失败 | 当前处理或证据边界 |
| --- | --- |
| 不完整 / malformed Markdown | 由 Comrak 决定 ordinary text 或可识别结构；不自行补写 source |
| 一个公式失败 | 原文 + 错误装饰，其余内容继续 |
| 一个图片失败 | 占位，其余内容继续 |
| 未允许的链接 scheme | 拒绝 Shell 动作；不能因视觉 link 样式就放行 |
| whole-document parse / conversion 失败 | 错误结果与 generation 关联；不能标为新 Preview 成功 |
| 旧 generation 的成功或错误 | 丢弃；不能覆盖当前状态 |
| layout 缺失 / generation 不匹配 | pipeline 返回 typed error，不能借旧布局返回伪成功 |
| zero-width / 无法创建 viewport frame | pipeline / paint 错误；不自动写文件或修改文本 |
| 自动化得到非空 raster | 只证明该用例可执行；不能等价为视觉正确 |

<a id="differences"></a>
## 15. 实现与合同的差异、后续路径

### 15.1 审查发现

本次没有改 runtime，也没有修改 plan 来迁就以下现状。所列事实来自当前源码；未执行的
桌面/网络行为明确保留为待验证。既有测试通过仅说明其已断言的行为，没有关闭这些差异。

| 优先性 / 类别 | 合同或预期边界 | 当前源码观察 | 影响与后续路径 |
| --- | --- | --- | --- |
| 高：读取边界风险 | plan 06/08 要求 Preview 不发起网络请求，本地 adapter 应限于 local read | `assets/path.rs` 接受 UNC / `file://server/...`；worker 直接 `File::open` | Windows 可能经 SMB 访问共享；当前未观察真实网络。后续需先为网络路径、设备路径、映射盘和 reparse 后的实际目标定义可证明的 local-only 检查，再修 resolver/adapter；不能只检查 `http` 前缀 |
| 中：代码块布局差异 | plan 06：超长行横向滚动或限宽截断 | `layout.rs` 对纯文本 code 使用 `Wrap::WordOrGlyph` | 实际自动换行；后续按合同补行为和长行/选择回归，不把换行改写为新合同 |
| 中：表格布局差异 | plan 06：宽度不足区域横向滚动，行背景轻微交替 | 等宽收窄、文字换行，无 table 横向 offset；所有 body 同色 | 宽表和行辨识与合同不同；需有几何、复制、主题及桌面验证 |
| 中：字体绑定差异 | plan 06：code block 使用 Consolas | Preview 为 `Family::Monospace`，未显式指定 Consolas | 依赖字体数据库默认选择，不能保证各机器等同；应先验证目标字体选择再修绑定 |
| 中：复杂列表投影局限 | Comrak 块结构应经 Owned → Render 投影保留适用语义 | item 首个输出块统一改成 `ListItem` | 首块为 heading/code/table 等时可能丢失块级布局信息；先补具体嵌套 fixture，再分离 marker 与原块职责 |
| 中：缩放工作范围差异 | plan 06 的 zoom 条目写“只重新布局可见内容” | `relayout_with_image_source` 调用整篇 `layout_document`，无额外 Comrak parse | 文档规模仍影响 zoom 布局工作量；未量化性能。本次只登记，与同章“全量 layout”的表述一起在后续方案中厘清 |
| 待核对：图片带范围 | plan 08 写 viewport 上下 300 DIP | 实际 margin 为 300 DIP 与一屏高度的较大者 | 扩大 admission 范围以减少重布局，但仍受 cache 上限；实现说明不能直接当作合同批准 |
| 待核对：数学 layout key | plan 06 写 source + display_mode + foreground | 当前 source + kind，默认前景 sentinel 留到 raster 阶段替换 | theme-independent layout 可复用；需结合已有 theme tests 核对合同描述，不擅自调整依赖或缓存政策 |

标题/表头没有隐式粗体、inline code 没有背景、info string 尚未画成标签是**当前表现**，
不都属于合同违约。例如 info label 在合同中是“可作”，不能据此发明强制实现任务。

### 15.2 本轮采用与未采用的路径

采用：读取合同 → 检查 parser / projection / worker / platform adapter → 复用现有输入运行
针对性回归 → 记录具体事实、覆盖和差异。没有引入另一套 Markdown parser 或用截图倒推语义。

后续若修复已明确的 implementation drift，应保持 Comrak/RaTeX 的权威、readonly Preview、
现有 source 身份和失败隔离。在动 runtime 前先增加能暴露差异的窄回归，再做必要的 Windows
实际检查。若方案需要改变网络/文件权限边界、公共行为或核心架构，应另列影响并取得授权；
本报告本身不授予这些变更。

<a id="verification"></a>
## 16. 验证结果与未验证项

### 16.1 本轮实际执行

在上述 source、Windows 工作树、锁定依赖下执行：

```powershell
cargo test -p stickymd-render --locked --test phase5_semantics --test table_math_pipes --test phase6_math --test rendering_stress
```

| Test target | 本轮结果 | 能证明的主要内容 |
| --- | --- | --- |
| `phase5_semantics` | 2 passed | Owned AST golden、LF/CRLF 等价 |
| `table_math_pipes` | 7 passed | GFM 分列、转义/UTF-8 source range、公式复制、delimiter conversion、图片局部导出 |
| `phase6_math` | 6 passed | 四种 delimiter、容器/过宽公式、baseline/居中、错误隔离、数量/长度限制、代表性公式 |
| `rendering_stress` | 6 passed | 当前语法/literal 边界、RaTeX 语料、320/900 px 与 50/100/300% 内容比例及 Light/Dark raster、滚动和代码行选择 |

共 21 passed、0 failed。真实输入位于版本化 fixtures 中；没有拿旧发布产物或收据证明本次
源码行为。这里的尺寸/比例组合是 headless raster 输入，不是 Windows 多显示器 DPI 人工验收。

文档维护另执行 `cargo run --quiet -p stickymd-smoke --locked -- dev-check`。现有选择器将
架构/验收投影视为共享输入，新增 `docs/README.md` 也按未知输入保守处理，因此执行了九项
完整 `tests` 模式任务：治理、fmt、workspace 严格 locked Clippy、locked cargo-deny、
两项 Phase 1 tests、workspace tests、locked Windows Release build、native-runtime dependency
gate，全部通过。日志保存在 ignored `target/docs-preview-20260930-203830.log`。它不启动
GUI/资源 campaign，也不写 qualification 收据；本轮没有为减少文档检查而放宽分类规则。

同 source 的 [CI run 36790130061](https://github.com/Develata/StickyMD/actions/runs/36790130061)
已完成，九个 jobs（含最终聚合）均成功：Windows tests/performance/build/lint、Linux smoke
与 portable-core、依赖政策及 plan/governance。它验证该源码的普通 CI，不是本文文档变更的
远程运行，也不是新候选的发布资格。

### 16.2 已有回归的定位入口

| 问题 | 查哪里 |
| --- | --- |
| 哪些 Comrak 扩展开启、math/code/HTML 是否混淆 | `preview/parser.rs` tests、`phase5_semantics`、`rendering_stress` |
| 文本样式、链接动作、缓存副本是否混用 | `preview/render_tree.rs`、`preview/text_layout.rs` tests |
| emoji / combining / BiDi / 自动折行的选择几何 | `preview/text_layout.rs` tests、Phase 14 selection 用例 |
| resize / scroll 是否重 parse，图片带是否正确 | `preview/pipeline.rs` 与 `preview/pipeline/audit_tests.rs` |
| 公式错误与缓存计账 | `math/engine.rs`、math cache/painter 单元测试、`phase6_math` |
| 图片格式、orientation、live cache | `image.rs`、`qualification_images.rs`、Phase 7 |
| debounce / pending 合并 / stale 丢弃 | `flow/preview.rs`、`preview/worker.rs` 与 app Preview tests |
| 自动化与人工证据分别是什么 | [Phase 05](../acceptance-cases/phase-05.md)、[Phase 06](../acceptance-cases/phase-06.md)、[Phase 07](../acceptance-cases/phase-07.md) |

这张定位表不是本轮逐条重跑全部用例的清单。具体运行结果以上一节命令和对应 CI 为准。

### 16.3 仍未验证

- 当前源码的真实 Windows 视觉、鼠标/剪贴板/Shell 交互、hover、字体缺失与多显示器 DPI 矩阵。
- 第 15 节差异的修复效果；本轮没有修复它们，也没有为这些差异建立新的通过结论。
- UNC/映射盘/reparse 的实际网络行为；未访问任何网络共享。
- 所有 CommonMark/GFM/RaTeX 输入的完全符合性；现有 fixture 不能穷尽整个语法空间。
- 本次文档维护的性能收益、完整资源/性能 Campaign、candidate / manual readiness。

人工未验收项保持原状态。维护此报告时保留日期与 source；后续修复或新证据以带日期的
Resolution 追加，不回改这次审查的输入、发现和测试结果。

<a id="review-2026-10-01"></a>
## Resolution — 2026-10-01 文档复审与勘误

本次复审 source 为 `ae2e0aa26cec6265bb2d1e594c5c21b75d97f46f`。
与上文 `be1322e` 相比，`apps/`、`crates/`、`Cargo.toml`、`Cargo.lock` 没有变化；
下述内容纠正文档或补充观察，不表示已经修复 runtime，也不改变原有验收结论。
按报告的追加规则保留原文；阅读对应章节时，应结合本节勘误。

### 1. 三处表述的纠正

**第 10.5 节：绘制顺序按层批量执行。**
[`paint_document`](../../crates/stickymd-render/src/preview/paint.rs) 先填充 frame 背景，
计算可见范围，然后依次画所有可见块的 decoration、所有选区矩形、所有可见块的内容。
原文的“该块”容易被读成逐块交错画完这三层，应以以下顺序理解：

```text
背景 → 所有可见 decoration → 所有 selection rectangles → 所有可见内容
```

文字自身的下划线和删除线由
[`text_layout/painting.rs`](../../crates/stickymd-render/src/preview/text_layout/painting.rs)
在相应文字行内绘制，不能与块 decoration 混为一层。这次通过代码路径确认顺序，
没有将它表述成已经完成重叠内容、主题和 DPI 的人工视觉验收。

**第 11.1、12.1 节：保留 RenderTree 不等于 UI 返回时免 parse。**
pipeline 在保留 RenderTree 后收到 `Relayout` 可以复用语义树；但当前普通 UI 从 Source
切回 Preview/Split 的调用链是：

```text
切入 Source
  → 清除显示 frame，Coordinator.release_projection() 清除 applied generation
  → worker.release_raster_caches() 保留语义树
重新显示 Preview/Split
  → Coordinator.show() 因没有 applied generation 选择 Build
  → 提交当前 snapshot → pipeline.build() 再次 parse
```

依据为 [`app/preview_runtime.rs`](../../apps/stickymd-win/src/app/preview_runtime.rs)、
[`flow/preview.rs`](../../apps/stickymd-win/src/flow/preview.rs) 与
[`pipeline.rs`](../../crates/stickymd-render/src/preview/pipeline.rs)。因此第 12.1 节
“清除 raster 后恢复显示”描述的是底层能力及其条件，不能推导为当前 UI 切换的免解析承诺。
同 generation 且仍保有已应用投影时的 Preview ↔ Split、普通 resize/scroll 需分别判断，
不因这条 Source 返回路径而全部改称 Build。本次没有测量切换耗时。

**第 8.2 节：严格 percent 解码仅覆盖特定 `file` 前缀。**
[`assets/path.rs`](../../apps/stickymd-win/src/assets/path.rs) 对 `file:///` 与 `file://`
做大小写不敏感的专用分支，percent 解码失败会拒绝；其他字符串进入普通路径分支，
解码失败则保留原文。parser 把 `file:` 归入 LocalAbsolute，并不意味着 resolver
完整支持所有 file URI 形式。`file:C:/...`、`file:/...` 等不能套用专用分支的结论。
这条纠正没有消除上文记录的 UNC 读取风险；本次没有访问网络共享。

### 2. 对两项实现差异补充实测

**列表内首块为表格时，内容会在 Render 投影中丢失。** 最小输入是：

```markdown
- | a | b |
  | --- | --- |
  | x | y |
```

将该字符串送入 `PreviewParser.parse`，Owned AST 保留完整的 List → Table，含表头
`a`、`b` 和内容 `x`、`y`。随后经 `RenderTreeBuilder.build`，首块被替换为 `ListItem`，
普通 spans 只剩 `"• "`；经 `PreviewPipeline.build` 后，
`frame.copy_selection(frame.select_all())` 也只返回 `Some("• ")`。
因此第 15.1 节的“可能丢失块级布局信息”在这个输入上已经具体表现为表格内容没有进入
Preview 内容与全选复制结果，而不只是表格外观变化。原始 snapshot 没有被修改。

根因入口是 [`render_tree.rs`](../../crates/stickymd-render/src/preview/render_tree.rs)
的列表投影。后续修复应保留表格块职责，另行承载列表 marker，并以这个输入建立窄回归；
不能把 GFM 语义从合同中删去以迁就现状。本次仅记录诊断，不引入 runtime 修复。

**当前等宽字体确实不能由 `Family::Monospace` 推断为 Consolas。**
锁定的 cosmic-text 0.19.0 在 `FontSystem::new_with_fonts` 中将 generic monospace 名称
设为 `Noto Sans Mono`；仓库没有再将它绑定为 Consolas。使用当前字体系统对 ASCII
`plain code 0123456789` 做 `Family::Monospace` shaping，本机观察为：

```text
monospace-default=Noto Sans Mono
monospace-sample-faces={"CascadiaCode-Roman"}
```

前者是 generic family 配置，后者是该字符串实际使用的 face，二者应分开理解。
这验证了第 10.1、15.1 节所列差异；它不保证另一台 Windows、另一组字符或另一套已安装
字体也选择 Cascadia Code。后续应按合同明确 code font 绑定并验证 fallback，
本次没有安装字体或修改字体政策。

### 3. 复审验证与边界

诊断使用当前源码和锁定依赖构建，临时文件位于 ignored
`target/docs-review-20261001/`：

```powershell
cargo build -p stickymd-render --locked --offline --message-format=json-render-diagnostics
```

从本次成功构建的 `compiler-artifact` JSON 取 `stickymd_core`、`stickymd_render`、
`cosmic_text` 的准确 rlib，再用 `rustc --edition=2024` 的 `--extern` 和
`-L dependency=target/debug/deps` 编译 `probe.rs`，运行结果记录于 `probe.log`。
没有按修改时间猜测旧 rlib，也没有运行旧 Release EXE。

| 合成输入 | 本次观察 |
| --- | --- |
| 独占 `$$x$$` | DisplayMath 块、Display math kind；复制保留 delimiter |
| `before $$x$$ after` | 段落中的 math 保留 `display=true`，投影采用 Inline math kind；复制保留完整文本 |
| 上述列表内表格 | Owned AST 表格完整，Preview 全选复制只剩列表 marker |
| `before ![ALT](https://example.invalid/image.png) after` | Remote 分类和 Https 动作，复制为 `before ALT after` |
| ``[`x`](https://example.invalid)`` | code 与 link 样式同时存在，保留 OpenLink 动作，复制为 `x` |

这些是五组输入的诊断观察，**不是五项新增通过的回归测试**。诊断未提供图片读取 adapter，
未调用 Shell 或启动 GUI；Remote 的对象分类和动作不等于实际点击已验收。
字体探针同样只覆盖本机合成字符串。

本轮另实际执行以下既有回归与文档检查：

```powershell
cargo test -p stickymd-win --locked flow::preview::tests
cargo test -p stickymd-win --locked assets::path::tests
cargo run --quiet -p stickymd-smoke --locked -- phase 00
cargo fmt --all -- --check
git diff --check
```

两组 targeted tests 分别为 8 passed 和 2 passed，治理、fmt、diff 检查通过。
文档扫描覆盖 `docs/` 全树及根 README、CONTRIBUTING、CLI README，共 189 份文档、
468 个本地 Markdown 链接及其锚点，没有发现失效项；代码示例中的伪路径排除在外。
另逐行比对了 963 条阶段验收状态数据行，全部保持原样，并确认本报告原有正文只追加、未回改。
链接扫描不验证外部 URL 可达性；语义复核重点是现行合同、验收身份、Preview 调用链与 CLI
说明，没有逐一重新执行所有历史报告中的实验。

本轮同时复核了文档链接、验收矩阵的证据身份与发布示例；Phase 05/07/08 的旧措辞
“receipt checked in”已在对应投影中纠正。正式 exact-candidate 动态收据按 plan 11
写入 ignored `dist/evidence/`，不要求为了更新当前发布 verdict 而回填历史 Markdown。
原有人工状态、版本身份、技术 `NOT_READY` 和 USER 发布特例均未升级。

本轮仍未验证完整 Markdown/RaTeX conformance、真实桌面视觉、网络共享行为、资源性能
或新的候选资格。前述 21 项回归与九项 `dev-check` 结果属于 2026-09-30 的执行记录，
不得当作本次重新运行的数量。
