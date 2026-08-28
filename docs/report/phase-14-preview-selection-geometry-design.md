# Phase 14 Preview Selection Geometry Design

## Status

Implemented; automated headless verification and the Release performance baseline pass. A new exact
candidate and its human visual/IME receipt remain pending.

## Problem

当前 Preview 排版已经通过 Cosmic Text 得到每个 `LayoutGlyph` 的 byte range、x、width 与 BiDi level，
但随后把 glyph 聚合成粗粒度 `PreviewTextBox`，丢弃 cluster boundary。选择层又按整段 grapheme 数量
占宽度的比例估算命中和蓝框。该近似对等宽 ASCII 偶尔看似正确，对 Times、CJK、Emoji、组合字符、
连字、自动换行与 BiDi 必然产生偏移。

因此不能靠增加容差、换成另一种字符计数或等待上游修复解决；上游已有所需几何，缺陷在 StickyMD
自己的降级投影。

## Chosen Design

```text
Owned/Render layout
    ├─ PreviewDocumentProjection
    │    generation + Arc<display text> + semantic scroll anchors
    └─ PreviewFrameGeometry
         visible rows + merged shaping clusters + atomic objects + actions
                  ↓
             PreviewFrame
                  ↓ semantic API only
       hit / highlight / copy / action / tooltip
```

完整文本和语义 anchor 与当前帧几何分权。Frame geometry 只为 viewport 与既有 overscan 建立；滚动、
resize、zoom 或 generation 改变时替换，selection/hover 变化时复用。App 不接触 Cosmic Text buffer、
glyph 或 cluster 数组。

每个文本 block 另保留固定宽度的 visual-row locator（logical line、layout row、top、height、logical
byte base）。它不复制 glyph/cluster，只让 viewport 构建通过 y 二分直达 Cosmic Text 已有 layout
row；否则一个数百行 code block 会在每次滚动帧线性扫描全部 visual rows，和 viewport-only 目标冲突。

最小 cluster 记录使用紧凑值类型：global selection byte start/end、leading/trailing x；行统一持有 y、
height 与 cluster slice。相同 shaping cluster range 的多个 glyph 合并，避免按 glyph 重复内存。RTL 由
leading/trailing 顺序表达，不另建平行索引。link action 与 tooltip payload 使用共享所有权，避免一个
长 URL 因可见字符数被重复复制。

## Rejected Alternatives

- **继续按 grapheme 比例估算**：低内存但不正确，无法处理变宽字形、连字与 BiDi。
- **永久保存全文 `LayoutGlyph`**：实现直接但耦合 Cosmic Text，长文内存随全部 glyph 增长，App 易
  依赖渲染内部类型。
- **重新引入 DirectWrite/HarfBuzz/Pango**：重复 shaping authority、新依赖和跨平台/unsafe 成本均无
  必要。
- **等待 Cosmic Text 上游**：上游已经提供 cluster 数据，不能修复 StickyMD 主动丢弃数据的问题。
- **自行实现 grapheme/ligature caret 算法**：复杂且不可靠；cluster 内不可细分时应遵循现有引擎。

## Complexity and Memory

- row locator build：O(block visual rows)，只在 block layout 时发生；geometry build：
  O(log block visual rows + visible glyphs)，与 viewport paint 同阶。
- point hit：O(log visible rows + log clusters in row)。
- selection paint：O(intersecting visible clusters)，跨行自然输出多个 rectangles。
- document projection：一份 `Arc<str>`；layout 额外为 O(visual rows) 固定宽度 locator；frame geometry：
  O(viewport clusters)，典型窗口预计数十 KiB。
- 不新增 runtime dependency，不增加 unsafe，不创建第二份 canonical text。

## Failure and Validation

所有 byte range 必须是 projection text 的 UTF-8 boundary 且在 generation/viewport key 内。非法映射
不得进入 frame；旧 frame 可保留或显示 skeleton，但不能 panic、越界复制或修改 Document。

自动化至少覆盖：Times/CJK/Emoji/combining/ligature、soft wrap、代码块/raw HTML 多逻辑行、反向与
跨行选择、BiDi 多矩形、atomic math/image、resize/zoom/scroll cache invalidation，以及 selection 改变
不触发 relayout。性能回归记录 viewport geometry bytes 与 hit/paint 时间；完整 exact candidate 仍需
重新资格化。

## Implementation Evidence

实现位于 `stickymd-render::preview::{text_layout,selection,paint,layout}`。`TextLayoutRow` 只保留固定宽度
visual-row locator；每个 `PreviewFrame` 从可见 block 的 Cosmic Text layout row 构造精确 cluster map，
App 只能通过 frame 的 semantic API 执行 hit、highlight、copy、link 与 tooltip。Source、DocumentState
和 Comrak/RaTeX authority 均未改变，Cargo dependency 与 unsafe delta 为零。

自动化结果：

- Preview targeted tests：64 passed、0 failed、5 ignored。
- `stickymd-win` tests：244 passed、0 failed、8 ignored。
- Phase 14 tests shard：PASS（governance、Phase 1 Markdown/math、Phase 1 persistence、workspace tests）。
- 5,000 visual rows 的 Release baseline：row locator 160,000 bytes；725 个 viewport clusters 的 frame
  geometry 87,000 bytes；projection median 15.1 µs、p95 19.1 µs、max 25.5 µs；10,000 次 point hit
  共 340.8 µs。hard guards 分别是 10 ms projection p95 与 20 ms/10,000 hits。

这些数据证明新增索引没有退化成全文 glyph cache 或滚动时线性扫描全文。真实输入法的 composition、
commit、cancel、Undo 与 Search 链路由 Phase 14 G4-06 exact automation 持有；候选窗是否出现、与 caret
的视觉距离、字体、遮挡、动画和 DPI 观感不能由 headless 测试代替，继续保持 `NOT TESTED`。
