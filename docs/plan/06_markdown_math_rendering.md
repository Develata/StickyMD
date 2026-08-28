# 06_markdown_math_rendering.md - Markdown / 数学 / 预览合同

## Metadata

- `Layer`: Capability
- `Status`: Approved Contract
- `Version`: 0.1.0
- `Last Review`: 2026-08-20
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
  BiDi 或不连续视觉范围允许产生多个矩形。复制只按最终 byte range 读取完整 document projection，
  不能从蓝框、glyph 或 source AST 重新拼文本。
- 公式和图片是 atomic selectable object：选择公式时绘制公式矩形，Ctrl+C 复制原始数学源码与
  delimiter；选择图片时复制 alt text，不复制 bitmap。它们不得伪造成普通等宽文本 cluster。
- `PreviewFrame` 只向 App 暴露 `hit_test`、`selection_rects`、`copy_selection`、`action_at`、
  `tooltip_at` 等语义方法；App 不读取 cluster 数组，不依赖 Cosmic Text 类型。

生命周期与缓存：

- geometry cache key 至少包含 `generation + viewport width/height + scroll + content scale`；影响
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
  超长行横向滚动或限宽截断，不得影响窗口布局。
- 表格：GFM alignment、单元格换行、基本边框、行背景轻微交替、宽度不足区域横向滚动；
  不支持列宽拖动与单元格编辑；task checkbox 只读。
- MathLayoutCache：source + display_mode + foreground → DisplayList；最多 512 entries。
- MathRasterCache：layout key + effective font size（已折入 DPI 与 Content Zoom）+ theme foreground → raster；
  严格预算 ≤ 8 MiB。
- painter 的复用 glyph outline 采用独立 ≤ 4 MiB bounded cache；不使用上游无界 outline cache。
- DecodedImageCache：只缓存 viewport 附近图片；≤ 16 MiB；LRU 淘汰。
- 单次文档 layout 内允许复用等价文字 shaping；只在第二次出现后 admission，最多 1024 个、
  单 key 文本最多 1024 bytes，layout 完成即释放索引，不跨 generation 保存历史 Preview。
- 进入纯 Source 或隐藏（tray/dock collapsed）一段时间后：清理解码图片与公式 raster，
  保留小型 layout cache、文档与字体数据库，不保留无必要 framebuffer 副本。
- Content Zoom 改变时复用已解析的 OwnedDocumentTree/RenderTree，只重新布局可见内容；
  不重新运行 Comrak，也不推进 Document generation。数学只失效受有效字号影响的 raster，
  保留语义/layout authority；图片按缩放后的布局框请求受 16 MiB 预算约束的 viewport raster，
  不预生成 300% 全文大图。

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
