# Phase 1D Spike — Markdown / Math（Comrak 0.54.0 + RaTeX 0.1.14）

> 实验性代码。本目录不属于生产 workspace，可随时删除。
> plan_ref: docs/plan/06_markdown_math_rendering.md ; docs/plan/10_performance_reliability.md

## 1. 验证目标（来自 Phase 1 任务 1D）

- Comrak 0.54.0（CommonMark + GFM + `math_dollars` + `math_latex`）解析 fixture，
  覆盖 4 种数学定界符与 raw HTML；验证 arena → 自有投影树的可行性（sourcepos 保留）。
- RaTeX 0.1.14（parser → layout → display-list → PNG）对每个数学字面量端到端渲染。
- 记录错误路径（未闭合括号）行为。
- 基准：markdown 解析 @ 20 / 100 / 1024 KiB；数学渲染单式延迟。

## 2. 环境

| 项 | 值 |
| --- | --- |
| OS | Windows 11 x64（构建机 20 逻辑核） |
| 工具链 | Rust 1.97.1（MSVC） |
| 依赖 | comrak 0.54.0 / ratex-parser 0.1.14 / ratex-layout 0.1.14 / ratex-render 0.1.14(`embed-fonts`) |
| 构建 | `cargo build --release`（独立 crate，`[workspace]` 空） |

## 3. 结果

### 3.1 Comrak 解析 + 投影：PASS

- fixture（865 B）解析 + `to_owned` 投影 = 0.22 ms，64 节点，arena 成功丢弃。
- 5 个数学字面量、1 个 raw HTML 字面量全部捕获；sourcepos 完整保留。
- 4 种源定界符（`$`、`$$`、`\(`、`\[`）均被解析为 Math 节点并渲染成功。
- ⚠ 定界符风格折叠：`NodeMath.dollar_math` 对所有 math 节点恒为 true（Comrak
  inlines.rs 硬编码），**仅 `display_math` 可区分 inline/display**。详见 `COMRAK_NOTES.md §3`。
  不影响 StickyMD 语义（只需 inline/display + 字面量），已记录为已知约束。

### 3.2 RaTeX 渲染：PASS（5/5）

| # | 类型 | 字面量 | DisplayList 项 | 盒（em 近似） | PNG |
| --- | --- | --- | --- | --- | --- |
| 0 | inline | `E = mc^2` | 5 | 3.8×0.9 | 4406 B |
| 1 | inline | 二次公式 | 16 | 8.9×1.6 | 10932 B |
| 2 | display | 高斯积分 | 14 | 7.8×1.4 | 11164 B |
| 3 | inline | 欧拉恒等式 | 7 | 4.7×0.9 | 4328 B |
| 4 | display | 巴塞尔级数 | 14 | 5.4×1.7 | 10187 B |

`parse → layout → to_display_list → render_to_png` 全链路成功，`embed-fonts` 无需外部字体文件。

### 3.3 错误路径：PASS（可恢复，非 panic）

`\frac{` → `Err(ParseError { "Unexpected end of input in a macro argument", loc: 6..6 })`。
RaTeX 返回结构化错误（含源码位置），生产侧可降级为「显示原文 + 错误标记」，不崩溃。

### 3.4 性能基准（release，24 次取中位 / p95 / max；峰值分配由计数 allocator 统计）

| 文档 | 大小 | median | p95 | max | 节点数 | 峰值分配 |
| --- | --- | --- | --- | --- | --- | --- |
| 20 KiB | 20574 B | 2.45 ms | 2.54 ms | 2.62 ms | 3321 | ≈1988 KiB |
| 100 KiB | 102478 B | 11.45 ms | 12.30 ms | 12.70 ms | 16481 | ≈13037 KiB |
| 1024 KiB | 1048640 B | 126.47 ms | 189.97 ms | 190.25 ms | 167961 | ≈114265 KiB |
| 数学渲染（单式） | — | 0.75 ms | 1.23 ms | 1.25 ms | — | — |

观察：
- 解析耗时随文档近似线性（1024 KiB ≈ 20 KiB 的 ~52×，节点数 50×）；峰值分配与节点数同阶。
- 单式数学渲染 < 1.3 ms（p95），满足「按需渲染、缓存位图」策略的预算。
- 1024 KiB 全量重解析 ~126 ms（median），超出单帧预算 → 生产必须用增量/视口裁剪
  （见 plan 10），不能在每次按键全量重解析巨型文档。此为 Phase 2+ 的设计约束，已记录。

## 4. 结论

| 项 | 判定 |
| --- | --- |
| Comrak 解析 + arena→自有投影（sourcepos 保留） | **PASS** |
| 4 种数学定界符识别 | **PASS**（风格折叠已记录，见 §3.1） |
| raw HTML 保持字面 | **PASS** |
| RaTeX parse→layout→display→PNG | **PASS（5/5）** |
| 数学错误路径可恢复 | **PASS** |
| 解析/渲染性能预算（≤100 KiB） | **PASS** |
| 巨型文档（1024 KiB）全量重解析 | **需增量策略（生产约束，非阻塞）** |

判定：**PASS（附 2 项已知约束：定界符风格折叠、巨型文档需增量解析）**。
Comrak + RaTeX 组合可作为生产 MarkdownProjection + MathRenderer 的语义/渲染基础。

## 5. 复现

```powershell
cd experiments/phase-01/markdown
cargo run --release
```
