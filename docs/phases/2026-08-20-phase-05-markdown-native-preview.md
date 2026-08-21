# StickyMD Phase 5 — Markdown Semantic Pipeline, Owned AST & Native Preview Foundation

你现在位于 StickyMD 本地 Git 仓库根目录。

Phase 0 已完成工程治理与架构合同。

Phase 1 已完成技术基础与高风险 Spike。

Phase 2 已完成 canonical `DocumentState`、`TextDelta`、Generation、Undo/Redo 与 Snapshot。

Phase 3 已完成 Native Source Editor、IME、Interaction Pipeline。

Phase 4 已完成 Portable Persistence、Autosave、Recovery、External Reconciliation 与 Conflict。

**只有 USER 已明确批准 Phase 4 进入 Phase 5 时，才允许执行本 Prompt。**

---

# 0. Phase 5 名称与核心目标

本阶段名称：

> **Phase 5 — Markdown Semantic Pipeline, Owned AST & Native Preview Foundation**

本阶段第一次正式建立：

```text
DocumentSnapshot
      │
      ▼
Comrak Parser Adapter
      │
      ▼
Transient Comrak Arena AST
      │
      ▼
StickyMD OwnedDocumentTree
      │
      ▼
RenderTree
      │
      ▼
Native Preview Layout
      │
      ▼
tiny-skia / cosmic-text
      │
      ▼
Preview Surface
```

本阶段必须证明：

1. Markdown 语义由 Comrak 完整承载。
2. Comrak 的 Arena AST 不泄漏为 StickyMD 长期状态。
3. StickyMD 拥有自己的稳定 parser-independent AST。
4. Preview 是 `DocumentState snapshot` 的派生投影。
5. Source / Preview / Split 三种模式正式成立。
6. Split Preview 使用 1000 ms debounce。
7. Preview-only 切换时立即刷新。
8. Preview 构建不阻塞 UI thread。
9. stale generation 永远不能覆盖新的 Preview。
10. Raw HTML 永远不执行。
11. remote image 永远不触发网络。
12. 数学 delimiter 必须正确识别，但正式 RaTeX 绘制留 Phase 6。
13. 图片语义必须正确进入 AST，但正式图片 decode/render 和 managed asset lifecycle 留后续阶段。

---

# 1. 本阶段明确不做什么

Phase 5 **禁止正式实现**：

```text
RaTeX final painter
完整数学公式 rasterization
数学字体 cache 最终实现

图片 decode
图片 clipboard
managed image GC
.trash
asset undo side effect
image cache
Export

Tray final lifecycle
Dock
Auto-hide
Hover reveal
Multi-monitor docking

最终 Theme selector
最终 Opacity selector

语法高亮
Mermaid
PlantUML
HTML renderer
CSS
DOM
JavaScript

PDF export
HTML export
plugin system
network client
```

---

# 2. Phase 5 完成后的用户可见状态

Phase 5 development build 至少应该：

### Source

继续使用 Phase 3/4 的源码编辑器。

### Preview

能够正确原生显示：

```text
paragraph
heading
bold
italic
strikethrough
blockquote
unordered list
ordered list
nested list
task list
inline code
fenced code
link
autolink
horizontal rule
table
hard break
soft break
raw HTML literal
math placeholder
image placeholder
```

### Split

```text
┌────────────────────┬────────────────────┐
│ Source             │ Preview            │
│                    │                    │
│ Markdown           │ rendered Markdown  │
└────────────────────┴────────────────────┘
```

固定：

```text
50 / 50
```

不可拖分隔线。

---

# 3. 数学在 Phase 5 的行为

以下必须正确识别：

```text
$...$
$$...$$
\(...\)
\[...\]
```

但是 Phase 5 暂时：

```text
不调用正式 RaTeX renderer
```

而是生成：

```text
InlineMathPlaceholder
DisplayMathPlaceholder
```

并显示原始数学内容的轻量占位。

例如：

```markdown
$E=mc^2$
```

Preview 可以暂时显示成视觉上可区分的：

```text
⟦ E=mc² source placeholder ⟧
```

但：

**不要擅自解释公式内容。**

更推荐：

```text
$E=mc^2$
```

以专用 math placeholder style 显示原始源码。

---

# 4. 图片在 Phase 5 的行为

Markdown：

```markdown
![diagram](images/a.png)
```

必须进入：

```text
OwnedImage
```

但不 decode。

Preview 显示：

```text
[image: diagram]
images/a.png
```

或类似极简 placeholder。

Remote：

```markdown
![remote](https://example.com/a.png)
```

必须：

```text
不请求网络
不 decode
```

只显示：

```text
[remote image: remote]
https://example.com/a.png
```

---

# 5. 开始前必须读取

严格执行根 `AGENTS.md`。

至少读取：

```text
AGENTS.md
docs/AGENTS.md
docs/plan/AGENTS.md

docs/plan/00_engineering_constitution.md
docs/plan/01_terminology.md
docs/plan/02_positioning_and_scope.md
docs/plan/03_system_architecture.md
docs/plan/04_runtime_state_model.md
docs/plan/05_document_persistence.md
docs/plan/06_markdown_math_rendering.md
docs/plan/07_editor_and_ime.md
docs/plan/09_windows_shell.md
docs/plan/10_performance_reliability.md
docs/plan/11_testing_and_release.md

docs/features/00_v1_product_behavior.md
docs/acceptance-cases/00_v1_acceptance.md
docs/coverage-matrix.md

docs/report/phase-01-technical-spike-report.md
docs/report/phase-02-core-document-model.md
docs/report/phase-03-source-editor-ime.md
docs/report/phase-04-portable-persistence.md
```

以及 Phase 1 中：

```text
experiments/phase-01/markdown-math/*
```

如果还存在。

---

# 6. Phase 4 前置 Gate

必须从 Phase 4 report 得到：

```text
APPROVE Phase 5
```

或：

```text
APPROVE Phase 5 WITH CONDITIONS
```

且条件已由 USER 接受。

如果：

```text
STOP — architecture review required
```

立即停止。

---

# 7. 仓库开始状态

执行：

```bash
git status --short
git branch --show-current
git log -10 --oneline

cargo metadata --no-deps
cargo tree -p stickymd-core
cargo tree -p stickymd-render
cargo tree -p stickymd-win
```

记录：

```text
branch
starting commit
clean / dirty
```

不得：

```text
reset
clean
rebase
覆盖 USER 修改
```

---

# 8. Phase 5 在四层架构中的位置

本阶段路径：

```text
Interaction Shell
      │
      ▼
SetViewMode Intent
      │
      ▼
PreviewCoordinator
      │
      ▼
DocumentSnapshot
      │
      ▼
Markdown Execution Capability
      │
      ▼
Preview Projection
      │
      ▼
Interaction Shell render
```

---

# 9. Authority 模型必须保持

必须始终成立：

```text
DocumentState
=
canonical runtime text authority
```

而：

```text
Comrak AST
OwnedDocumentTree
RenderTree
LaidOutDocument
cosmic-text preview buffers
```

全部只是：

```text
derived projection
```

---

# 10. Preview 绝不能反写 Document

禁止：

```text
click Preview
→ modify Markdown
```

禁止：

```text
task checkbox Preview
→ toggle source
```

Phase 5 Preview 是严格只读。

---

# 11. Parse Source

唯一 parse 输入：

```text
DocumentSnapshot
```

不得从：

```text
SourceProjection
cosmic-text Source Buffer
disk note.md
```

直接 parse。

---

# 12. Preview generation

Preview 必须绑定：

```text
Document Generation
```

例如：

```rust
struct PreviewRevision {
    generation: Generation,
}
```

---

# 13. Preview invariant

任何 Preview：

```text
preview.generation <= document.generation
```

只有：

```text
preview.generation == document.generation
```

才称为：

```text
Clean Preview
```

---

# 14. Dirty Preview

如果：

```text
document generation > preview generation
```

Preview：

```text
Dirty
```

但旧 Preview 可以继续显示。

---

# 15. 不要编辑时清空 Preview

用户持续输入：

旧 Preview 保留。

不要：

```text
每 keypress
→ blank Preview
```

---

# 16. Comrak dependency baseline

当前技术基线：

```text
Comrak 0.54.x
```

正式修改 Cargo 前必须重新：

```bash
cargo search comrak
```

或：

```text
检查 crates.io / upstream
```

确认当前 stable。

---

# 17. 不擅自升级 major/minor 设计语义

如果本地 Phase 1 已锁：

```text
0.54.x
```

优先延续已验证版本。

---

# 18. Comrak Features 是 Phase 5 Hard Gate

当前 Comrak 默认 features 会带入：

```text
CLI
syntect
Oniguruma backend
```

StickyMD 不需要这些。

必须优先配置：

```toml
comrak = {
    version = "...",
    default-features = false
}
```

具体语法按实际 Cargo。

---

# 19. 禁止 Comrak CLI feature

不得启用：

```text
cli
```

---

# 20. 禁止 syntect

不得启用：

```text
syntect
syntect-onig
syntect-fancy
```

Phase 5 不做语法高亮。

---

# 21. Cargo Tree Gate

执行：

```bash
cargo tree -p stickymd-render
```

确认没有：

```text
syntect
onig
onig_sys
clap
xdg
```

由于 Comrak 引入。

如出现：

先修 features。

---

# 22. Comrak 只属于 render crate

依赖应进入：

```text
stickymd-render
```

不得进入：

```text
stickymd-core
```

---

# 23. Core Dependency Gate

最终：

```bash
cargo tree -p stickymd-core
```

不得出现：

```text
comrak
```

---

# 24. Parser Adapter

建立内部：

```text
markdown/comrak_adapter.rs
```

或等价模块。

职责只有：

```text
DocumentSnapshot
→ Comrak AST
→ OwnedDocumentTree
```

---

# 25. Comrak 类型泄漏禁止

Comrak 类型不得出现在：

```text
public stickymd-render API
AppState
PreviewState
stickymd-core
Interaction Shell
```

---

# 26. 为什么需要 Owned AST

Owned AST 是稳定边界：

```text
Comrak
可以升级 / 替换

StickyMD Owned AST
保持稳定
```

---

# 27. 不创建 Parser trait

目前只有一个 Markdown parser：

```text
Comrak
```

不要提前造：

```rust
trait MarkdownParser
```

除非有两个真实实现。

模块边界本身已经提供替换点。

---

# 28. Comrak Arena 生命周期

解析函数中：

```text
Arena create
→ parse_document
→ traverse
→ build OwnedDocumentTree
→ Arena drop
```

---

# 29. Arena 不允许存入 PreviewState

禁止：

```text
struct PreviewState {
    arena: Arena<...>
}
```

---

# 30. Arena 不跨线程结果边界

Worker 内可以使用。

返回：

```text
OwnedDocumentTree
```

或后续派生结果。

---

# 31. OwnedDocumentTree

建议：

```rust
pub(crate) struct OwnedDocument {
    blocks: Vec<BlockNode>,
}
```

---

# 32. BlockNode 最低集合

至少：

```rust
enum BlockNode {
    Paragraph(...),

    Heading {
        level: HeadingLevel,
        children: Vec<InlineNode>,
    },

    BlockQuote {
        children: Vec<BlockNode>,
    },

    List(ListNode),

    CodeBlock(CodeBlockNode),

    Table(TableNode),

    ThematicBreak,

    HtmlLiteral(HtmlLiteralNode),

    DisplayMath(MathNode),
}
```

根据 Comrak AST 可调整。

---

# 33. InlineNode 最低集合

至少：

```rust
enum InlineNode {
    Text(...),

    Emphasis(...),

    Strong(...),

    Strikethrough(...),

    Code(...),

    Link(LinkNode),

    Image(ImageNode),

    InlineMath(MathNode),

    SoftBreak,

    HardBreak,

    HtmlLiteral(...),
}
```

---

# 34. 不复制 parser implementation details

Owned AST 不应有：

```text
ComrakNodeValue
AstNode<'a>
typed_arena
```

---

# 35. SourceRange

所有有意义节点尽量带：

```rust
struct SourceRange {
    start: usize,
    end: usize,
}
```

使用：

```text
UTF-8 byte offsets
```

与 DocumentState 一致。

---

# 36. Source Range 用途

未来支持：

```text
Preview selection
link source
math source copy
debug
error reporting
```

---

# 37. Comrak Sourcepos

不要假设 Comrak source position 与 StickyMD byte range 完全相同。

必须先验证：

```text
line semantics
column semantics
end inclusive/exclusive
UTF-8 columns
```

---

# 38. SourceMap

建立：

```rust
SourceMap
```

用于：

```text
line + parser column
→ absolute canonical UTF-8 byte offset
```

---

# 39. Runtime 文本已归一换行

Phase 4 后：

```text
DocumentState text
=
UTF-8 + \n
```

因此 SourceMap 只针对 canonical runtime text。

---

# 40. SourceMap 测试

至少：

```text
ASCII
中文
emoji
combining
multiple lines
empty line
line ending
```

---

# 41. Source Range Fail Conservative

如果 parser sourcepos 无法可靠转换：

不要产生错误 range。

允许：

```text
SourceRange::Unknown
```

或：

```text
Option<SourceRange>
```

比猜错更好。

---

# 42. Owned Text Storage

不要每个 Text node 都无脑复制独立 String，如果明显浪费。

可选择：

```text
Arc<str> source
+
Range<usize>
```

表示纯文本 slice。

---

# 43. 但 parser-transformed text 需要 owned

例如：

- entity decode。
- normalized code content。

可以 owned。

---

# 44. 不为零拷贝污染设计

如果 `Arc<str> + range` 导致代码极度复杂：

允许 owned `String`。

必须：

```text
measure
```

再决定。

---

# 45. Phase 5 初始策略

推荐：

```text
DocumentSnapshot.text = Arc<str>

Owned textual nodes:
source-backed when exact slice is valid
owned only when transformation occurred
```

---

# 46. Markdown Extensions

必须明确启用：

```text
strikethrough
table
autolink
tasklist
math_dollars
math_latex
```

根据 Comrak 实际 API。

---

# 47. 不启用额外 Extensions

不要因为 Comrak 支持就开启：

```text
footnotes
wikilinks
spoiler
highlight
underline
alerts
subtext
description lists
emoji shortcodes
header attributes
```

除非当前 plan 明确已经批准。

---

# 48. CommonMark/GFM 语义

StickyMD 的目标：

```text
CommonMark 0.31.2
+ GFM core extensions
+ approved math extensions
```

---

# 49. Raw HTML

Comrak 识别：

```text
HtmlInline
HtmlBlock
```

或对应实际类型。

Owned 转成：

```text
HtmlLiteral
```

---

# 50. Raw HTML 不执行

Preview：

```text
literal
```

不得：

```text
parse DOM
interpret style
```

---

# 51. Script Tag

例如：

```html
<script>alert(1)</script>
```

Preview：

显示原始文字。

不得执行。

---

# 52. iframe

同样 literal。

---

# 53. Raw HTML Styling

Inline HTML：

```text
inline code-like
```

Block HTML：

```text
code block-like
```

---

# 54. HTML 测试

至少：

```text
<b>hello</b>
<div>block</div>
<script>alert(1)</script>
<style>...</style>
<iframe ...>
```

---

# 55. Links

Owned link：

```rust
struct LinkNode {
    destination: String,
    title: Option<String>,
    children: Vec<InlineNode>,
    source_range: Option<SourceRange>,
}
```

---

# 56. Preview 链接基本行为

Phase 5 至少必须：

```text
视觉上可识别
hit-test 可识别为 Link
```

是否本阶段正式 Shell 打开：

建议实现。

---

# 57. Link 安全 scheme

允许：

```text
http
https
mailto
file
```

以及：

```text
relative local path
```

---

# 58. 自定义 scheme

例如：

```text
vscode:
steam:
javascript:
```

不得执行。

---

# 59. javascript scheme

必须明确禁止：

```text
javascript:
```

---

# 60. URL parser

不要手写复杂 URL parser。

如果需要 scheme parsing：

优先成熟：

```text
url crate
```

---

# 61. url dependency

新增前：

dependency audit。

如果 current dependency tree 已有：

复用。

---

# 62. Link Activation Port

Interaction Shell 不直接：

```text
ShellExecute
```

推荐：

```rust
trait OpenTargetPort
```

或已有 Windows shell adapter。

---

# 63. Link open 是 Interaction effect

Preview hit-test：

```text
click
→ OpenLink intent
→ coordinator
→ validate scheme
→ shell adapter
```

---

# 64. Local relative link base

相对：

```markdown
[foo](other.md)
```

以：

```text
./note/
```

为基准。

---

# 65. 不构建文件浏览器

点击：

```text
交给 Windows Shell
```

---

# 66. 不读取 link target 进 StickyMD

---

# 67. Image Node

建议：

```rust
struct ImageNode {
    destination: String,
    title: Option<String>,
    alt: String,
    source_range: Option<SourceRange>,
}
```

---

# 68. Alt Text

需要从 image child inline nodes 收集可读文字。

不能假设只有 Text。

---

# 69. Phase 5 Image Classification

至少区分：

```text
LocalRelative
LocalAbsolute
RemoteHttp
RemoteHttps
OtherUnsupported
```

---

# 70. 但不 decode

不得引入：

```text
image crate
```

仅为了 placeholder。

---

# 71. 不检查 local 文件是否存在，除非必要

Preview semantic layer 不需要每次 parse hit filesystem。

可在 placeholder UI 上显示路径。

---

# 72. Remote Image hard invariant

Phase 5 Cargo tree 不应出现 HTTP client。

---

# 73. Math Node

Owned math：

```rust
struct MathNode {
    kind: Inline | Display,
    source: MathSource,
    source_range: Option<SourceRange>,
}
```

---

# 74. MathSource

必须明确保存：

```text
inner math source
```

并尽可能保存：

```text
raw source / delimiter info
```

未来复制时需要。

---

# 75. 不要自己 regex 识别数学

只使用：

```text
Comrak math node
```

---

# 76. 数学 delimiter 测试

必须：

```text
$x$
$$x$$
\(x\)
\[x\]
```

---

# 77. 数学边界测试

至少：

```text
\$5
$5
code `$x$`
fenced code with $$x$$
escaped delimiters
multiline display math
```

结果完全服从 Comrak。

---

# 78. Phase 5 不调用 RaTeX parser

即使 Phase 1 已验证。

目的：

先证明 Markdown pipeline 独立。

---

# 79. RenderTree

Owned AST 是 Markdown 语义。

RenderTree 是 StickyMD 视觉语义。

两者必须分开。

---

# 80. 为什么不能 Owned AST 直接 paint

否则：

```text
Markdown parser semantics
=
layout semantics
```

耦合过深。

---

# 81. RenderTree 建议

```rust
struct RenderDocument {
    blocks: Vec<RenderBlock>,
}
```

---

# 82. RenderBlock

至少：

```rust
enum RenderBlock {
    Paragraph(RenderParagraph),
    Heading(RenderHeading),
    Quote(RenderQuote),
    List(RenderList),
    CodeBlock(RenderCodeBlock),
    Table(RenderTable),
    Rule,
    HtmlLiteral(RenderLiteral),
    DisplayMath(RenderMathPlaceholder),
}
```

---

# 83. Inline Render

可以：

```rust
struct StyledSpan {
    content: TextContent,
    style: InlineStyle,
    source_range: Option<SourceRange>,
    interaction: Option<InlineInteraction>,
}
```

---

# 84. InlineStyle

不要和 CSS 模型一样无限扩张。

只需：

```text
weight
italic
strikethrough
code
link
```

加 font role。

---

# 85. FontRole

建议：

```text
Body
Code
MathPlaceholder
```

---

# 86. 中文/Latin Font Rules 继续沿用

Body：

```text
CJK → 仿宋_GB2312
Latin → Times New Roman
```

Code：

```text
Consolas
```

---

# 87. Bold

正文 bold：

使用当前 script 对应字体 family 的 bold face。

---

# 88. Italic

Latin：

Times New Roman Italic。

中文字体如果没有真正 italic：

使用 cosmic-text 合理 fallback / synthetic behavior。

不要自行做 shear transform，除非现有 text stack 正确支持。

---

# 89. Bold Italic

组合正确。

---

# 90. Heading

Heading 1–6。

不要建立复杂 CSS-like type scale。

固定 style token。

例如：

```text
H1 1.75em
H2 1.45em
H3 1.25em
H4 1.12em
H5 1.05em
H6 1.00em + emphasis
```

实际按 plan。

---

# 91. Heading Font Family

仍遵循：

```text
CJK FangSong
Latin Times New Roman
```

不要偷偷换 Segoe UI。

---

# 92. Paragraph

正文：

```text
17 DIP
line height ≈ 1.55
```

如果 plan 已固定。

---

# 93. Preview Padding

约：

```text
22–28 DIP
```

保持固定。

---

# 94. Block Spacing

用集中 StyleTokens。

不要散落 magic numbers。

---

# 95. PreviewStyle

建议：

```rust
struct PreviewStyle {
    body_size: f32,
    line_height: f32,
    block_gap: f32,
    ...
}
```

crate-private。

---

# 96. 不做 User Style Config

---

# 97. Code Inline

字体：

```text
Consolas
```

背景：

轻微灰块。

---

# 98. Code Block

字体：

```text
Consolas
```

不做 syntax highlighting。

---

# 99. Code Block 原文

必须保留：

```text
spaces
tabs
line breaks
```

---

# 100. Code Block Info String

例如：

```markdown
```rust
```
```

可以显示一个极小：

```text
rust
```

标签。

不据此做语法高亮。

---

# 101. Code Block 超长行

遵循已有 plan。

如果 plan 没完全固定：

Phase 5 优先：

```text
soft wrap
```

以避免 nested horizontal scroller。

在 report 明确。

最终行为后续可细化而不影响 AST。

---

# 102. Lists

必须支持：

```text
unordered
ordered
nested
```

---

# 103. Ordered Start

例如：

```markdown
5. item
```

必须显示从 5 开始。

---

# 104. List Marker

UI 自己绘制：

```text
•
1.
2.
```

不要把 marker 变成 source text span。

---

# 105. Nested List

缩进必须按 nesting level。

---

# 106. Task List

识别：

```markdown
- [ ] todo
- [x] done
```

Preview：

```text
☐ todo
☑ done
```

或绘制 checkbox。

---

# 107. Task List 只读

点击 checkbox：

```text
不得修改 Markdown
```

---

# 108. Blockquote

绘制：

```text
left rule
indent
```

不引入不同字体。

---

# 109. Nested Blockquote

支持基本 nesting。

---

# 110. Thematic Break

简单 1 DIP 线。

---

# 111. Hard Break

必须产生实际 line break。

---

# 112. Soft Break

遵循 CommonMark/Preview 视觉策略。

通常可以：

```text
space / line wrap
```

不得全部变硬换行，除非 plan指定。

---

# 113. Table

GFM table 必须进入 Owned AST 和 RenderTree。

---

# 114. Table 数据模型

```rust
struct TableNode {
    alignments: Vec<TableAlignment>,
    rows: Vec<TableRow>,
}
```

---

# 115. Table Cell

每 cell：

```text
inline content
```

不能假设纯 string。

---

# 116. Table Alignment

支持：

```text
left
center
right
none
```

---

# 117. Table Phase 5 Layout

目标：

> 正确、稳定，不追求浏览器级表格算法。

推荐：

1. 测量各 column intrinsic width。
2. 设置合理 min width。
3. 若总宽度 ≤ viewport，按 intrinsic/flex 分配。
4. 若超过 viewport，优先在 cell 内 wrap。
5. 不建立 nested horizontal scroll widget，除非 plan 已明确要求。

---

# 118. Table 不可导致整个 Preview 爆宽

必须 clip / wrap。

---

# 119. 超长单词

可以 overflow clip 或 hard-break according to text engine。

不得 OOM。

---

# 120. Preview Layout 与 Paint 分离

必须有：

```text
semantic RenderTree
→ layout
→ paint
```

---

# 121. LaidOutDocument

建议：

```rust
struct LaidOutDocument {
    generation: Generation,
    blocks: Vec<LaidOutBlock>,
    content_height: f32,
    ...
}
```

---

# 122. LaidOutBlock

至少有：

```text
y
height
bounding rect
paint items / text layout refs
interactions
```

---

# 123. Layout 不依赖 winit Window

输入：

```text
viewport width
scale factor / DPI
style
font resources
RenderTree
```

---

# 124. stickymd-render 仍跨平台

不得：

```rust
use windows::
use winit::platform::windows::
```

---

# 125. Paint Output

沿用 Phase 3：

```text
tiny-skia Pixmap
```

最终由 app：

```text
softbuffer present
```

---

# 126. Preview Worker

正式建立 dedicated：

```text
Preview Worker
```

---

# 127. 不用 Tokio

使用：

```text
std::thread
```

与现有 bounded message infrastructure。

---

# 128. Worker thread 数量

```text
1
```

不是 thread pool。

---

# 129. Worker Lazy Spawn

推荐：

```text
第一次进入 Preview / Split
```

时才启动。

Source-only 启动不需要 Preview worker stack。

---

# 130. Worker Stack

建议：

```text
512 KiB
```

如果实际 Comrak recursion / layout 需要更大：

测量后调整。

---

# 131. Preview Job

建议：

```rust
struct PreviewJob {
    snapshot: DocumentSnapshot,
    viewport_width_dip: f32,
    scale_factor: f64,
    style_revision: u64,
}
```

---

# 132. Preview Result

```rust
struct PreviewResult {
    generation: Generation,
    layout: LaidOutDocument,
    diagnostics: PreviewDiagnostics,
}
```

---

# 133. Worker 不持有 DocumentState

---

# 134. Job Coalescing

不要每个 generation 入 queue。

目标：

```text
1 in-flight
+
1 logical latest desired generation
```

---

# 135. Split debounce

Split mode：

每 canonical edit：

```text
preview_dirty = true
deadline = now + 1000ms
```

---

# 136. 连续输入

不断 reset deadline。

---

# 137. Deadline 时才 snapshot

禁止：

```text
每 keypress
→ DocumentSnapshot clone
```

---

# 138. Preview-only 切换

进入：

```text
Preview
```

立即：

```text
snapshot latest
→ request preview
```

---

# 139. Preview-only 有旧缓存

可以立即显示旧 Preview。

同时显示极轻：

```text
refreshing
```

可选。

---

# 140. 不需要 spinner 动画

避免 permanent redraw。

---

# 141. 第一次 Preview

如果没有旧 Preview：

显示：

```text
Loading preview…
```

静态。

---

# 142. stale result

Worker 返回：

```text
generation = 10
```

current：

```text
12
```

则：

```text
drop
```

---

# 143. stale result 永不 commit

hard invariant。

---

# 144. Worker panic

不得带崩整个 app。

建议：

- worker main body避免 panic。
- parse/layout errors typed。

如果 thread 意外退出：

```text
PreviewUnavailable
```

Source Editor 继续工作。

---

# 145. Preview failure 不影响 Document

---

# 146. Comrak parse 基本不应失败

但 conversion/layout 仍需要 typed errors。

---

# 147. PreviewError

至少：

```text
SourceRangeMapping
UnsupportedInternalNode
Layout
WorkerUnavailable
ResourceLimit
```

---

# 148. Unknown Comrak Node

如果 Comrak 新版出现未知 node：

不要 panic。

可以：

```text
UnsupportedLiteral
```

或 typed fallback。

---

# 149. 但不要静默丢用户文字

如果无法识别：

尽可能显示原 source literal。

---

# 150. Preview Resource Guard

Phase 5 建立基础限制。

建议：

```text
MAX_PREVIEW_SOURCE = 5 MiB
```

超过：

Preview 不自动 parse。

显示：

```text
文档较大，当前预览已暂停。
```

---

# 151. 这个限制不是 Markdown 文件限制

Source editing / persistence 仍可工作。

---

# 152. 防止 pathological nesting

Comrak 自身处理。

但 conversion 使用：

```text
recursive traversal
```

时要注意超深 nesting。

---

# 153. Owned Conversion 不应 stack overflow

优先：

```text
iterative traversal
```

或设明确深度 guard。

---

# 154. Depth Guard

建议：

```text
MAX_AST_DEPTH = 256
```

超限：

Preview error / literal fallback。

不得影响 source。

---

# 155. Node Count Guard

建议工程安全：

```text
MAX_AST_NODES = 200_000
```

超过：

停止 Preview build。

---

# 156. 不修改 Markdown

所有 resource guard：

只影响 Preview。

---

# 157. Preview ViewMode

正式枚举应已存在或建立：

```rust
enum ViewMode {
    Source,
    Preview,
    Split,
}
```

---

# 158. Config Integration

Phase 4 config已有：

```text
view_mode
```

Phase 5 正式启用。

改变模式：

```text
update ConfigState
→ config persistence
```

---

# 159. ViewMode 不改变 Document generation

必须测试。

---

# 160. View Mode Intent

例如：

```text
SetViewMode(Source)
SetViewMode(Preview)
SetViewMode(Split)
```

Shell 不直接切业务 state。

---

# 161. Source Mode

```text
Preview worker 可以 idle
```

不自动 parse。

---

# 162. Preview Mode

Source editor不显示。

DocumentState仍存在。

---

# 163. Split Mode

Source 与 Preview 同时显示。

---

# 164. Split divider

固定：

```text
1 DIP
```

不可拖。

---

# 165. Split 比例

```text
50/50
```

不得保存 pane width。

---

# 166. Split minimum

每 pane 目标：

```text
>= 240 DIP
```

如果窗口太窄：

可以使用最小 window width clamp。

不要引入复杂 auto-expand monitor logic，除非 plan已经批准。

---

# 167. Split Source 继续编辑

正常。

---

# 168. Split Preview debounce 1000ms

hard requirement。

---

# 169. Preview Mode immediate refresh

hard requirement。

---

# 170. Source selection state

切到 Preview：

保留 Source：

```text
selection
scroll
preferred_x
```

切回来恢复。

---

# 171. Preview scroll 独立

建立：

```text
PreviewSession.scroll_y
```

---

# 172. 不做 Source ↔ Preview scroll sync

明确禁止 Phase 5。

---

# 173. Preview scroll 不改变 generation

---

# 174. Preview painting viewport culling

不能每次 scroll repaint/layout所有 block。

---

# 175. Layout 一次后

Scroll 只：

```text
find visible block range
paint visible + margin
```

---

# 176. Culling margin

例如：

```text
100–200 DIP
```

---

# 177. Block Y Index

建立：

```text
sorted block bounds
```

可以 binary search visible start。

---

# 178. 不需要复杂 spatial tree

Vec + binary search 足够。

---

# 179. Window Resize

Preview width变化：

必须 relayout。

---

# 180. Resize 不重新 Parse

hard optimization：

```text
same RenderTree
→ new layout
```

除非当前实现边界暂时无法。

必须尽量做到。

---

# 181. 为什么 parse/layout 分离

Resize 是 presentation change。

不应重新执行 Markdown parser。

---

# 182. RenderTree cache

当前 generation：

```text
RenderTree
```

可以保留。

Resize：

```text
layout only
```

---

# 183. Theme 尚未 final

Phase 5 使用当前 Light dev style。

如果 Phase 4 config已有 Light/System/Dark：

不要实现完整三态 UI。

---

# 184. Dark Preview

如果 Phase 3/4 已存在 theme infrastructure：

可以保证 Preview style token支持 Light/Dark。

但不要求最终 theme selector。

---

# 185. Preview Selection

Phase 5 是否必须正式实现？

**本 Prompt 规定：基础 Preview text selection 必须实现。**

原因：

Preview 是只读阅读视图，复制能力属于基本可用性。

---

# 186. 但 Selection 范围受控

Phase 5 必须支持：

```text
normal text
heading
quote
list text
code
table text
raw HTML literal
```

---

# 187. Math placeholder selection

选中 math placeholder：

Copy：

```text
原始数学 source
```

建议包括原 delimiter 或完整 source range。

---

# 188. Image placeholder selection

可以：

```text
copy alt text
```

不复制 bitmap。

---

# 189. Preview Selection Authority

Preview selection：

```text
presentation state
```

不进入 DocumentState。

---

# 190. PreviewTextIndex

推荐建立：

```rust
struct PreviewTextIndex
```

映射：

```text
visual/selectable run
→ source range
→ display text
```

---

# 191. 不使用一个新的 canonical preview String

可以建立：

```text
derived display text index
```

但必须明确非 authority。

---

# 192. Preview Copy

复制内容来源：

```text
LaidOutDocument / PreviewTextIndex
```

允许，因为 Preview Copy 复制的是：

> rendered representation

不是用于保存。

---

# 193. Preview Copy 不能写回 Document

---

# 194. Preview Selection Across Blocks

必须至少支持普通连续 block：

```text
paragraph → heading → paragraph
```

选择。

---

# 195. Copy line breaks

跨 block：

用：

```text
\n
```

合理重建 rendered text。

---

# 196. Code Block Copy

保留 code 原始内部文本。

---

# 197. Link Copy

复制 visible label。

---

# 198. Preview Ctrl+C

如果 Preview 有 selection：

复制 Preview selection。

如果没有：

无操作。

---

# 199. Preview Ctrl+A

选择整个可复制 Preview 文本。

---

# 200. Preview 不支持 Cut/Paste

```text
Ctrl+X
Ctrl+V
```

不得修改内容。

---

# 201. Preview Mouse Hit Test

至少支持：

```text
text selection
link click
```

---

# 202. Link click 与 selection 冲突

Mouse down + drag：

selection。

click release without meaningful drag：

如果在 link：

open link。

---

# 203. Links 与 remote image

Remote image placeholder的 URL：

可以作为 link。

---

# 204. No Network Check

运行 Preview remote fixture。

使用：

```text
netstat / firewall / dependency inspection
```

至少确认程序没有主动 HTTP。

更重要：

Cargo tree无 HTTP client。

---

# 205. Preview FontSystem

必须调查 Phase 3 Source `FontSystem` 是否可以与 Preview Worker安全共享。

不要猜。

---

# 206. 优先级

1. 正确性。
2. UI thread responsiveness。
3. 内存。
4. 架构简洁。

---

# 207. 方案 A

如果 `FontSystem` 可以安全共享且不会造成 contention：

可以共享受控 font resources。

---

# 208. 方案 B

Preview worker 自己持有长生命周期 `FontSystem`。

优点：

```text
无锁
layout完全后台
```

缺点：

```text
字体数据库/cache可能重复
```

---

# 209. 方案 B 必须测内存

如果：

```text
Preview first open
```

导致显著 >12 MiB 额外稳定内存：

必须分析。

---

# 210. 不得每个 Preview Job new FontSystem

这是 hard forbidden。

---

# 211. FontSystem lifetime

至少：

```text
per preview worker
```

长期复用。

---

# 212. Source FontSystem 与 Preview FontSystem

如果双份：

必须在 report 明确：

```text
why
memory cost
future optimization options
```

---

# 213. 不要为了共享引入全局 Mutex FontSystem

除非实测证明合理。

---

# 214. Preview Layout Thread

理想：

```text
parse
owned conversion
render tree
layout
```

全部 worker。

---

# 215. Paint

UI thread：

```text
visible blocks
→ tiny-skia
```

---

# 216. 如果 cosmic-text layout object 不能跨线程

这是技术事实风险。

不要通过 unsafe 强行 Send。

---

# 217. 绝不 unsafe impl Send

禁止：

```rust
unsafe impl Send for ...
```

---

# 218. 如果 layout result无法跨线程

选择：

1. worker做 parse + RenderTree；
2. UI分帧/预算式 layout；
3. 或建立自己的 Send-safe flattened glyph layout。

必须比较。

---

# 219. 若需要架构调整

如果 Comrak/RenderTree可以后台，但 cosmic layout必须UI：

这不自动是骨架变更。

只要：

```text
UI 不被长时间阻塞
```

即可调整 execution boundary。

---

# 220. UI Layout Budget

如果 layout在UI分批：

每 frame预算：

```text
≤4 ms
```

并 viewport-first。

---

# 221. 但不要一开始做复杂 incremental scheduler

首先验证技术事实。

---

# 222. Phase 5 风险报告条件

如果：

```text
native preview full layout无法在目标性能/内存范围内成立
```

创建：

```text
docs/report/phase-05-preview-layout-risk.md
```

---

# 223. No WebView fallback

无论 Preview多难：

不得：

```text
WebView2
```

---

# 224. Parser Error Handling

Comrak本身通常不会 parse error。

但是 conversion必须：

```text
Result
```

不得 panic。

---

# 225. RenderTree Error Handling

同样 typed。

---

# 226. Layout Error Handling

单 block失败：

优先：

```text
render literal fallback
```

不要丢整个 Preview，除非全局 resource failure。

---

# 227. Block-level fallback

例如 table layout异常：

显示：

```text
[Preview layout error]
```

+ 原始 source literal。

---

# 228. Preview Diagnostics

内部记录：

```text
unsupported nodes
layout fallbacks
resource guard
```

不记录用户全文。

---

# 229. Privacy

日志不得记录：

```text
full Markdown
link text
math source
raw HTML body
```

可记录：

```text
node type
length
generation
source range
```

---

# 230. Comrak Dependency Report

创建/更新：

```text
docs/report/phase-05-dependency-delta.md
```

至少：

```text
Comrak version
license
default features
selected features
transitive dependency count
binary impact
memory impact
```

---

# 231. 必须明确记录

```text
default-features = false
```

是否成功。

---

# 232. Comrak Binary Size Delta

测：

```text
Phase 4 release exe
Phase 5 release exe
```

---

# 233. Preview Memory

测：

```text
Source only
Preview first open
Preview stable
Split stable
```

---

# 234. Memory baseline fixtures

至少：

```text
20 KiB
100 KiB
1 MiB
```

---

# 235. 初始硬门槛

沿总体规格：

### Preview 20 KiB

```text
Private Working Set ≤ 52 MiB
```

### Split 20 KiB

```text
≤64 MiB
```

作为 engineering hard gate。

如果 Phase 3/4 baseline环境不同：

报告 raw delta。

---

# 236. 更重要的是 delta

记录：

```text
Source stable
→ first Preview
```

增量。

---

# 237. 预览关闭后的内存

切回 Source。

可以保留 semantic cache。

但不应保留多个旧 generation tree。

---

# 238. Old generation释放

新 Preview commit：

旧：

```text
Owned AST / RenderTree / Layout
```

必须释放。

---

# 239. 不维护 Preview history

---

# 240. Preview Cache

Phase 5 可以只有：

```text
latest RenderTree
latest Layout
```

---

# 241. 不需要 LRU AST Cache

---

# 242. Resize Cache

可以保留 RenderTree。

---

# 243. Parse Performance

Release。

fixtures：

```text
20 KiB
100 KiB
1 MiB
```

分阶段记录：

```text
Comrak parse
Arena→Owned
Owned→RenderTree
Layout
Total
```

---

# 244. 目标

### 20 KiB

```text
total ≤100 ms hard
target ≤50 ms
```

### 100 KiB

```text
≤400 ms hard
target ≤200 ms
```

### 1 MiB

```text
≤2 s hard background
target ≤1 s
```

---

# 245. UI responsiveness

即使 1 MiB Preview 2 秒：

Source typing必须不卡。

---

# 246. Test while Preview building

Split：

启动 1 MiB parse。

同时 Source输入。

输入 latency不应显著爆炸。

---

# 247. Worker CPU

Preview parse是短暂 CPU spike可接受。

---

# 248. Idle CPU

Preview稳定后：

```text
<0.1%
```

---

# 249. Preview Scroll Performance

已有 layout：

60秒滚动测试。

不应 reparse。

---

# 250. Instrument Counters

Debug/test：

```text
parse_count
render_tree_build_count
layout_count
paint_count
stale_drop_count
```

---

# 251. Resize Test

Resize 100次。

Expected：

```text
parse_count unchanged
layout_count increments
```

---

# 252. Scroll Test

Scroll 1000 events。

Expected：

```text
parse_count unchanged
layout_count unchanged
paint only
```

---

# 253. Edit Split Test

100快速 edits <1s。

Expected：

```text
not 100 parses
```

---

# 254. Debounce Test

例如：

```text
edit t=0
edit t=300
edit t=700
```

不得 parse。

到：

```text
t=1700
```

才请求一次 latest Preview。

---

# 255. Preview Switch Immediate Test

Source dirty。

点击 Preview。

Expected：

```text
no 1000ms wait
```

---

# 256. stale generation test

```text
gen10 preview request
gen11 edit
gen10 result
```

Expected：

```text
drop
```

---

# 257. stale result不得改变 preview generation

---

# 258. stale diagnostics也不能覆盖 latest

---

# 259. Failure + stale

gen10 failed但当前gen12：

不要显示 stale error banner。

---

# 260. View Mode Persistence

切：

```text
Source → Split
```

Phase 4 config应记录：

```text
split
```

下次启动恢复。

---

# 261. 但 Startup Preview

如果 config mode：

```text
Preview
```

启动完成后：

立即请求 preview。

---

# 262. Startup Source

不创建 Preview job。

---

# 263. Startup Split

Source出现。

Preview job遵循：

```text
initial immediate render
```

不要启动后等1000ms。

---

# 264. Source editor untouched

Phase 5改动不能破坏：

```text
IME
undo
autosave
conflict
```

---

# 265. External Reload + Preview

Phase 4 clean external reload：

Document generation更新。

如果当前：

```text
Preview
```

立即刷新。

如果：

```text
Split
```

外部 reload不是打字，所以建议立即刷新。

不要等1000ms。

---

# 266. Conflict + Preview

Conflict期间 DocumentState仍 local authority。

Preview显示：

```text
local DocumentState
```

不能突然显示external。

---

# 267. Load External

解决 conflict：

DocumentState reload。

Preview立即重新构建。

---

# 268. Keep Local

Document text不变。

Preview不必重parse，除非 generation/metadata变化需要。

---

# 269. Recovery startup

恢复后Preview根据恢复后的 DocumentState。

---

# 270. Preview SourceRange after external reload

从新snapshot重建。

---

# 271. Preview Interaction State stale

新Preview commit时：

旧 link hitboxes/selection必须清理或 remap。

---

# 272. Selection after Preview refresh

最简单安全行为：

```text
clear preview selection
```

---

# 273. Preview Scroll after refresh

可以保留：

```text
scroll_y clamped
```

---

# 274. 不需要 source anchor sync

---

# 275. CommonMark Fixtures

建立：

```text
tests/markdown/commonmark/
```

不必复制完整官方 spec文本进repo，除非许可证和体积合理。

---

# 276. 最低固定 Fixture

至少：

```text
paragraphs.md
headings.md
emphasis.md
lists.md
blockquote.md
code.md
links.md
tables.md
tasklists.md
breaks.md
html.md
math.md
images.md
mixed-cjk.md
malformed.md
```

---

# 277. Golden semantic tests

对于 Markdown fixture：

输出：

```text
OwnedDocumentTree debug snapshot
```

进行 golden比较。

---

# 278. 不对 Comrak 内部 debug format 做 snapshot

只 snapshot自己的 Owned AST。

---

# 279. Golden format稳定

使用项目自己的 compact serializer/debug printer。

不必加入 serde。

---

# 280. AST test 目的

保证 Comrak升级时：

```text
StickyMD semantic projection
```

不意外改变。

---

# 281. Comrak upgrade gate

未来升级 Comrak：

所有 Owned AST golden必须 review。

---

# 282. Rendering Golden

Phase 5 可以建立少量 image golden：

```text
paragraph
mixed CJK
heading
list
table
html literal
math placeholder
```

---

# 283. Golden DPI

至少：

```text
100%
150%
200%
```

125可由手工。

---

# 284. Font-dependent golden risk

Windows系统字体实际版本可能导致像素差异。

不要做极严格 full-image byte compare。

---

# 285. 可以：

```text
layout geometry golden
```

比 pixel golden更稳定。

---

# 286. Layout geometry test

验证：

```text
block count
y positions tolerance
height
line count
font role
interaction bounds
```

---

# 287. Parser Robustness

生成随机 Markdown-like input。

固定 seed。

至少：

```text
10,000 cases
```

小字符串。

保证：

```text
no panic
```

---

# 288. 不需要在 Phase 5引入proptest

可使用 deterministic generator。

---

# 289. Deep Nest Test

构造：

```text
300 levels blockquote/list
```

确保：

```text
guard triggers
no stack overflow
```

---

# 290. Huge Node Test

生成大量小 nodes。

确认 node limit。

---

# 291. Raw HTML Security Tests

特别：

```text
<script>
<img onerror=...>
<a href="javascript:...">
<iframe>
```

Preview全部 literal/blocked。

---

# 292. Link Scheme Tests

```text
https → allowed
http → allowed
mailto → allowed
file → allowed
relative → allowed
javascript → blocked
data → blocked
custom → blocked
```

---

# 293. Remote Image Test

必须检查：

```text
https image
```

没有打开链接之前不会访问。

---

# 294. Image Destination malicious

例如：

```text
javascript:
data:
```

placeholder only。

---

# 295. Table Stress

100×20 table。

不得卡死或巨大 allocation。

---

# 296. Code Stress

10,000 chars single code line。

layout必须有界。

---

# 297. Math Stress

2,000 math nodes placeholder。

不调用 RaTeX。

---

# 298. Source Range Unicode Test

Markdown：

```text
中文 **粗体** 🙂
```

每 node byte range正确。

---

# 299. Source Range Roundtrip

对有source range节点：

```text
&source[start..end]
```

必须合法UTF-8。

---

# 300. Raw Source Preservation

Math / HTML / code需要原source时：

测试roundtrip。

---

# 301. Preview Selection Tests

至少：

```text
single span
across style boundary
across CJK/Latin
across paragraph
code block
link label
math placeholder
HTML literal
```

---

# 302. Preview Copy Test

使用 mock clipboard。

---

# 303. Preview Link Hit Test

测：

```text
inside link
outside link
selection drag
```

---

# 304. Preview Read-only Test

模拟：

```text
Backspace
Delete
typing
Paste
```

Preview focus下：

Document generation不得变化。

---

# 305. View Mode Test

```text
Source typing → generation changes
Preview typing → no change
Split Source typing → changes
Split Preview click → no change
```

---

# 306. Split Focus

点击Source：

keyboard给Source。

点击Preview：

keyboard selection/copy给Preview。

---

# 307. IME in Split

Source pane继续支持IME。

Preview pane不启用IME。

---

# 308. Candidate Position

Split Source caret仍正确。

Phase 3 IME不能回归。

---

# 309. Preview Scroll Wheel

鼠标位于Preview：

scroll Preview。

位于Source：

scroll Source。

---

# 310. 不共享 Scroll

---

# 311. Divider Input

divider不是draggable。

Cursor不显示resize。

---

# 312. Preview Cursor

link：

```text
hand
```

text：

```text
text selection cursor
```

其它：

default。

---

# 313. Clipboard Ownership

Source Copy：

canonical source。

Preview Copy：

derived rendered selection。

二者逻辑分开。

---

# 314. Phase 5 模块结构建议

`stickymd-render`：

```text
src/
├─ lib.rs
├─ markdown/
│  ├─ mod.rs
│  ├─ comrak_adapter.rs
│  ├─ source_map.rs
│  ├─ owned_ast.rs
│  └─ convert.rs
├─ preview/
│  ├─ mod.rs
│  ├─ render_tree.rs
│  ├─ style.rs
│  ├─ layout.rs
│  ├─ block.rs
│  ├─ inline.rs
│  ├─ table.rs
│  ├─ hit_test.rs
│  ├─ selection.rs
│  └─ paint.rs
```

根据cohesion调整。

---

# 315. App modules

建议：

```text
apps/stickymd-win/src/
├─ flow/
│  ├─ preview.rs
│  └─ view_mode.rs
├─ preview/
│  ├─ worker.rs
│  └─ session.rs
```

不要机械照搬。

---

# 316. File Size Review

仍遵循：

```text
~250 lines soft warning
~500 handwritten hard review
```

---

# 317. plan_ref

每个正式module：

```rust
//! plan_ref: docs/plan/06_markdown_math_rendering.md#...
```

或相关稳定anchor。

---

# 318. stickymd-render unsafe

必须仍：

```rust
#![forbid(unsafe_code)]
```

---

# 319. 不允许 Win32进入render

---

# 320. Markdown parser thread safety

不要共享 Comrak Arena。

每次 job内部局部创建。

---

# 321. Comrak Options

建立一个函数：

```rust
fn sticky_markdown_options() -> Options
```

或实际类型。

---

# 322. 不在多个地方配置 Comrak

只有一个canonical parser options factory。

---

# 323. Parser Version Contract

测试可以assert关键extension启用。

---

# 324. Markdown Options 不从 config 暴露

用户不能关闭表格/数学等。

---

# 325. Comrak parser option 是产品语义

不可用户自定义。

---

# 326. GFM Task list readonly

再确认。

---

# 327. Preview background

复用当前 paper背景。

---

# 328. Preview 不引入最终 toolbar

只加入最小ViewMode控制。

---

# 329. ViewMode UI

Phase 5至少需要可以操作：

```text
Source
Split
Preview
```

可以使用三个小图标/按钮。

---

# 330. 最终视觉以后细化

本阶段按钮样式不必 final。

但不能丑到影响测试。

---

# 331. Mode button不是Markdown功能

Interaction Shell。

---

# 332. Config save

ViewMode变更后：

使用Phase4 config persistence。

---

# 333. 不每次hover写config

---

# 334. Performance instrumentation

不要让release常驻昂贵instrumentation。

可：

```text
cfg(debug_assertions)
```

或test feature。

---

# 335. Parse Timing

worker记录：

```text
parse_ms
owned_ms
render_tree_ms
layout_ms
total_ms
```

不记录content。

---

# 336. Phase 5 dependency delta

至少可能：

```text
comrak
url
```

如果新增。

---

# 337. 禁止不必要依赖

不添加：

```text
syntect
tree-sitter
pulldown-cmark
html5ever
scraper
cssparser
image
ratex
```

RaTeX如果已经Phase1 dependency存在于experiment不等于production加入。

---

# 338. 如果 RaTeX 已在 workspace dependency

Phase 5 production render crate不得主动引用。

---

# 339. Cargo Feature

如果未来需要：

```text
math
```

不要提前feature-gate复杂化。

---

# 340. Binary Size

记录：

```text
Phase4 exe
Phase5 exe
delta
```

---

# 341. Comrak默认features误配置检测

Cargo tree若出现：

```text
onig_sys
```

Phase5不能通过。

---

# 342. No Web Engine Scan

```bash
rg \
  "webview|wry|tauri|cef|chromium|html5ever" \
  Cargo.toml apps crates
```

人工排除文档文字。

---

# 343. No Syntax Highlight Scan

```bash
cargo tree | rg "syntect|onig"
```

预期无输出。

---

# 344. No Network Scan

```bash
cargo tree | rg "reqwest|hyper|ureq|curl"
```

预期无HTTP client。

---

# 345. Preview worker leak test

切：

```text
Source ↔ Preview
```

100次。

不得创建100线程。

---

# 346. Worker count

始终：

```text
0 or 1 preview worker
```

---

# 347. Worker lifetime

一旦创建可以保持到app退出。

不需反复spawn。

---

# 348. Source-only startup memory

如果本次启动 never打开Preview：

不加载 Preview math/image resources。

Comrak crate代码在binary里无妨。

Runtime不应创建 large preview state。

---

# 349. Preview first-open latency

记录：

```text
worker spawn
font initialization
first parse
first layout
```

---

# 350. 首次和后续区别

报告：

```text
cold preview
warm preview
```

---

# 351. Font init成本

单独记录。

---

# 352. Paint Culling Test

1000段文章。

只绘viewport附近。

instrument：

```text
painted blocks / total blocks
```

---

# 353. Scroll to bottom

不得重新parse。

---

# 354. Preview viewport resize

只layout。

---

# 355. Resize debounce

连续Window resize可能每event relayout昂贵。

可以：

```text
短debounce 50–100ms
```

或者低成本 progressive relayout。

---

# 356. 不要让resize parsing

hard invariant。

---

# 357. Split Preview resize

Source和Preview各自relayout。

Document不变。

---

# 358. Error UI

Preview整体失败：

显示：

```text
预览暂时不可用
```

并继续保留Source。

---

# 359. Preview Error不弹modal

---

# 360. Preview Error重试

下一次：

```text
mode switch
document change
resize if layout failure
```

可再次尝试。

---

# 361. Math placeholder error不可能由RaTeX

因为未parse数学。

---

# 362. Raw HTML label

不要显示“危险HTML”吓用户。

只是literal style。

---

# 363. Link title

Hover可以显示title/target。

不必复杂tooltip framework。

---

# 364. Tooltip

Phase5可以不实现。

---

# 365. Preview Accessibility

不做Windows UIA完整支持Phase5。

不是gate。

---

# 366. Text Contrast

Light style应清晰。

---

# 367. Dark mode如果已有

确保不出现黑字黑底。

---

# 368. Phase 5 Task

创建：

```text
docs/tasks/phase-05-markdown-native-preview.md
```

结构至少：

```text
Status
Prerequisites
Inherited Conditions
Scope
Out of Scope
Authority
Comrak Configuration
Owned AST
Source Mapping
RenderTree
Layout
Preview Worker
View Modes
Selection
Security
Performance Gates
Acceptance
Risks
Result
```

开始：

```text
Status: In Progress
```

结束：

```text
Status: Completed — awaiting USER review
```

---

# 369. Phase 5 Report

创建：

```text
docs/report/phase-05-markdown-native-preview.md
```

必须包含：

# Phase 5 Markdown Native Preview Report

## Executive Result

```text
Comrak Integration:
PASS / CONDITIONAL / FAIL

Minimal Dependency Configuration:
PASS / FAIL

Owned AST:
PASS / CONDITIONAL / FAIL

Source Range Mapping:
PASS / CONDITIONAL / FAIL

RenderTree:
PASS / CONDITIONAL / FAIL

Native Layout:
PASS / CONDITIONAL / FAIL

Preview Worker:
PASS / CONDITIONAL / FAIL

Source Mode:
PASS / FAIL

Preview Mode:
PASS / CONDITIONAL / FAIL

Split Mode:
PASS / CONDITIONAL / FAIL

Raw HTML Safety:
PASS / FAIL

Remote Image No-Network:
PASS / FAIL

Math Delimiter Semantics:
PASS / FAIL

Preview Selection:
PASS / CONDITIONAL / FAIL

Performance:
PASS / CONDITIONAL / FAIL

Memory:
PASS / CONDITIONAL / FAIL
```

---

# 370. Comrak Evidence

表：

```text
version
default features?
enabled features
disabled heavy features
license
MSRV
```

---

# 371. Cargo Tree Evidence

明确：

```text
syntect: absent
onig_sys: absent
CLI: absent
```

---

# 372. Semantic Coverage

表：

| Markdown | Owned AST | RenderTree | Preview |
|---|---|---|---|
| paragraph | | | |
| heading | | | |
| emphasis | | | |
| strong | | | |
| strike | | | |
| quote | | | |
| list | | | |
| task | | | |
| inline code | | | |
| code block | | | |
| link | | | |
| image | | | placeholder |
| table | | | |
| HTML | | literal | literal |
| inline math | | placeholder | placeholder |
| display math | | placeholder | placeholder |

---

# 373. Source Range Evidence

至少：

```text
ASCII
CJK
emoji
mixed
```

---

# 374. Security Evidence

明确：

```text
raw HTML execution = none
JavaScript = none
DOM = none
remote image HTTP = none
custom URI execution = none
```

---

# 375. Preview Scheduling Evidence

记录：

```text
100 rapid edits
parse count
stale drop count
```

---

# 376. Resize Evidence

```text
resize count
parse count
layout count
```

---

# 377. Performance 表

| Size | Parse | Owned | RenderTree | Layout | Total |
|---:|---:|---:|---:|---:|---:|
| 20 KiB | | | | | |
| 100 KiB | | | | | |
| 1 MiB | | | | | |

至少：

```text
median
p95
max
```

---

# 378. Cold/Warm Preview

单独。

---

# 379. Memory 表

```text
Source
Preview 20K
Split 20K
Preview 100K
Preview 1M
```

---

# 380. Binary Size

Phase4 vs Phase5。

---

# 381. Idle CPU

Source、Preview、Split。

---

# 382. Dependencies Added

完整表。

---

# 383. Unsafe

必须：

```text
core = 0
render = 0
```

---

# 384. Windows APIs Added

如果 Link opening新增：

列出。

如果无：

```text
None.
```

---

# 385. Architecture Authority

回答：

```text
Canonical text owner:
Parse input:
Comrak AST lifetime:
Owned AST owner:
RenderTree authority:
Preview authority:
Can Preview write source?:
Can raw HTML execute?:
Can remote image fetch?:
```

---

# 386. Architecture Drift

如果：

```text
None.
```

明确。

---

# 387. Risks

如果：

- sourcepos不可靠。
- cosmic layout无法后台。
- Preview内存超标。
- table layout过于昂贵。

分别report。

---

# 388. Phase 6 推荐

如果Phase5通过：

Phase6推荐：

> **RaTeX Math Integration & Formula Rendering**

内容：

```text
RaTeX parser/layout
DisplayList bridge
KaTeX fonts
inline/display formula layout
formula cache
formula copy
error rendering
```

不自动执行。

---

# 389. Acceptance Cases

本阶段应实质覆盖/推进：

```text
AC-013 Markdown Preview
AC-014 Math Delimiters   # semantics only, visual formula Phase6 final
AC-016 Raw HTML Safety
AC-017 Remote Image No Network
```

---

# 390. AC-013

应达到：

```text
核心 Markdown 均有 native Preview
```

---

# 391. AC-014 状态必须诚实

Phase5：

```text
Delimiter recognition PASS
Formula rendering PENDING Phase6
```

不能把AC-014整体写Completed。

---

# 392. AC-016

应完整通过。

---

# 393. AC-017

应完整通过。

---

# 394. Coverage Matrix

更新真实code paths。

---

# 395. Overview Architecture

加入：

```text
DocumentSnapshot
↓
Comrak Adapter
↓
Owned AST
↓
RenderTree
↓
Preview Worker/Layout
↓
Preview Surface
```

---

# 396. Plan 修改

只允许：

- Comrak已验证API事实。
- source range约束。
- preview worker实际边界。
- technical corrections。

---

# 397. 不因为实现方便改变产品语义

---

# 398. Review Subagents

如果支持，最多3个。

### Reviewer 1

```text
Comrak semantics + source ranges + owned AST
```

### Reviewer 2

```text
RenderTree/layout + preview generation + concurrency
```

### Reviewer 3

```text
security + memory + dependency + architecture authority
```

---

# 399. Self Review

最终必须逐项回答：

1. Comrak是否仅在render层？
2. Comrak type是否泄漏？
3. Arena是否每次parse后释放？
4. Owned AST是否parser-independent？
5. Preview是否可能反写Document？
6. Preview parse是否只用DocumentSnapshot？
7. 每次编辑是否没有立即全文snapshot？
8. Split是否真正1000ms debounce？
9. Preview-only是否立即刷新？
10. stale result是否绝对丢弃？
11. resize是否没有reparse？
12. scroll是否没有reparse/layout？
13. raw HTML是否完全literal？
14. remote image是否完全零网络？
15. math delimiter是否只由Comrak决定？
16. 是否错误提前调用RaTeX？
17. Comrak默认features是否关闭？
18. 是否出现syntect/onig？
19. 是否出现WebView/HTML renderer？
20. Preview是否有第二canonical text authority？
21. Source IME是否回归？
22. persistence/autosave是否回归？
23. Preview worker是否无界创建？
24. old preview generation是否释放？
25. 20KiB Preview内存是否达标？

---

# 400. 自动化验证

最终至少：

```bash
cargo fmt --check

cargo clippy \
  --workspace \
  --all-targets \
  -- -D warnings

cargo test \
  --workspace \
  --locked

cargo build \
  --workspace \
  --release \
  --locked

cargo test \
  -p stickymd-core \
  --release \
  --locked

cargo test \
  -p stickymd-render \
  --release \
  --locked

cargo test \
  -p stickymd-win \
  --release \
  --locked

git diff --check
```

---

# 401. Cargo Tree

```bash
cargo tree -p stickymd-render
cargo tree -p stickymd-win
```

---

# 402. Forbidden Dependency Check

```bash
cargo tree | rg \
"syntect|onig|onig_sys|tauri|wry|webview|tokio|wgpu|reqwest|hyper|html5ever"
```

预期：

```text
无 runtime forbidden dependency
```

---

# 403. Core unsafe

```bash
rg "\bunsafe\b" crates/stickymd-core
```

runtime：

```text
0
```

---

# 404. Render unsafe

```bash
rg "\bunsafe\b" crates/stickymd-render
```

runtime：

```text
0
```

---

# 405. Parse Count Instrument Test

确认：

```text
100 rapid edits in Split
<< 100 parse calls
```

---

# 406. Resize Instrument Test

确认：

```text
100 resizes
0 new parse calls
```

---

# 407. Scroll Instrument Test

确认：

```text
1000 scrolls
0 parse calls
0 semantic rebuilds
```

---

# 408. Security Runtime Smoke

打开包含：

```html
<script>
<iframe>
```

和：

```markdown
![x](https://...)
```

确认：

- 无执行。
- 无网络。

---

# 409. User-visible Smoke

Release build：

1. 输入Markdown。
2. 切Preview。
3. 查看heading/bold/list/table/code。
4. 选择Preview文字并Copy。
5. 点击安全链接。
6. 切Split。
7. 连续输入。
8. Preview停止输入约1秒才更新。
9. 输入数学四种delimiter。
10. 确认为占位而非错误公式渲染。
11. Raw HTML显示literal。
12. Remote image显示placeholder。
13. Autosave仍工作。
14. 外部编辑器reload后Preview更新。
15. Conflict时Preview仍显示local DocumentState。

---

# 410. Git Commit 建议

如果起始clean，可以：

```text
feat(preview): integrate Comrak semantic pipeline

feat(preview): add owned Markdown tree and render model

feat(preview): add native preview layout and view modes

feat(preview): add selection and safe link interaction

test(preview): verify Markdown semantics and security

docs: record phase 5 preview results
```

不强制commit数量。

---

# 411. 不 Push

```text
push = no
```

除非USER明确要求。

---

# 412. 最终回复格式

必须严格：

# Phase 5 Result

## Preconditions

```text
Phase 4 recommendation
USER approval
starting commit
inherited conditions
```

## Repository State Before Work

```text
branch
clean / dirty
```

## Comrak Integration

```text
version
license
default-features
enabled extensions
disabled heavy features
```

## Dependency Audit

明确：

```text
syntect absent?
onig absent?
CLI feature absent?
network client absent?
```

## Semantic Pipeline

说明：

```text
DocumentSnapshot
→ Comrak
→ Arena AST
→ OwnedDocumentTree
→ RenderTree
→ Layout
→ Paint
```

## Owned AST

列 node coverage。

## Source Mapping

```text
byte range
Unicode result
known limitations
```

## View Modes

### Source

PASS/FAIL

### Preview

PASS/FAIL

### Split

PASS/FAIL

## Preview Scheduling

```text
Split debounce:
Preview immediate refresh:
stale drop:
job coalescing:
```

## Markdown Coverage

表。

## Math

```text
$...$:
$$...$$:
\(...\):
\[...\]:
```

明确：

```text
RaTeX rendering = intentionally deferred to Phase 6
```

## Raw HTML Safety

PASS/FAIL。

## Images

```text
local placeholder:
remote placeholder:
network requests:
```

## Preview Selection

说明。

## Links

```text
allowed schemes
blocked schemes
relative paths
```

## Performance

完整表。

## Memory

完整表。

## Binary Size

Phase4 → Phase5。

## Idle CPU

Source / Preview / Split。

## Worker

```text
thread count
stack
cold/warm
```

## Dependencies Added

表。

## Unsafe

```text
core = 0
render = 0
windows adapter = ...
```

## Architecture Authority

```text
Canonical text owner:
Parser input:
Comrak AST lifetime:
Owned AST status:
Preview status:
```

## Acceptance

```text
AC-013:
AC-014:
AC-016:
AC-017:
```

必须诚实标记partial。

## Architecture Drift

```text
None
```

或Risk report。

## Verification

逐命令PASS/FAIL。

## Documentation

```text
task
report
coverage
overview
plan refinements
dependency report
```

## Git

```text
commits
push = no
```

## Recommendation

只能：

```text
APPROVE Phase 6
```

或：

```text
APPROVE Phase 6 WITH CONDITIONS
```

或：

```text
STOP — architecture review required
```

最后：

> Awaiting USER review. Do not start Phase 6 automatically.

---

# 413. Phase 5 Definition of Done

只有全部成立才算完成：

- [ ] USER已批准Phase5。
- [ ] Phase4 Gate通过。
- [ ] 所有适用plan已读。
- [ ] Comrak正式进入stickymd-render。
- [ ] Comrak exact版本已记录。
- [ ] Comrak `default-features=false`。
- [ ] CLI feature未启用。
- [ ] syntect未引入。
- [ ] onig/onig_sys未引入。
- [ ] approved GFM extensions启用。
- [ ] math_dollars启用。
- [ ] math_latex启用。
- [ ] 未启用未经批准extensions。
- [ ] Comrak AST为transient。
- [ ] Arena不进入AppState。
- [ ] Arena不作为长期Preview。
- [ ] OwnedDocumentTree正式实现。
- [ ] Owned AST无Comrak类型。
- [ ] SourceMap正式实现。
- [ ] CJK source range测试。
- [ ] emoji source range测试。
- [ ] malformed source range fail-safe。
- [ ] RenderTree正式实现。
- [ ] Owned AST与RenderTree职责分离。
- [ ] paragraph Preview。
- [ ] heading Preview。
- [ ] emphasis Preview。
- [ ] strong Preview。
- [ ] strikethrough Preview。
- [ ] blockquote Preview。
- [ ] ordered list Preview。
- [ ] unordered list Preview。
- [ ] nested list Preview。
- [ ] task list Preview。
- [ ] inline code Preview。
- [ ] fenced code Preview。
- [ ] table Preview。
- [ ] thematic break Preview。
- [ ] hard/soft break行为明确。
- [ ] raw HTML literal Preview。
- [ ] raw HTML无执行。
- [ ] link节点。
- [ ] safe scheme validation。
- [ ] javascript/custom scheme blocked。
- [ ] image节点。
- [ ] local image placeholder。
- [ ] remote image placeholder。
- [ ] remote image零网络。
- [ ] inline math节点。
- [ ] display math节点。
- [ ] 四类delimiter由Comrak识别。
- [ ] Phase5未正式调用RaTeX renderer。
- [ ] Math placeholder稳定。
- [ ] Preview Worker存在。
- [ ] Preview Worker最多一个。
- [ ] Preview job bounded/coalesced。
- [ ] 每次edit不立即snapshot。
- [ ] Split debounce =1000ms。
- [ ] Preview模式立即刷新。
- [ ] startup Preview立即刷新。
- [ ] stale result丢弃。
- [ ] stale failure不覆盖最新状态。
- [ ] resize不reparse。
- [ ] scroll不reparse。
- [ ] viewport culling。
- [ ] Source mode工作。
- [ ] Preview mode工作。
- [ ] Split mode工作。
- [ ] Split固定50/50。
- [ ] Split divider不可拖。
- [ ] Source selection在mode切换后保留。
- [ ] Preview scroll独立。
- [ ] 不做Source/Preview scroll sync。
- [ ] Preview text selection。
- [ ] Preview Ctrl+C。
- [ ] Preview Ctrl+A。
- [ ] Preview不能编辑Document。
- [ ] link hit test。
- [ ] safe link open。
- [ ] Source IME无回归。
- [ ] Source autosave无回归。
- [ ] External reload后Preview更新。
- [ ] Conflict期间Preview显示local authority。
- [ ] Comrak parse benchmark。
- [ ] Owned conversion benchmark。
- [ ] RenderTree benchmark。
- [ ] Layout benchmark。
- [ ] 20KiB total Preview benchmark。
- [ ] 100KiB total Preview benchmark。
- [ ] 1MiB background Preview benchmark。
- [ ] 20KiB Preview memory测量。
- [ ] 20KiB Split memory测量。
- [ ] cold/warm Preview测量。
- [ ] idle CPU测量。
- [ ] binary size delta测量。
- [ ] core unsafe=0。
- [ ] render unsafe=0。
- [ ] 无WebView。
- [ ] 无HTML engine。
- [ ] 无network client。
- [ ] 无syntax highlighting。
- [ ] 无image decoder。
- [ ] 无RaTeX production rendering。
- [ ] coverage matrix更新。
- [ ] overview更新。
- [ ] Phase5 task完成。
- [ ] Phase5 report完成。
- [ ] dependency report完成。
- [ ] baseline commands通过。
- [ ] 未自动进入Phase6。

完成后停止。

