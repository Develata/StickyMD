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

## 数学权威：RaTeX

### 支持范围

> RaTeX / KaTeX-compatible 数学语法。

它**不是** TeX Live、完整 LaTeX 文档系统、宏包管理器、`\usepackage` 环境、
任意 TeX 执行器或 LaTeX 编译器。

### 引擎与字体

- 使用 RaTeX 的 parser / layout / types / font crates；纯 Rust，无 JS/DOM/WebView。
- 数学字体使用 RaTeX 配套的 KaTeX-compatible 字体（OFL 1.1），不强制 Cambria Math。
- Release 必须包含第三方字体声明与 OFL 文本。

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

---

## Preview 调度与 Generation

```text
分栏已打开：停止输入 → 1000 ms debounce → 后台解析最新文本 → 构建预览
             → generation 仍为最新才提交（连续输入重置计时器）
切换纯 Preview：立即刷新；可暂显旧预览；新结果原子替换；过期结果丢弃
```

Preview 只读，但必须支持：鼠标选择文字、Ctrl+C、滚动、点击允许的链接、
公式错误提示、本地图片显示。

### Preview 文本选择

- 每个文字 run 保存 source range + 显示文本 + glyph rects。
- 选择公式：视觉选中公式矩形；Ctrl+C 复制其原始数学源码与 delimiter。
- 选择图片：复制 alt text（不把 bitmap 复制到剪贴板，除非未来单独设计）。

---

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

## 渲染与缓存

- 代码块：Consolas，不做语法高亮；fenced info string 可作顶部小标签；
  超长行横向滚动或限宽截断，不得影响窗口布局。
- 表格：GFM alignment、单元格换行、基本边框、行背景轻微交替、宽度不足区域横向滚动；
  不支持列宽拖动与单元格编辑；task checkbox 只读。
- MathLayoutCache：source + display_mode → DisplayList。
- MathRasterCache：display_list_hash + font_size + dpi + theme → raster；预算 ≤ 8 MiB。
- DecodedImageCache：只缓存 viewport 附近图片；≤ 16 MiB；LRU 淘汰。
- 隐藏（tray/dock collapsed）一段时间后：清理解码图片与公式 raster，
  保留小型 layout cache、文档与字体数据库，不保留无必要 framebuffer 副本。

---

## Inputs

`doc::snapshot`（带 generation）、主题与 DPI 上下文。

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

Preview debounce 1000 ms 为固定内部参数；无用户可配排版项（排版 token 固定）。

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
