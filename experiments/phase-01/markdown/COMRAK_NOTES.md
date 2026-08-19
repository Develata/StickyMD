# Comrak 0.54.0 行为笔记（Phase 1D）

> 实验性记录。本目录不属于生产 workspace，可随时删除。
> plan_ref: docs/plan/06_markdown_math_rendering.md

本文件记录 spike 实测到的 Comrak 0.54.0 解析行为，作为生产 `MarkdownProjection`
实现时的约束清单。所有结论均以本仓库 `fixtures/all.md` + release 二进制实测为准。

## 1. 启用的扩展

```rust
Options {
  extension: ExtensionOptions {
    math_dollars: true,   // $...$ 与 $$...$$
    math_latex: true,     // \(...\) 与 \[...\]
    table: true,          // GFM 表格
    tasklist: true,       // GFM 任务列表 - [x]
    strikethrough: true,  // GFM 删除线 ~~..~~
    ..
  },
  parse: ParseOptions { smart: false, .. },  // 保持字面，不做弯引号替换
  ..
}
```

## 2. Arena 所有权模型（关键约束）

- `parse_document(&arena, text, &options)` 返回 `&'a ArenaNode`，其生命周期 `'a`
  绑定到 `Arena`。**Comrak 的 AST 不能跨出 arena 存活。**
- 生产做法：spike 用 `to_owned()` 把需要的信息（tag / sourcepos / literal /
  math_kind / children）拷贝成自有 `SpikeNode` 树，随后丢弃 arena。
  实测 `arena dropped` 后自有树仍可完整遍历、sourcepos 保留。
- 生产 `MarkdownProjection` 必须同样做一次「arena → 自有结构」的投影拷贝，
  不能把 `&Arena` 或 `&Node` 泄漏到 DocumentState 之外。

## 3. 数学节点（NodeMath）分隔符折叠 ⚠ 重要限制

`NodeMath` 只有两个布尔：`dollar_math`、`display_math`。实测四种源分隔符的映射：

| 源写法 | dollar_math | display_math | 结论 |
| --- | --- | --- | --- |
| `$…$` | true | false | inline |
| `\(...\)` | **true**（被硬编码） | false | inline |
| `$$…$$` | true | true（`opendollars==2`） | display |
| `\[...\]` | **true**（被硬编码） | true | display |

根因：`comrak-0.54.0/src/parser/inlines.rs` 中
- `handle_dollars`（约 L1282-1283）设 `dollar_math: !code_math`、`display_math: opendollars==2`；
- `handle_latex_math`（约 L526）对 `\(...\)`/`\[...\]` **硬编码 `dollar_math: true`**。

**因此 `dollar_math` 对所有 math 节点恒为 true，无法区分 `$` 与 `\(`（或 `$$` 与 `\[`）。**
唯一可靠区分的是 **inline vs display**（`display_math`）。

生产影响：若需要回显/保留「源分隔符风格」（如 round-trip 原样、或按风格差异化渲染），
**不能**依赖 `NodeMath` 标志，只能通过 `sourcepos` 回到原始文本重扫首尾定界符。
当前 StickyMD 语义层只需要「这是 inline/display 数学，字面内容为 X」交给 RaTeX，
所以该折叠不影响正确性，但已记录为已知约束。

## 4. 原始 HTML 保持字面

- `NodeValue::HtmlBlock` / `NodeValue::HtmlInline` 的 `literal` 保留原始 HTML 文本。
- fixture 中 `<div class="raw-html">...</div>` 被完整捕获为单个 HtmlBlock literal
  （`@40:1-42:6`），**不会**被展开成子节点或被当作 markdown 解析。
- 生产渲染策略：StickyMD 无 WebView、不执行 HTML；raw HTML 应按「字面占位」处理
  （显示原文或占位块），避免任何解析/执行。

## 5. sourcepos 保留

- 每个节点的 `sourcepos`（`start.line/col..end.line/col`，1-based）在投影后完整保留，
  可用于：math 分隔符风格回扫（§3）、渲染区域 ↔ 源行映射、诊断定位。

## 6. 代码块 / 行内代码保持字面

- `CodeBlock.literal` / `Code.literal` 保留原文，不做语法高亮解析（高亮不在 Phase 1 范围）。

## 7. 复现

```powershell
cd experiments/phase-01/markdown
cargo run --release
```
