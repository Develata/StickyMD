# 表格绝对值公式与源码坐标修复

日期：2026-09-08。基线：`main`，`HEAD` 与本地 `origin/main` 均为
`37fa6e0b6da3ae83c16bb29a42b605ad078ac3c2`；工作树包含上一轮尚未提交的维护修改。
本报告只记录本次表格问题，不赋予工作树 `v0.1.0` exact-artifact 身份。

## 已核实事实

用户截图中的表头含 `$|\mathcal N_i|$`。将其放入六列表头、配六列分隔行即可复现：
两个未转义的 `|` 被计算为分列符，整张表成为普通段落。这符合
[GFM 表格规则](https://github.github.com/gfm/#tables-extension-)：行内内容中的竖线仍须转义，
表头和分隔行列数须一致。`$\lvert\mathcal N_i\rvert$` 与
`$\left\lvert\mathcal N_i\right\rvert$` 均能生成六列表并完成公式栅格化。

另一个可复现的实现缺陷是：合法的 `$\|\mathcal N_i\|$` 能形成表格，复制原文却变成
`$\|\mathcal N_i\`，丢失末尾 `|$`。在公式之前加入转义竖线，也会使后续节点范围前移；
图片导出会错取相邻字符，数学分隔符转换可能漏掉公式。

根因已对照本地锁定的 Comrak 0.54.0 源码和 AST 坐标确认：它先移除表格单元格内
`\|` 的反斜杠，再解析行内节点，行内坐标仍来自缩短后的文本；单元格边界则来自原文。
原 `SourceMap` 直接将这些行内坐标当作 canonical UTF-8 字节坐标。

修复前新增的六组集成测试中，一组 GFM 行为测试通过，五组源码范围/复制/转换/导出测试失败。
连续反斜杠补充测试最初还过度要求普通转义文本节点覆盖所有原始反斜杠；经对照行内节点
坐标约定，改为验证这些反斜杠不会使后面的完整公式范围错位。被表格预处理删除的
反斜杠在范围起点是否被保留，由独立坐标边界测试验证。

## 修复与合同影响

- `preview/source_map.rs`：仅对 Comrak 已识别的单元格建立逆向字节坐标映射，恢复后再验证
  UTF-8 边界，并拒绝越出单元格的坐标。
- `preview/parser.rs`：映射只在转换当前单元格行内节点期间生效；退出该单元格即释放，
  不影响下一单元格、表格外文本或 canonical 文档。
- `tests/table_math_pipes.rs`：覆盖四种公式分隔符、表头/正文、中文/emoji、CRLF、缩进、
  引用/列表、连续反斜杠、预览复制、图片重写和分隔符转换。

单元格扫描复杂度为 O(单元格字节数)，每个范围恢复为 O(log(转义竖线数))；临时内存为
O(当前单元格转义竖线数)，受既有 5 MiB 源码上限约束，无转义时不分配该映射。
这些是代码复杂度结论，未测量本次改动的运行时间或进程内存差值。

沿用 `06_markdown_math_rendering.md` 的 Markdown semantics / owned AST projection 合同，
并验证 `07_editor_and_ime.md` 的分隔符转换及 `08_assets_and_export.md` 的导出消费者。
没有预解析公式、修改输入文本、替换 Markdown/TeX parser、增加依赖或改变 authority。
现有 Rust Phase 05 任务已包含整个 render crate 测试，因此 PowerShell 薄入口无需改动。

可选路径：表格绝对值推荐使用 `\lvert...\rvert`；也保留标准表格转义写法 `\|...\|`。
对未转义竖线作“公式内不分列”的特殊处理会改变已冻结的 GFM 语义，本次未采用。

## 实际验证

| 命令 | 结果 |
| --- | --- |
| `cargo test -p stickymd-render --locked` | 155 passed，0 failed，9 个专用 Release 性能测试按原有 ignore 规则未运行；含新增 7 个集成测试与 1 个坐标单元测试 |
| `cargo test -p stickymd-win --locked math_delimiter` | 2 passed，验证批量转换/选区转换的 generation 与单次 Undo 边界 |
| `cargo clippy -p stickymd-render --locked --all-targets -- -D warnings` | 通过 |
| `cargo check -p stickymd-win --locked` | 通过 |
| `cargo fmt --all -- --check` / `git diff --check` | 通过 |

## 基于证据的推断与尚未验证事项

截图表格退化为段落与最小复现一致，因此判断为未转义竖线引起的列数不匹配；
用户完整 Markdown 原文尚未取得，不能排除其中还存在其他格式问题。

真实 Windows 窗口的外观、鼠标复制、系统剪贴板与运行中程序的实际效果为 `NOT TESTED`。
未运行完整资格化、资源测量或发布流程，未替换已发布资产，也未 commit、push 或修改 tag。

## Resolution — 2026-09-08

新增报告及文档投影后，`cargo run -p stickymd-smoke --locked -- phase 00` 完成唯一的
governance contracts 任务并返回 `StickyMD smoke PASS: phase-00`。
