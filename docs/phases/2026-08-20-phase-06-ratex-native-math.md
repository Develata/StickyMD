# StickyMD Phase 6 — RaTeX Native Math Layout, Rendering & Formula Cache

你现在位于 StickyMD 本地 Git 仓库根目录。

Phase 0 已完成治理与架构合同。

Phase 1 已完成技术 Spike。

Phase 2 已完成 canonical `DocumentState`。

Phase 3 已完成 Source Editor + IME。

Phase 4 已完成 Portable Persistence / Autosave / Recovery / Conflict。

Phase 5 已完成：

```text
DocumentSnapshot
→ Comrak
→ OwnedDocumentTree
→ RenderTree
→ Native Preview
```

并已经正式识别：

```text
$...$
$$...$$
\(...\)
\[...\]
```

但仍使用数学 placeholder。

USER 已批准进入 Phase 6。

本阶段名称：

> **Phase 6 — RaTeX Native Math Layout, Rendering & Formula Cache**

---

# 0. Phase 6 唯一目标

将 Phase 5 中：

```text
InlineMathPlaceholder
DisplayMathPlaceholder
```

正式替换成：

```text
Comrak MathNode
      │
      ▼
StickyMD MathNode
      │
      ▼
RaTeX Parser
      │
      ▼
RaTeX ParseNode
      │
      ▼
RaTeX Layout
      │
      ▼
RaTeX DisplayList
      │
      ▼
StickyMD Math Painter
      │
      ▼
Native tiny-skia Raster
      │
      ▼
Inline / Display Math Box
      │
      ▼
Native Preview
```

本阶段必须使数学公式成为 Preview layout 的真正一等对象。

---

# 1. Phase 6 最终用户能力

完成后：

```markdown
$E=mc^2$
```

应渲染为真正数学公式。

```markdown
$$
\int_0^1 x^2\,dx
$$
```

应成为独立 display math block。

以下全部正式支持：

```text
$...$       inline
\(...\)     inline

$$...$$     display
\[...\]     display
```

delimiter 仍由：

```text
Comrak
```

决定。

公式内部语义由：

```text
RaTeX / KaTeX-compatible math syntax
```

决定。

---

# 2. 本阶段明确禁止

Phase 6 不允许正式实现：

```text
图片 decode
图片 clipboard
images/ asset management
.trash
图片 GC
图片 Undo side effect
Export

Dock
Auto-hide
Hover reveal
Tray final lifecycle
Multi-monitor docking

最终 theme selector
最终 opacity selector

Mermaid
PlantUML
TikZ
MathJax
KaTeX JavaScript
LaTeX executable
TeX Live
WebView
SVG intermediate renderer
HTML renderer
PDF renderer
```

---

# 3. 绝对禁止数学自研

不得自己实现：

```text
TeX lexer
TeX parser
macro expansion
fraction layout
root layout
superscript layout
subscript layout
matrix layout
delimiter stretching
math spacing
math font metrics
```

这些全部属于：

```text
RaTeX
```

---

# 4. StickyMD 自己只负责三件事

StickyMD Phase 6 自己拥有：

### 4.1 Adapter

```text
StickyMD MathNode
→ RaTeX
```

### 4.2 Platform Painter

```text
RaTeX DisplayList
→ tiny-skia
```

### 4.3 Preview Integration

```text
Math box
→ StickyMD paragraph/block layout
```

这三项不是重新实现数学排版。

---

# 5. Phase 5 条件必须继承

Phase 5 最终 Recommendation：

```text
APPROVE Phase 6 WITH CONDITIONS
```

继承的 `NOT TESTED`：

```text
真实 Windows 视觉验收
真实 Shell 行为
Private Working Set
idle CPU
部分人工 Preview 验收
```

这些状态不得因为进入 Phase 6 自动变成 PASS。

---

# 6. Phase 6 Preflight 特别 Gate

在添加任何 RaTeX production dependency 前：

必须先使用当前 Phase 5 commit 测量：

```text
Source Private Working Set
Preview Private Working Set
Split Private Working Set

Source idle CPU
Preview idle CPU
Split idle CPU
```

原因：

> 没有 Phase 5 基线，就无法判断 RaTeX 给内存增加了多少。

---

# 7. Preflight 测量要求

使用：

```text
Release build
Windows 11
真实 standalone portable exe
无 debugger
等待 30 秒
重复至少 5 次
```

记录：

```text
median
max
```

至少：

```text
Private Working Set
Private Bytes / Commit
```

---

# 8. Preflight 文档

把结果追加到：

```text
docs/report/phase-05-markdown-native-preview.md
```

新增：

```markdown
## Phase 6 Preflight Runtime Baseline
```

明确：

```text
Measured during Phase 6 preflight.
```

---

# 9. Preflight Idle CPU

每模式：

```text
60 seconds
```

记录：

```text
average CPU
```

目标：

```text
< 0.1%
```

如果现有 Phase 5 已明显违反：

先分析。

不要把 RaTeX 加进去后再混淆原因。

---

# 10. Phase 5 Runtime Gate

如果发现明显结构性问题，例如：

```text
Preview idle CPU > 1%
持续 redraw
Preview memory明显持续增长
```

立即停止 Phase 6。

创建：

```text
docs/report/phase-06-preflight-blocked.md
```

等待 USER。

小幅偏离目标但架构正常：

可以记录并继续。

---

# 11. 开始前必须读取

严格遵循 `AGENTS.md`。

至少：

```text
AGENTS.md
docs/AGENTS.md
docs/plan/AGENTS.md

docs/plan/00_engineering_constitution.md
docs/plan/01_terminology.md
docs/plan/02_positioning_and_scope.md
docs/plan/03_system_architecture.md
docs/plan/04_runtime_state_model.md
docs/plan/06_markdown_math_rendering.md
docs/plan/10_performance_reliability.md
docs/plan/11_testing_and_release.md

docs/features/00_v1_product_behavior.md
docs/acceptance-cases/00_v1_acceptance.md
docs/coverage-matrix.md

docs/report/phase-01-technical-spike-report.md
docs/report/phase-05-markdown-native-preview.md
docs/report/phase-05-dependency-delta.md

docs/tasks/phase-05-markdown-native-preview.md
```

同时读取：

```text
experiments/phase-01/markdown-math/
```

若仍存在。

---

# 12. 仓库检查

执行：

```bash
git status --short
git branch --show-current
git log -10 --oneline

cargo metadata --no-deps

cargo tree -p stickymd-render
cargo tree -p stickymd-win
```

记录：

```text
branch
starting commit
clean / dirty
```

---

# 13. RaTeX 当前上游事实

开始 dependency 修改前必须重新验证当前 upstream。

当前 Prompt 编写时基线是：

```text
RaTeX workspace version: 0.1.14
license: MIT
```

主要 crates：

```text
ratex-parser
ratex-layout
ratex-types
ratex-font
ratex-font-loader
ratex-katex-fonts
ratex-unicode-font
```

但 Agent 必须重新核实 crates.io / Cargo resolved version。

不得仅信 Prompt 中版本。

---

# 14. 当前公开 pipeline

当前 upstream 大致公开：

```text
ratex_parser::parse / Parser
        ↓
ratex_layout::layout
        ↓
ratex_layout::to_display_list
        ↓
ratex_types::DisplayList
```

Agent 必须检查 exact API。

不要根据 Prompt 猜函数签名。

---

# 15. DisplayList 语义

当前上游 `DisplayList` 是：

```text
width
height
depth
items
```

其中：

```text
height = baseline above
depth  = baseline below
```

总高度：

```text
height + depth
```

必须确认实际 upstream semantics。

---

# 16. DisplayItem

当前基线只有：

```text
GlyphPath
Line
Rect
Path
```

如果当前版本新增 variant：

必须处理或 fail safely。

不得：

```rust
_ => {}
```

静默忽略用户公式的一部分。

---

# 17. RaTeX dependency 必须进入哪里

正式 runtime dependency 只允许进入：

```text
stickymd-render
```

不得进入：

```text
stickymd-core
```

---

# 18. Core Gate

最终：

```bash
cargo tree -p stickymd-core
```

不得出现：

```text
ratex
```

---

# 19. RaTeX crate 选择原则

不要一股脑依赖整个 workspace。

优先最小集合。

预期可能是：

```text
ratex-parser
ratex-layout
ratex-types
ratex-font-loader
```

加必要 transitive crates。

---

# 20. 不允许 production 依赖

除非实际必要且有报告：

```text
ratex-svg
ratex-pdf
ratex-wasm
ratex-cairo
ratex-gtk4
ratex-ffi
```

这些都不属于 StickyMD。

---

# 21. ratex-render 特别审计

当前 upstream `ratex-render`：

```text
依赖 tiny-skia 0.11
提供 PNG raster renderer
```

而 StickyMD 已经有自己的 tiny-skia。

因此：

**不得未经审计直接把 `ratex-render` 放入 production dependency。**

---

# 22. 先检查 StickyMD tiny-skia 版本

执行：

```bash
cargo tree -p stickymd-render | rg "tiny-skia"
```

记录 exact version。

---

# 23. 如果版本不同

例如：

```text
StickyMD tiny-skia 0.12
RaTeX renderer tiny-skia 0.11
```

production 使用 `ratex-render` 将导致：

```text
duplicate tiny-skia
duplicate types
binary growth
```

默认拒绝。

---

# 24. 即使 tiny-skia 版本相同

仍然检查：

```text
ratex-render 是否提供 direct Pixmap / painter API
```

如果只公开：

```text
render_to_png()
```

也不适合作为正式热路径。

---

# 25. PNG 热路径禁止

正式 Preview 不得：

```text
DisplayList
→ encode PNG
→ PNG bytes
→ decode PNG
→ Pixmap
```

这是 hard rule。

---

# 26. PNG 仅允许 Test Oracle

`ratex-render` 可以作为：

```text
dev-dependency
```

用于：

```text
reference rendering
golden comparison
```

如果有价值。

---

# 27. Production Painter 优先方案

优先级：

### A — Upstream direct painter

如果当前 RaTeX 已公开：

```text
render_into_pixmap
paint_display_list
```

之类 API：

优先使用。

前提：

- compatible tiny-skia。
- 无 PNG hot path。
- dependency合理。

---

# 28. B — StickyMD thin DisplayList painter

如果没有 direct API：

在：

```text
stickymd-render
```

实现一个**极薄**：

```text
RaTeX DisplayList
→ existing tiny-skia surface
```

的 painter。

---

# 29. Thin Painter 不属于数学引擎

它只能负责：

```text
GlyphPath → glyph outline
Line → line
Rect → rect
Path → path
```

不得包含：

```text
fraction logic
sqrt logic
sup/sub logic
matrix logic
```

---

# 30. 不复制整个 ratex-render

禁止：

```text
vendor crates/ratex-render
```

或：

```text
copy entire renderer crate
```

---

# 31. 如果参考 upstream renderer

若实现结构明显借鉴：

```text
RaTeX ratex-render/src/renderer.rs
```

必须：

- 保持 upstream MIT attribution。
- 在源码注释中注明来源。
- 更新 `THIRD_PARTY_NOTICES.md`。
- 记录 upstream commit。

---

# 32. 推荐 source header

例如：

```rust
//! DisplayList painter for StickyMD.
//!
//! The mathematical parser and layout are provided by RaTeX.
//! Rendering logic is adapted in part from RaTeX's MIT-licensed
//! ratex-render backend. See THIRD_PARTY_NOTICES.md.
```

不要机械照抄版权内容之外的实现。

---

# 33. Painter 行数

目标：

```text
200–500 LOC
```

不是硬限制。

如果出现 >1000 LOC：

检查是否开始重新实现 math。

---

# 34. 数学模块建议

```text
crates/stickymd-render/src/math/
├─ mod.rs
├─ engine.rs
├─ model.rs
├─ painter.rs
├─ fonts.rs
├─ cache.rs
├─ raster.rs
└─ error.rs
```

根据 cohesion 合并。

不要机械拆。

---

# 35. plan_ref

所有正式模块：

```rust
//! plan_ref: docs/plan/06_markdown_math_rendering.md#...
```

---

# 36. MathEngine

建议建立：

```rust
struct MathEngine
```

职责：

```text
source
→ RaTeX parse
→ RaTeX layout
→ DisplayList
→ StickyMD math result
```

---

# 37. MathEngine 不知道

```text
DocumentState
Window
winit
filesystem
clipboard
```

---

# 38. 输入

建议：

```rust
struct MathRequest<'a> {
    source: &'a str,
    kind: MathKind,
    style: MathStyleContext,
}
```

---

# 39. MathKind

```rust
enum MathKind {
    Inline,
    Display,
}
```

这个是 StickyMD 自己的类型。

不要把 RaTeX `MathStyle` 泄漏到 Preview public API。

---

# 40. Inline style

RaTeX inline math 应使用：

```text
Text style
```

而不是 Display style。

当前 RaTeX `MathStyle` 包含：

```text
Display
Text
Script
ScriptScript
...
```

Agent 必须验证 current API。

---

# 41. Display style

Display formula：

```text
MathStyle::Display
```

或 current equivalent。

---

# 42. 不让 RaTeX 自己决定 delimiter

输入 RaTeX 的 source：

应已经是：

```text
delimiter 内部内容
```

---

# 43. Comrak 仍拥有 delimiter semantics

例如：

```text
$x$
```

Comrak：

```text
识别 InlineMath
```

StickyMD：

```text
取 math source
```

RaTeX：

```text
只看到 x
```

---

# 44. 不 regex 剥 delimiter

Phase 5 Owned MathNode 应已经保存：

```text
inner source
raw/source range
kind
```

直接使用。

---

# 45. 如果 Phase 5 MathNode 没有足够信息

允许最小补充：

```text
raw_source_range
inner_source
delimiter_kind
```

但不能改变 Comrak semantic authority。

---

# 46. 公式 Copy

公式选择复制必须尽可能使用：

```text
DocumentSnapshot exact source range
```

而不是：

```text
重新拼 $ + source + $
```

---

# 47. 原 delimiter 保留

例如：

```text
\(x\)
```

复制最好仍得到：

```text
\(x\)
```

不是：

```text
$x$
```

---

# 48. Math result model

不要让 RaTeX types扩散。

可以 crate-private：

```rust
struct MathDisplay {
    display_list: ratex_types::DisplayList,
    metrics: MathMetrics,
}
```

但对 Preview pipeline暴露 StickyMD type：

```rust
struct MathBox {
    width: f32,
    ascent: f32,
    descent: f32,
    ...
}
```

---

# 49. DisplayList 生命周期

可以：

```text
cache
```

但不是 authority。

---

# 50. 数学权威关系

```text
DocumentState source
    ↓
MathNode
    ↓
RaTeX DisplayList
    ↓
MathRaster
```

任何下层都不能反写 source。

---

# 51. Math parse failure

RaTeX：

```text
Err(ParseError)
```

StickyMD：

不得 panic。

生成：

```text
MathErrorBox
```

---

# 52. MathErrorBox

Preview 中显示：

```text
raw formula source
```

配：

```text
轻微错误边框 / indicator
```

---

# 53. 不自动修复公式

禁止：

```text
猜少了 }
补 delimiter
修改 command
```

---

# 54. 错误 hover

可以显示简短：

```text
公式无法解析
```

详细 error 可只在 Debug diagnostics。

本阶段 tooltip不是 gate。

---

# 55. 错误公式 Copy

仍复制：

```text
原始完整 source
```

---

# 56. 公式资源限制

正式落实此前规格：

```text
单个 math source <= 64 KiB
单个文档 math nodes <= 2000
```

---

# 57. 超长公式

超过 64 KiB：

不调用 RaTeX。

Preview：

```text
显示 raw source / "公式过长"
```

---

# 58. 超过 2000 formulas

前 2000 可以渲染。

之后：

```text
fallback raw math
```

或整个 Preview标记 resource guard。

选择清楚、测试。

推荐逐节点 fallback。

---

# 59. RaTeX 自身 stack safety

当前 upstream parser 已有内部 depth budget。

但 StickyMD 不能只假设安全。

必须测试：

```text
深层 {}
深层 \frac
深层 \left
```

---

# 60. 不 catch_unwind 作为主策略

Release：

```text
panic = abort
```

因此不要假装：

```text
catch_unwind
```

可以救所有 panic。

依靠：

- upstream safe parser。
- input guard。
- tests。
- fuzz-like deterministic stress。

---

# 61. Formula raster safety

除了 source 长度，还要保护 raster size。

建立工程 guard。

建议：

```text
MAX_SINGLE_MATH_RASTER_BYTES = 8 MiB
```

---

# 62. Raster size 计算必须 checked

```text
width * height * 4
```

必须：

```text
checked_mul
```

---

# 63. 超过 raster budget

不 allocate。

显示：

```text
公式尺寸过大，预览已省略
```

保留 source。

---

# 64. 不允许单公式引发几十/几百 MiB allocation

hard invariant。

---

# 65. Font strategy

数学字体正式使用：

```text
RaTeX KaTeX-compatible fonts
```

---

# 66. Portable 字体策略

StickyMD 是 portable single EXE。

因此优先：

```text
embedded KaTeX math fonts
```

而不是：

```text
要求用户安装字体目录
```

---

# 67. ratex-font-loader

检查 exact feature。

当前 upstream：

```text
default = []
embed-fonts = [...]
```

优先使用：

```text
embed-fonts
```

---

# 68. 不把字体文件手工复制进 repo

如果 RaTeX crate 自带字体并合法 embed：

直接依赖 crate。

不要维护 StickyMD 私有字体副本。

---

# 69. 数学字体许可证

RaTeX 程序：

```text
MIT
```

KaTeX 字体：

```text
SIL Open Font License 1.1
```

---

# 70. 必须更新

```text
THIRD_PARTY_NOTICES.md
```

以及 release license 目录规划。

---

# 71. 至少保留

```text
SIL-OFL-1.1.txt
KaTeX-fonts-NOTICE.txt
```

内容应来源于 upstream RaTeX。

---

# 72. Cargo deny

如果需要：

更新：

```text
deny.toml
```

接受：

```text
OFL-1.1
```

或实际 SPDX identifier。

---

# 73. 不因为 cargo deny 方便就把所有 font license wildcard 放开

精确允许。

---

# 74. Font Lazy Loading

数学字体不能在：

```text
Source-only startup
```

就全部加载。

---

# 75. Hard requirement

从未打开 Preview / 没有公式时：

```text
RaTeX font loading = 0
```

---

# 76. 第一次真正公式渲染

才：

```text
load required font subset
```

---

# 77. Font loader复用

RaTeX font loader本身已有 lazy/cache能力。

优先使用。

不要建立第二套完整 font loader。

---

# 78. 不每公式重新加载字体

hard invariant。

---

# 79. Font Load Plan

如果 DisplayList只需要：

```text
MainRegular
MathItalic
Size1
```

不要无脑初始化所有字体外加fallback。

使用 RaTeX loader 的按 display items需求计划。

---

# 80. Unicode math fallback

测试：

```text
\text{中文}
```

以及 Unicode math：

```text
α β ∑ ∫
```

---

# 81. CJK in \text

如果 RaTeX 当前不完整支持：

记录：

```text
CONDITIONAL
```

不要自行重写 math text shaping。

---

# 82. Math Painter

Painter 输入：

```text
DisplayList
FontSet
scale
origin
```

输出：

```text
tiny-skia Pixmap / PixmapMut
```

---

# 83. Painter item support

必须 100% 覆盖 current DisplayItem variants。

---

# 84. GlyphPath

优先沿用 upstream renderer所使用的：

```text
ab_glyph
ttf-parser
```

或当前公开字体outline能力。

---

# 85. 不通过 cosmic-text画 RaTeX glyph

因为：

RaTeX DisplayList指定自己的：

```text
font
char_code
scale
metrics
```

使用另一 text layout engine可能破坏几何。

---

# 86. Glyph geometry

必须使用 RaTeX 指定数学字体 outline。

---

# 87. Lines

支持：

```text
fraction
overline
array rules
```

---

# 88. Dashed Line

`DisplayItem::Line` 若：

```text
dashed = true
```

必须实现。

不要忽略。

---

# 89. Rect

支持：

```text
\colorbox
```

等。

---

# 90. Path

支持：

```text
radicals
large delimiters
arrows/path-based shapes
```

---

# 91. Path 命令

必须处理 current：

```text
MoveTo
LineTo
CurveTo
Close
...
```

实际 variant按 upstream。

未知 variant：

typed error。

不要静默 skip。

---

# 92. Painter Color

RaTeX DisplayList携带 color。

必须尊重：

```text
\color
\colorbox
```

等。

---

# 93. 默认 formula color

由当前 Preview style foreground 注入：

```text
Light → dark
Dark → light
```

---

# 94. Theme color不是数学 parser职责

通过：

```text
LayoutOptions / style context
```

设置 default color。

---

# 95. 用户显式 \color

应覆盖 default color。

按 RaTeX semantics。

---

# 96. 透明背景

Math raster background：

```text
transparent
```

---

# 97. 不把 paper background烘焙进 formula bitmap

这样：

- theme更灵活。
- selection背景可正确显示。
- composition简单。

---

# 98. Formula bitmap

建议：

```rust
struct MathRaster {
    pixels: ...,
    width_px: u32,
    height_px: u32,
    ascent_px: f32,
    descent_px: f32,
}
```

---

# 99. 不直接长期存 tiny-skia Pixmap 如果它妨碍 Send

根据实际 trait检查。

可以使用：

```text
Arc<[u8]>
```

+ dimensions。

---

# 100. 不 unsafe impl Send

绝对禁止。

---

# 101. Math metrics

RaTeX DisplayList自然提供：

```text
width
height
depth
```

转换：

```text
width_px = width * em_px
ascent = height * em_px
descent = depth * em_px
```

具体加padding时必须修正。

---

# 102. Painter Padding

用于抗锯齿 edge 可以留：

```text
1–2 physical px
```

但：

math box logical metrics不能把padding当正文尺寸。

---

# 103. Baseline

这是 Phase 6 最重要视觉 invariant：

```text
inline formula baseline
==
surrounding text baseline
```

---

# 104. Inline Math Integration

原：

```text
Text span
Math
Text span
```

必须成为：

```text
text inline box
math atomic inline box
text inline box
```

---

# 105. 不能把公式变成 Unicode replacement text

禁止：

```text
" [formula] "
```

然后作为 text layout。

---

# 106. Inline math 是 replaced object

在 paragraph line breaker中拥有：

```text
width
ascent
descent
source range
raster reference
```

---

# 107. Line Breaking

公式作为：

```text
atomic unbreakable inline box
```

---

# 108. Inline Formula 换行

如果当前行剩余空间不足：

整个 formula移到下一行。

---

# 109. 公式内部不由 StickyMD断行

---

# 110. Inline 公式比 viewport更宽

不得 crash。

保守：

```text
放到独立行
clip to content viewport
```

不要强制缩到极小。

---

# 111. Display Math

作为：

```text
standalone block
```

---

# 112. Display Math 对齐

如果自然宽度 <= 内容宽度：

```text
center
```

---

# 113. Display Math 过宽

Phase 6 推荐保守行为：

```text
left align within block
clip to viewport
```

不要静默把公式缩到 30%。

---

# 114. 不实现 nested horizontal scrollbar

先保持极简。

未来如果真有需求再分析。

---

# 115. Display Math Vertical Space

固定适当：

```text
margin before
margin after
```

由 PreviewStyle token管理。

---

# 116. Inline font size

使用正文 Preview size：

例如：

```text
17 DIP
```

的适当数学 em。

建议：

```text
1.0 × body font size
```

---

# 117. Display font size

可以：

```text
1.05–1.1 × body
```

如果未在 plan固定：

优先：

```text
1.0
```

减少额外设计。

---

# 118. 禁止用户调 Math Font Size

---

# 119. Formula Cache

Phase 6 必须实现 bounded cache。

---

# 120. 两层 cache

建议：

```text
MathLayoutCache
MathRasterCache
```

---

# 121. MathLayoutCache

保存：

```text
source
kind
RaTeX DisplayList / metrics
```

---

# 122. Layout Cache Key

至少包含：

```text
math source
inline/display kind
math semantic/style revision
default math foreground if baked into DisplayList
```

---

# 123. DPI 不需要影响数学 geometry

如果 DisplayList是 em-relative：

DPI不应影响 LayoutCache key。

必须确认实际 upstream。

---

# 124. MathRasterCache Key

至少：

```text
layout identity
font size
device scale
theme/style color revision
```

---

# 125. Cache 不得用 generation 作为 key

同一公式在新 generation：

应能复用。

---

# 126. 同一公式重复出现

例如 100 次：

```text
$x^2$
```

应显著 cache hit。

---

# 127. Layout cache bound

建议：

```text
max 512 entries
```

---

# 128. Raster cache hard memory

冻结：

```text
<= 8 MiB
```

---

# 129. Raster cache accounting

精确按：

```text
pixel bytes
+ small metadata estimate
```

---

# 130. LRU

可以用简单：

```text
HashMap + VecDeque
```

不要为此引入大型 cache crate。

---

# 131. 不需要完美 O(1) LRU

512 entries范围下：

简单、正确优先。

---

# 132. Cache eviction

超过预算：

删除最久未使用。

---

# 133. 单 raster >8MiB

不进入 cache。

若超过安全单公式预算：

直接 fallback。

---

# 134. Source Mode Cache Policy

切回：

```text
Source
```

推荐：

```text
立即释放 MathRasterCache
保留小型 LayoutCache
```

原因：

用户优先低内存。

---

# 135. Preview/Split 再打开

重新 raster，reuse layout。

---

# 136. 不用 background timer 自动清 cache

避免多一套计时状态机。

---

# 137. DPI Change

DPI改变：

```text
LayoutCache keep
RasterCache invalidate
```

---

# 138. Resize

普通宽度 resize：

```text
MathLayoutCache keep
MathRasterCache keep
```

只改变公式位置/line break。

---

# 139. Scroll

```text
Math cache untouched
```

---

# 140. Theme style revision

如果 default color改变：

invalidate受影响的 math raster/display entries。

---

# 141. Preview generation替换

Math cache是共享 bounded cache。

不需要随旧Preview全清。

---

# 142. 但旧 formula Arc引用必须释放

cache + latest layout之外不得持有。

---

# 143. Cache instrumentation

Debug/test：

```text
layout_hits
layout_misses
raster_hits
raster_misses
evictions
raster_bytes
```

---

# 144. 不做 telemetry

仅本地 debug/test。

---

# 145. Preview Worker 与 Math

优先：

```text
Preview worker
```

中完成：

```text
RaTeX parse
layout
DisplayList
raster
```

---

# 146. UI Thread

只：

```text
paint ready math bitmap
```

---

# 147. 如果字体/Pixmap跨线程问题

不得 unsafe。

分析实际 Send/Sync。

---

# 148. 如果 Math raster无法跨线程

可以：

```text
worker produce DisplayList
UI raster with per-frame budget
```

但必须测量。

---

# 149. UI Math raster budget

如果不得不UI raster：

单 frame：

```text
<=4 ms
```

---

# 150. 不允许 100个公式在一帧UI thread全部同步 raster

---

# 151. 首选还是 worker raster

---

# 152. Preview stale result

Math integration不能破坏 Phase5规则。

```text
gen 10 math build
current gen 12
```

结果：

```text
drop entire gen10 preview
```

---

# 153. 不把 stale formula cache当问题

cache本身可留相同source结果。

但 stale Preview不能 commit。

---

# 154. Math Parse Error不是整个 Preview Error

单公式错误：

```text
only that formula fallback
```

其余文档正常。

---

# 155. Math Layout Error同理

---

# 156. Math Painter Error同理

---

# 157. Font Load Error

如果数学字体加载失败：

这是全局 math capability失败。

Preview仍显示：

```text
raw formula fallback
```

Source继续工作。

---

# 158. 不因数学字体失败让整个App退出

---

# 159. Font Error 状态

可：

```text
MathUnavailable
```

并在 Preview公式处统一 fallback。

---

# 160. Formula Selection

Preview 中公式作为：

```text
atomic selectable item
```

---

# 161. 不实现公式内部 glyph-by-glyph selection

---

# 162. 点击拖选跨公式

例如：

```text
text A [formula] text B
```

selection跨越：

Copy 结果包含：

```text
A
<original formula source>
B
```

---

# 163. 公式单独选中

视觉 selection：

覆盖公式 bounding box。

---

# 164. Copy exact source

优先：

```text
DocumentSnapshot[source_range]
```

---

# 165. 如果 source range unavailable

fallback：

使用 MathNode保存的 raw source。

---

# 166. 绝不复制渲染后的Unicode approximation

---

# 167. Ctrl+A Preview

包括所有 math source。

---

# 168. Ctrl+C math-only

得到：

```text
原始 markdown math
```

例如：

```text
\[
x^2
\]
```

---

# 169. Copy 不取 RaTeX ParseNode

---

# 170. Accessibility

公式不做MathML。

Phase6不需要。

---

# 171. Tooltips

不是 gate。

---

# 172. Math Error Copy

保留 malformed source。

---

# 173. Formula Visual Error Style

建议：

```text
code-like text
subtle red/danger border
```

不要巨大错误框。

---

# 174. Error 不显示 stack trace

---

# 175. KaTeX-compatible Scope

不要在 StickyMD 再维护一份 command whitelist。

---

# 176. 支持的命令

定义：

> 当前 RaTeX release所支持的 KaTeX-compatible数学语法。

---

# 177. 不“修复”RaTeX 不支持的 command

如果 upstream不支持：

显示公式错误。

---

# 178. 需要提交 upstream issue吗？

可以在 report建议。

不要修改产品骨架。

---

# 179. Math Fixture 基础

必须至少覆盖：

```text
x
x^2
x_i
x_i^2
a+b=c
\frac{a}{b}
\sqrt{x}
\sqrt[3]{x}
```

---

# 180. Greek

```text
\alpha
\beta
\gamma
\Gamma
\Delta
\pi
\theta
```

---

# 181. Large operators

```text
\sum_{n=1}^{\infty}
\prod_{k=1}^{n}
\int_0^1
\iint
\lim_{x\to0}
```

---

# 182. Delimiters

```text
\left(\frac ab\right)
\left[
\right]
\left\{
\right\}
\middle|
```

---

# 183. Matrices

至少：

```text
matrix
pmatrix
bmatrix
vmatrix
Vmatrix
```

---

# 184. Cases

```text
\begin{cases}
...
\end{cases}
```

---

# 185. Align

按 RaTeX 支持：

```text
aligned
align-like environment
```

不能假设所有环境都有。

记录实际。

---

# 186. Fonts/styles

```text
\mathbf
\mathrm
\mathit
\mathbb
\mathcal
\mathfrak
\mathtt
```

按 RaTeX 支持。

---

# 187. Text

```text
\text{hello}
\operatorname{rank}
```

---

# 188. CJK text in math

测试：

```text
\text{中文}
```

---

# 189. Accents

```text
\hat{x}
\bar{x}
\vec{x}
\overline{AB}
\overrightarrow{AB}
```

---

# 190. Relations

```text
\le
\ge
\neq
\approx
\sim
\subset
\in
```

---

# 191. Arrows

```text
\to
\rightarrow
\Leftarrow
\leftrightarrow
```

---

# 192. Sets

```text
\mathbb{R}
\mathbb{Z}
\emptyset
```

---

# 193. Spacing

```text
\,
\;
\quad
\qquad
```

---

# 194. Fractions nested

至少 5层。

---

# 195. Binomial

```text
\binom{n}{k}
```

若支持。

---

# 196. Colors

如果 RaTeX支持：

```text
\color{red}{x}
\colorbox{yellow}{x}
```

测试 painter。

---

# 197. Lines / dashed path

使用能触发：

```text
Line dashed
```

的 upstream fixture。

---

# 198. Path items

使用：

```text
sqrt
large delimiters
```

确保 Painter Path实际覆盖。

---

# 199. Malformed

至少：

```text
\frac{
\sqrt{
x^
\begin{matrix}
\left(
```

---

# 200. Deep input

构造 >RaTeX depth budget。

必须：

```text
error
not crash
```

---

# 201. Huge source

65 KiB+。

StickyMD guard应先拦截。

---

# 202. 2001 formulas

document guard。

---

# 203. Differential Painter Test

这是 Phase6强烈推荐的重要测试。

---

# 204. ratex-render 作为 dev oracle

如果当前 upstream `ratex-render`可方便使用：

只加入：

```text
[dev-dependencies]
```

---

# 205. Oracle pipeline

相同：

```text
DisplayList
```

分别：

```text
A → upstream ratex-render PNG
B → StickyMD MathPainter bitmap
```

---

# 206. Compare

至少：

```text
dimensions
baseline metrics
alpha coverage
pixel difference tolerance
```

---

# 207. 不要求完全bit-identical

tiny-skia版本可能不同。

允许抗锯齿小差异。

---

# 208. Geometry必须一致

formula bounding box不能明显偏差。

---

# 209. Oracle fixture

至少 30个代表公式。

---

# 210. production cargo tree

必须证明：

```text
ratex-render
```

如果仅dev：

不进入 normal dependency。

执行：

```bash
cargo tree -p stickymd-win -e normal | rg "ratex-render"
```

预期：

```text
无
```

---

# 211. 如果不能用ratex-render oracle

使用：

```text
RaTeX upstream golden metrics
```

或项目 własny golden。

不要因此阻塞。

---

# 212. Formula Geometry Golden

对于每个 fixture保存：

```text
width
ascent
descent
item count
```

---

# 213. Painter Golden

选择少量：

```text
fraction
sqrt
matrix
large delimiter
color
```

做 raster regression。

---

# 214. Pixel Golden 环境

应固定：

```text
embedded math fonts
DPI 100
known math size
transparent bg
```

---

# 215. 不依赖系统仿宋做 math golden

---

# 216. DPI Tests

至少：

```text
100%
125%
150%
200%
```

---

# 217. Math scale

公式应随 DPI清晰放大。

---

# 218. 不允许双 scaling

常见bug：

```text
DIP size * DPI
然后DisplayList又再乘DPI
```

必须测试。

---

# 219. 100/200 DPI size relation

物理pixel约2倍。

逻辑DIP大小保持。

---

# 220. Inline Baseline Tests

混排：

```text
Before $x^2$ After
中文 $x_i$ 文本
```

检查：

```text
formula baseline与body baseline
```

---

# 221. Tall Inline Formula

```text
A $\frac{\frac ab}{\frac cd}$ B
```

行高必须增加。

---

# 222. Descent Formula

带深subscript。

不能被下一行clip。

---

# 223. Math Selection Bounding

选择框覆盖真实formula box。

---

# 224. Math Hover link无关

公式不是link。

---

# 225. Inline Math Wrap

构造：

```text
非常接近行尾 + formula
```

必须整体移下一行。

---

# 226. 不切 formula中间

hard invariant。

---

# 227. Display Center Test

多宽度viewport。

---

# 228. Display Overwidth

超宽公式：

不得造成：

```text
panic
infinite width allocation
whole Preview resize
```

---

# 229. Preview viewport仍固定

公式clip。

---

# 230. Table 内公式

这是重要 integration test：

```markdown
| A | B |
|---|---|
| $x^2$ | $\frac12$ |
```

必须正确。

---

# 231. List 内公式

---

# 232. Quote 内公式

---

# 233. Heading 内 inline math

如果 Comrak允许：

正确。

---

# 234. Bold containing math

按 Markdown semantic实际。

---

# 235. Code 中 math delimiter

必须仍显示code。

不调用RaTeX。

---

# 236. Raw HTML 中 math-like text

仍literal。

不调用RaTeX。

---

# 237. Escaped math

由Comrak决定。

---

# 238. Preview scheduling回归

100 rapid Split edits：

仍：

```text
≈1 latest preview build
```

---

# 239. RaTeX调用计数

增加 debug counters：

```text
math_parse_count
math_layout_count
math_raster_count
```

---

# 240. Resize hard invariant

100 resize：

如果formula source/style/DPI未变：

```text
math_parse_count delta = 0
math_layout_count delta = 0
math_raster_count delta = 0
```

仅Preview block layout改变。

---

# 241. Scroll invariant

1000 scroll：

```text
math parse = 0
math layout = 0
math raster = 0
```

---

# 242. DPI change

应：

```text
math parse = 0
math layout preferably 0
math raster rebuild
```

---

# 243. Theme foreground change

可能：

```text
DisplayList/raster rebuild
```

根据cache设计。

记录。

---

# 244. Repeated identical formulas

文档含100个：

```text
$x^2$
```

理想：

```text
layout misses = 1
raster misses = 1
hits ≫ misses
```

---

# 245. Unique formulas

测试：

```text
500 unique formulas
```

确认 cache bound。

---

# 246. Raster cache eviction

必须触发并测试：

```text
<=8MiB
```

---

# 247. Layout cache eviction

超过512 entries：

老项淘汰。

---

# 248. Cache Key Correctness

以下不能误共享：

```text
inline x
display x
不同 font size
不同 DPI raster
不同 color/style
```

---

# 249. Source collision

使用完整 source equality作为HashMap key判等。

不要只用短hash而不比较原source。

---

# 250. 不需要cryptographic hash做runtime cache

可以直接：

```text
String/Arc<str> key
```

或 stable owned key。

---

# 251. Math Cache线程所有权

推荐：

```text
Preview worker owns MathEngine/cache
```

避免锁。

---

# 252. UI thread不访问mutable math cache

---

# 253. Result中的 raster

使用：

```text
Arc<MathRaster>
```

或等价 immutable data。

---

# 254. Raster lifetime

LaidOutDocument引用。

cache也可能引用。

旧文档释放后正确refcount。

---

# 255. 不产生双份 pixel Vec

避免：

```text
cache Vec
+
layout clone Vec
```

用 Arc。

---

# 256. Source-only memory

引入RaTeX后：

从未打开含数学Preview：

runtime memory不应明显上涨。

---

# 257. Phase6 memory baseline

测：

```text
Source
Preview no math
Preview 1 formula
Preview 20 formulas
Split 20 formulas
Preview 200 unique formulas
Source after leaving Preview
```

---

# 258. 20KiB +20 formula hard gate

沿总规格：

```text
Preview Private Working Set <=52 MiB
```

---

# 259. Split 20 formula hard gate

```text
<=64 MiB
```

---

# 260. 如果 Phase5 baseline本身接近门槛

报告：

```text
raw current value
math incremental delta
```

不要模糊。

---

# 261. Math incremental memory

重点：

```text
Preview no math
→ Preview 20 math
```

delta。

---

# 262. Raster cache内部

Debug确认：

```text
<=8MiB
```

不以WorkingSet猜。

---

# 263. First formula memory

记录：

```text
before first math
after first math
```

用于测font load。

---

# 264. Font retained memory

切回Source后：

KaTeX font global cache可能仍存在。

这是上游设计。

记录。

---

# 265. 不为了强制卸载font hack upstream global cache

除非实测成为重大问题。

---

# 266. Binary Size

记录：

```text
Phase5 exe
Phase6 exe
delta
```

---

# 267. Binary delta review trigger

如果：

```text
+8 MiB以上
```

必须分析。

这不是自动FAIL产品contract，但必须 `CONDITIONAL` review。

---

# 268. Portable ZIP仍有总hard target

长期：

```text
<=30MiB
```

保持关注。

---

# 269. Math Performance Bench

Release。

---

# 270. Cold first formula

测：

```text
font init
parse
layout
raster
total
```

---

# 271. Warm simple formula

```text
x^2
```

至少100次。

---

# 272. Warm complex formula

例如：

```text
matrix + fractions + delimiters
```

---

# 273. Cache hit

重复相同formula。

---

# 274. Benchmark阶段

表：

```text
parse
RaTeX layout
to DisplayList
font load
raster
total
```

---

# 275. Cold 与 Warm必须分开

---

# 276. 工程目标

非对外承诺。

简单formula warm：

```text
target <5ms
```

复杂formula warm：

```text
target <20ms
```

---

# 277. Cold first formula

```text
target <200ms
```

如果超过：

先分析font loading。

---

# 278. Preview whole-doc benchmark

至少：

### 20 KiB +20 math

### 100 KiB +100 math

### 1 MiB +500 math

---

# 279. 总 Preview hard gates仍沿Phase5：

```text
20KiB p95 <=100ms
100KiB p95 <=400ms
1MiB p95 <=2s
```

数学文档可能更慢。

因此额外报告：

```text
plain fixture
math fixture
```

---

# 280. 如果数学fixture超过旧plain门槛

不要立刻FAIL。

看：

```text
UI thread responsiveness
math worker cost
```

若背景完成且仍合理，可 `CONDITIONAL`。

---

# 281. UI typing during math build

在Split中启动复杂Preview。

同时Source输入。

必须测：

```text
input p95
```

目标仍：

```text
<=50ms at 1MiB
```

---

# 282. Worker monopolizing CPU

单worker可满一个core短时间。

允许。

但不能阻塞UI。

---

# 283. 不增加额外公式线程池

一个Preview worker。

---

# 284. Idle CPU

数学Preview稳定后：

```text
<0.1%
```

---

# 285. Math动画

无。

---

# 286. 不持续重raster

---

# 287. Visual Manual Acceptance

Phase5遗留视觉 `NOT TESTED`。

Phase6必须建立/执行真实数学视觉矩阵。

---

# 288. 如果Agent无视觉能力

不能写PASS。

必须：

```text
NOT TESTED — visual inspection unavailable
```

---

# 289. 这种情况Recommendation

如果所有自动化正确：

```text
APPROVE Phase 7 WITH CONDITIONS
```

不能无条件APPROVE。

---

# 290. Visual Matrix

至少：

```text
MATH-VIS-001 simple inline
MATH-VIS-002 superscript/subscript
MATH-VIS-003 fraction
MATH-VIS-004 nested fraction
MATH-VIS-005 square root
MATH-VIS-006 large delimiters
MATH-VIS-007 sum/integral
MATH-VIS-008 matrix
MATH-VIS-009 cases
MATH-VIS-010 Greek/mathbb
MATH-VIS-011 mixed Chinese + inline
MATH-VIS-012 display centered
MATH-VIS-013 table math
MATH-VIS-014 malformed error box
MATH-VIS-015 125% DPI
MATH-VIS-016 150% DPI
MATH-VIS-017 200% DPI
```

---

# 291. Visual PASS标准

重点：

```text
无glyph缺失
无公式被切掉
fraction bars连续
radicals正确
sup/sub位置合理
baseline合理
matrix对齐
large delimiter高度合理
```

---

# 292. 不要求与LaTeX像素完全相同

标准是：

```text
RaTeX/KaTeX-compatible geometry
```

---

# 293. Painter Oracle

自动化可以比视觉更客观地证明：

StickyMD painter与upstream reference接近。

---

# 294. Test Formula Count

建议至少：

```text
50 representative formulas
```

自动化。

---

# 295. 如果成本低

可扩到：

```text
100+
```

---

# 296. 不复制整个KaTeX测试库

只挑代表fixture。

---

# 297. Upstream bug separation

发现formula语义bug：

先建立最小：

```text
RaTeX direct parse/layout reproduction
```

---

# 298. 如果 direct RaTeX也错

这是：

```text
upstream issue
```

不是 StickyMD painter bug。

---

# 299. 如果 direct正确、StickyMD错误

则：

```text
adapter/painter/integration bug
```

---

# 300. 报告必须区分

---

# 301. Fuzz-like Test

固定seed生成：

```text
small math expressions
braces
sup/sub
fractions
commands
```

至少：

```text
10,000 cases
```

目标：

```text
no StickyMD panic
```

---

# 302. 但随机垃圾不应期待 parse success

Err正常。

---

# 303. Pathological Input

测试：

```text
64KiB braces
深nested
大量 superscript
large matrices
```

确认guard。

---

# 304. Preview Error Isolation

文档：

```text
good math
bad math
good math
```

必须：

```text
good
error box
good
```

---

# 305. One bad formula不能使whole Preview失败

hard invariant。

---

# 306. Math style与Markdown style

例如：

```markdown
**$x^2$**
```

公式本身不需要Markdown bold叠加。

RaTeX source控制math font。

---

# 307. Link containing math

如果AST支持：

link hitbox应包含formula区域。

---

# 308. Formula in heading

heading font size变化时：

math raster size同步。

---

# 309. Cache Key因此必须包含 effective math size

---

# 310. Formula in table

table cell字号如正文。

---

# 311. Formula in code

不渲染。

---

# 312. Formula in HTML literal

不渲染。

---

# 313. Formula copy across heading

正确raw source。

---

# 314. External Reload

Phase4 clean external reload含公式：

如果Preview/Split：

立即math rebuild。

---

# 315. Conflict

仍只渲染local DocumentState。

---

# 316. Load External

math cache可以复用相同formula。

---

# 317. Autosave

Math Preview不能影响save source。

---

# 318. Save永远仍来自DocumentSnapshot

不是Math AST。

---

# 319. Document generation

Math render：

不得改变。

---

# 320. Mode change

不得改变generation。

---

# 321. Math Cache hit

不得改变generation。

---

# 322. Theme/DPI

不得改变generation。

---

# 323. Preview selection

不得改变generation。

---

# 324. Diagnostics Privacy

不得log：

```text
formula source
```

---

# 325. 可以log：

```text
formula byte len
node index
error category
generation
```

---

# 326. ParseError detail

Debug报告可以：

```text
error kind
offset
```

但不要记录完整formula。

---

# 327. MathError用户信息

简单。

---

# 328. Dependencies Audit

创建：

```text
docs/report/phase-06-dependency-delta.md
```

---

# 329. 每个新增crate

记录：

```text
name
exact version
license
purpose
default features
selected features
transitive dependency count
binary impact
runtime impact
replaceability
```

---

# 330. 特别列出

```text
ratex-parser
ratex-layout
ratex-types
ratex-font-loader
ratex-katex-fonts
ratex-unicode-font
ab_glyph / ttf-parser if direct
ratex-render if dev-only
```

---

# 331. 不允许production出现

```text
ratex-svg
ratex-pdf
ratex-wasm
ratex-cairo
ratex-gtk4
ratex-ffi
```

除非有非常具体技术理由。

默认：

```text
FAIL dependency review
```

---

# 332. Cargo Tree Commands

```bash
cargo tree -p stickymd-render
cargo tree -p stickymd-win
cargo tree -p stickymd-win -e normal
```

---

# 333. Dev-only Oracle检查

如果加入 `ratex-render`：

```bash
cargo tree -p stickymd-win -e normal | rg "ratex-render"
```

必须无输出。

---

# 334. PNG crate

如果只因dev oracle出现：

可以。

Production normal tree不得因math painter引入PNG encode/decode hot path。

---

# 335. No Web Check

```bash
cargo tree | rg \
"wry|webview|tauri|cef|chromium|wasmtime|node"
```

---

# 336. No MathJax/JS

代码search：

```bash
rg \
"MathJax|KaTeX.*js|javascript|node" \
apps crates Cargo.toml
```

人工排除文档。

---

# 337. No external process

不得：

```text
Command::new("latex")
Command::new("pdflatex")
Command::new("xelatex")
```

---

# 338. No Network

Math完全离线。

---

# 339. THIRD_PARTY_NOTICES

必须更新。

---

# 340. RaTeX notice

至少记录：

```text
RaTeX
MIT
repository
version
```

---

# 341. KaTeX Font notice

保留：

```text
SIL OFL 1.1
```

---

# 342. 如果 adapted painter

增加：

```text
RaTeX ratex-render renderer implementation attribution
```

---

# 343. 不声称字体是 MIT

这是错误。

---

# 344. README

更新项目当前状态：

```text
Native Markdown Preview + RaTeX math rendering implemented in development.
Images and desktop docking are not yet complete.
```

不声称 v1发布。

---

# 345. Feature Doc

更新数学用户行为：

```text
KaTeX-compatible
4 delimiters
error fallback
```

---

# 346. Plan

`06_markdown_math_rendering.md`：

补入已验证：

```text
RaTeX exact version
adapter boundary
DisplayList painter boundary
font embedding
cache
baseline model
error fallback
```

---

# 347. 不把exact patch-level版本变成永久产品语义

版本是 implementation baseline。

---

# 348. Architecture Overview

更新：

```text
MathNode
↓
RaTeX Parser
↓
Layout
↓
DisplayList
↓
MathPainter
↓
MathRaster
↓
Preview Inline/Block Box
```

---

# 349. Coverage Matrix

AC-014：

Phase6完成后目标：

```text
full automated implementation PASS
manual visual maybe NOT TESTED
```

---

# 350. AC-015 Math Error

Phase6应完整推进。

---

# 351. Acceptance Matrix

新增：

```text
AC-014 Math Delimiters + Rendering
AC-015 Math Error
```

实际code path。

---

# 352. Phase6 Task

创建：

```text
docs/tasks/phase-06-ratex-native-math.md
```

---

# 353. Task结构

至少：

```text
Status
Prerequisites
Inherited Conditions
Preflight Baseline
Scope
Out of Scope
RaTeX Version
Dependency Strategy
Math Adapter
Painter
Font Strategy
Layout Integration
Cache
Selection
Error Handling
Resource Guards
Performance
Manual Verification
Risks
Result
```

开始：

```text
Status: In Progress
```

---

# 354. 完成：

```text
Status: Completed — awaiting USER review
```

如果manual visual未测：

```text
Status: Implementation Complete — manual verification incomplete
```

更诚实。

---

# 355. Phase6 Report

创建：

```text
docs/report/phase-06-ratex-native-math.md
```

---

# 356. Report Executive

必须：

```text
RaTeX Parser:
PASS / CONDITIONAL / FAIL

RaTeX Layout:
PASS / CONDITIONAL / FAIL

Direct Native Painter:
PASS / CONDITIONAL / FAIL

Math Fonts:
PASS / CONDITIONAL / FAIL

Inline Math:
PASS / CONDITIONAL / FAIL

Display Math:
PASS / CONDITIONAL / FAIL

Baseline Alignment:
PASS / CONDITIONAL / FAIL

Math Error Isolation:
PASS / FAIL

Math Selection/Copy:
PASS / CONDITIONAL / FAIL

Math Cache:
PASS / CONDITIONAL / FAIL

Memory:
PASS / CONDITIONAL / FAIL / NOT TESTED

Idle CPU:
PASS / CONDITIONAL / FAIL / NOT TESTED

Visual:
PASS / CONDITIONAL / FAIL / NOT TESTED
```

---

# 357. Preflight Baseline

列Phase5真实值。

---

# 358. Dependency Strategy

明确：

```text
production ratex crates
dev-only crates
ratex-render production? yes/no
why
```

---

# 359. Painter Strategy

明确：

```text
upstream direct API
```

或：

```text
StickyMD thin DisplayList painter
```

---

# 360. 如果 thin painter

报告：

```text
LOC
supported DisplayItem variants
upstream attribution
oracle test
```

---

# 361. Font Evidence

记录：

```text
embed fonts feature
first load
font subset
OFL notice
```

---

# 362. Formula Coverage

表至少：

```text
fractions
roots
sup/sub
operators
delimiters
matrices
cases
aligned
fonts
text
CJK text
accents
colors
```

实际 PASS/FAIL。

---

# 363. 不支持项

如果是 RaTeX upstream limitation：

明确：

```text
UPSTREAM LIMITATION
```

---

# 364. Error Evidence

malformed formula matrix。

---

# 365. Cache Evidence

```text
layout hits/misses
raster hits/misses
evictions
max bytes
```

---

# 366. Repeated Formula Evidence

100 × same formula。

---

# 367. Resize Evidence

100 resize：

```text
math parse delta
math layout delta
raster delta
```

---

# 368. Scroll Evidence

1000 scroll。

---

# 369. DPI Evidence

100/125/150/200%。

---

# 370. Baseline Evidence

mixed text + formula。

---

# 371. Performance Table

至少：

| Formula | Cold Parse/Layout/Raster | Warm p50 | Warm p95 | Cache Hit |
|---|---:|---:|---:|---:|
| simple | | | | |
| fraction | | | | |
| matrix | | | | |
| complex | | | | |

---

# 372. Document Math Performance

| Fixture | Plain Phase5 | Math Phase6 | Delta |
|---|---:|---:|---:|
| 20KiB +20 | | | |
| 100KiB +100 | | | |
| 1MiB +500 | | | |

---

# 373. Memory Table

至少：

| State | PWS median | PWS max | Private/Commit |
|---|---:|---:|---:|
| Source | | | |
| Preview no math | | | |
| Preview 1 math | | | |
| Preview 20 math | | | |
| Split 20 math | | | |
| Preview 200 unique | | | |
| Source after math | | | |

---

# 374. Binary Size

```text
Phase5 exe
Phase6 exe
delta
```

---

# 375. Idle CPU

```text
Source
Preview math
Split math
```

60秒。

---

# 376. Visual Matrix

真实结果。

不能自动化代替。

---

# 377. Architecture Authority

必须回答：

```text
Who owns formula source?
Who determines delimiters?
Who determines math semantics?
Who determines math geometry?
Who paints DisplayList?
Can math result mutate DocumentState?
Can math cache become authority?
```

正确：

```text
source → DocumentState
delimiter → Comrak
math semantics/layout → RaTeX
painting → StickyMD adapter
authority → none below DocumentState
```

---

# 378. Unsafe

必须：

```text
stickymd-core = 0
stickymd-render = 0
```

---

# 379. Math painter也不需要unsafe

tiny-skia / ab_glyph足够。

---

# 380. Windows API

Phase6理想：

```text
None added.
```

---

# 381. Architecture Drift

如果没有：

```text
None.
```

---

# 382. Risk Conditions

以下任一出现必须单独Risk Report：

### R1

RaTeX不能稳定输出所需核心公式。

### R2

direct native painter无法正确复现DisplayList。

### R3

必须依赖PNG hot path才能工作。

### R4

数学字体使binary/runtime memory明显越界。

### R5

inline formula无法集成现有line layout而不重构主骨架。

### R6

RaTeX产生不可隔离panic。

---

# 383. Risk文件

例如：

```text
docs/report/phase-06-ratex-painter-risk.md
docs/report/phase-06-inline-layout-risk.md
```

---

# 384. 不偷偷换数学引擎

如果 RaTeX有结构问题：

停止。

不要改：

```text
MathJax
KaTeX JS
LaTeX executable
```

---

# 385. Review Subagents

如果支持，最多3个。

### Reviewer 1

```text
RaTeX API / parser / layout / upstream boundary
```

### Reviewer 2

```text
DisplayList painter / baseline / visual geometry / cache
```

### Reviewer 3

```text
memory / performance / licenses / architecture authority
```

---

# 386. Self Review

必须回答：

1. delimiter是否仍完全由Comrak决定？
2. 是否出现自写TeX解析？
3. RaTeX类型是否泄漏core？
4. ratex-render是否进入production？
5. 是否存在PNG encode/decode hot path？
6. 是否duplicate tiny-skia？
7. Painter是否只画DisplayList？
8. 是否覆盖所有DisplayItem variant？
9. Inline math是否真正baseline-aligned？
10. Formula是否atomic line box？
11. Formula是否可能被行中间拆分？
12. Display math是否保持viewport稳定？
13. Bad formula是否只影响自身？
14. Formula source copy是否精确保留delimiter？
15. Resize是否不重新parse math？
16. Scroll是否不重新raster？
17. Cache是否bounded？
18. Raster cache是否<=8MiB？
19. Source-only是否lazy load math fonts？
20. Font license是否正确？
21. 是否把KaTeX fonts误写成MIT？
22. 是否引入WebView/JS？
23. 是否引入SVG/HTML intermediate？
24. Math Preview是否影响Autosave authority？
25. Phase5 Source/IME是否回归？

---

# 387. Automated Baseline

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

cargo deny check

git diff --check
```

---

# 388. Project Smoke

运行：

```text
tools/smoke/all.ps1 -Ci
```

若现有。

建立/更新：

```text
tools/smoke/phase-06.ps1
```

---

# 389. Phase6 Smoke至少支持

例如：

```powershell
tools/smoke/phase-06.ps1
tools/smoke/phase-06.ps1 -Performance
tools/smoke/phase-06.ps1 -Runtime
```

按现有工具风格。

---

# 390. Cargo Tree Forbidden

```bash
cargo tree -p stickymd-win -e normal | rg \
"ratex-svg|ratex-pdf|ratex-wasm|ratex-cairo|ratex-gtk4|ratex-ffi"
```

预期无。

---

# 391. ratex-render Production Check

```bash
cargo tree -p stickymd-win -e normal | rg "ratex-render"
```

除非上游已有合适direct painter且经过审计。

默认预期：

```text
无
```

---

# 392. Web Check

```bash
cargo tree | rg \
"wry|webview|tauri|cef|chromium|tokio|wgpu|reqwest|hyper"
```

---

# 393. Unsafe Scan

```bash
rg "\bunsafe\b" crates/stickymd-core
rg "\bunsafe\b" crates/stickymd-render
```

Runtime：

```text
0
```

---

# 394. Math Own Parser Scan

review：

```bash
rg \
"frac|sqrt|superscript|subscript|matrix|delimiter" \
crates/stickymd-render/src/math
```

确保没有数学布局重实现。

---

# 395. Runtime Formula Smoke

Release build，用单独portable目录。

至少输入：

```markdown
# 数学测试

Euler:
$e^{i\pi}+1=0$

\[
\int_0^1 x^2\,dx=\frac13
\]

\[
A=
\begin{pmatrix}
a&b\\
c&d
\end{pmatrix}
\]

\[
f(x)=
\begin{cases}
x^2,&x\ge0\\
-x,&x<0
\end{cases}
\]

坏公式：

\[
\frac{
\]
```

---

# 396. Smoke Expected

必须：

```text
good formulas render
bad formula fallback
app remains responsive
source unchanged
autosave unchanged
```

---

# 397. Mixed Typography Smoke

```markdown
这是一个行内公式 $x^2+y^2=1$ and this is English.
```

检查：

```text
CJK正文仍仿宋
Latin仍Times New Roman
Math独立KaTeX font
baseline一致
```

---

# 398. Split Smoke

连续编辑公式：

```text
1000ms 后更新
```

---

# 399. Preview-only Smoke

切Preview：

立即更新。

---

# 400. Mode Switch Memory

```text
Preview math
→ Source
```

确认：

```text
raster cache清理
```

---

# 401. Phase6 Definition of Done

只有全部成立才算完成：

- [ ] USER批准Phase6。
- [ ] Phase5 inherited conditions记录。
- [ ] Phase5 memory baseline在RaTeX前补测。
- [ ] Phase5 idle CPU在RaTeX前补测。
- [ ] Phase5基线无结构性阻塞。
- [ ] 当前RaTeX版本重新核实。
- [ ] RaTeX许可证重新核实。
- [ ] math crates最小化。
- [ ] RaTeX仅进入render层。
- [ ] core无RaTeX。
- [ ] ratex-render production使用经过显式审计。
- [ ] production无PNG encode/decode math hot path。
- [ ] tiny-skia duplicate已审计。
- [ ] Comrak仍拥有delimiter。
- [ ] 4类delimiter全部正式渲染。
- [ ] MathKind Inline/Display分离。
- [ ] Inline使用正确RaTeX math style。
- [ ] Display使用正确RaTeX math style。
- [ ] RaTeX parse正式集成。
- [ ] RaTeX layout正式集成。
- [ ] RaTeX DisplayList正式生成。
- [ ] 所有DisplayItem variants被Painter覆盖。
- [ ] GlyphPath正确。
- [ ] Line正确。
- [ ] dashed Line正确。
- [ ] Rect正确。
- [ ] Path正确。
- [ ] 数学字体portable embed。
- [ ] Font lazy loading。
- [ ] Source-only不主动加载math fonts。
- [ ] KaTeX font OFL notice完整。
- [ ] THIRD_PARTY_NOTICES更新。
- [ ] cargo deny license通过。
- [ ] Inline math成为atomic inline box。
- [ ] Inline math baseline alignment实现。
- [ ] Tall formula行高正确。
- [ ] Formula不会被行内拆开。
- [ ] Display math独立block。
- [ ] Display math正常center。
- [ ] Display overwidth安全。
- [ ] Table math工作。
- [ ] List math工作。
- [ ] Quote math工作。
- [ ] Heading math工作。
- [ ] Code math不渲染。
- [ ] HTML literal math不渲染。
- [ ] Math parse error isolated。
- [ ] Math layout error isolated。
- [ ] Math painter error isolated。
- [ ] raw formula fallback。
- [ ] 不自动修改公式。
- [ ] 64KiB单公式guard。
- [ ] 2000公式guard。
- [ ] raster allocation checked。
- [ ] single formula raster safety guard。
- [ ] Formula selection工作。
- [ ] Formula copy保留原source。
- [ ] Formula copy保留原delimiter。
- [ ] Preview Ctrl+A包含math source。
- [ ] Layout cache bounded。
- [ ] Raster cache<=8MiB。
- [ ] Raster cache eviction测试。
- [ ] Duplicate formula cache reuse。
- [ ] 500 unique formula cache测试。
- [ ] Source mode释放raster cache。
- [ ] DPI change仅必要raster invalidation。
- [ ] Resize不重新RaTeX parse/layout/raster。
- [ ] Scroll不重新RaTeX parse/layout/raster。
- [ ] Stale Preview仍绝对丢弃。
- [ ] 50+公式fixture自动化。
- [ ] malformed fixture。
- [ ] deep nesting fixture。
- [ ] upstream-vs-painter oracle或等价golden。
- [ ] 100% DPI测试。
- [ ] 125% DPI测试。
- [ ] 150% DPI测试。
- [ ] 200% DPI测试。
- [ ] Cold first formula性能测量。
- [ ] Warm formula性能测量。
- [ ] Math whole-document benchmark。
- [ ] Source typing duringmath build benchmark。
- [ ] First formula memory delta测量。
- [ ] 20 formula memory测量。
- [ ] Split math memory测量。
- [ ] 200 unique formula memory测量。
- [ ] Raster cache实际bytes验证。
- [ ] Source-after-math memory测量。
- [ ] Binary size delta测量。
- [ ] Idle CPU math Preview测量。
- [ ] Source IME无回归。
- [ ] Autosave无回归。
- [ ] External reload math更新正确。
- [ ] Conflict仍显示local formula。
- [ ] 不存在MathJax。
- [ ] 不存在KaTeX JS。
- [ ] 不存在LaTeX executable。
- [ ] 不存在WebView。
- [ ] 不存在SVG intermediate。
- [ ] 不存在PDF renderer。
- [ ] core unsafe=0。
- [ ] render unsafe=0。
- [ ] dependency report完成。
- [ ] license notices完成。
- [ ] coverage matrix更新。
- [ ] overview更新。
- [ ] Phase6 task完成。
- [ ] Phase6 report完成。
- [ ] 所有baseline command通过。
- [ ] Manual visual完成或诚实NOT TESTED。
- [ ] 未自动进入Phase7。

---

# 402. Final Recommendation

只有三种：

```text
APPROVE Phase 7
```

或：

```text
APPROVE Phase 7 WITH CONDITIONS
```

或：

```text
STOP — architecture review required
```

---

# 403. Phase7 预定方向

如果 Phase6通过：

下一阶段建议：

> **Managed Images, Clipboard Image Paste, Asset GC, Undo Asset Transactions & Export**

即：

```text
clipboard image
→ encoding preserve
→ SHA-256 managed asset
→ Markdown insertion
→ lazy Preview image
→ .trash
→ Undo/Redo asset restore
→ startup reconciliation
→ Export assets rewrite
```

但：

**不要自动执行。**

---

# 404. 最终回复格式

必须严格：

# Phase 6 Result

## Preconditions

```text
Phase 5 recommendation
USER approval
starting commit
inherited conditions
```

## Phase 5 Runtime Preflight

```text
Source memory:
Preview memory:
Split memory:

Source idle CPU:
Preview idle CPU:
Split idle CPU:
```

## Repository State Before Work

```text
branch
clean / dirty
```

## RaTeX Integration

```text
version
license
crates used
features
```

## Dependency Strategy

```text
ratex-render production?
dev-only?
tiny-skia versions?
duplicate versions?
```

## Math Pipeline

```text
Comrak MathNode
→ StickyMD MathNode
→ RaTeX parse
→ RaTeX layout
→ DisplayList
→ MathPainter
→ MathRaster
→ Preview
```

## Painter

列：

```text
GlyphPath
Line
Dashed Line
Rect
Path
```

各 PASS/FAIL。

## Font Strategy

```text
embedded?
lazy?
first-load?
OFL notice?
```

## Math Syntax

覆盖表。

## Inline Math

```text
baseline
line break
tall formula
mixed CJK/Latin
```

## Display Math

```text
centering
overwidth behavior
block spacing
```

## Error Handling

列 malformed fixture。

## Selection & Copy

```text
atomic selection
exact raw source
delimiter preservation
```

## Cache

```text
layout entries
raster bytes
hit/miss
evictions
duplicate formula
```

## Scheduling

```text
resize math rebuild count
scroll math rebuild count
stale result
```

## Performance

完整表。

## Memory

完整表。

## Binary Size

```text
Phase5
Phase6
delta
```

## Idle CPU

完整结果。

## Visual Verification

逐项：

```text
PASS
FAIL
NOT TESTED
```

## Acceptance

```text
AC-014
AC-015
```

## Dependencies Added

表。

## Licenses

```text
RaTeX MIT
KaTeX fonts OFL-1.1
notices
cargo deny
```

## Unsafe

```text
core = 0
render = 0
windows adapter = ...
```

## Architecture Authority

```text
Formula source authority:
Delimiter authority:
Math semantic authority:
Geometry authority:
Painter responsibility:
Cache authority:
```

## Architecture Drift

```text
None
```

或 Risk Report。

## Verification

所有命令 PASS/FAIL。

## Documentation

```text
task
report
coverage
overview
plan
dependency delta
third-party notices
```

## Git

```text
commit(s)
push = no
```

## Recommendation

三选一。

最后：

> Awaiting USER review. Do not start Phase 7 automatically.

完成后立即停止。
