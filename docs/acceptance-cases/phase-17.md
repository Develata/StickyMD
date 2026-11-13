# Phase 17 Acceptance Matrix

本矩阵验证下一版本的复制语义、复制侧条、提示条与文末留白
（[`11` Phase 17 节](../plan/11_testing_and_release.md#phase-17-copy-and-notices)）。

- 契约：`02` #copy-what-you-see、`04` #notice-state、`06` #preview-copy-semantics 与 #preview-copy-rail、`09` #notices 与 #vertical-scrollbars。
- 依据：[提议报告](../report/2026-11-14-copy-semantics-rail-notices.md)，USER 批准 2026-11-14。
- 设计已完成，代码尚未实现，所有行均为 `NOT TESTED`。它不是资格化 suite，不产生 exact-candidate 证据。

## M1 提示条

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P17-N01 | 瞬时提示到期或被替换后，露出持续层当时派生出的提示（待决定 > 持续故障 > 进行中） | Automated | 待建：提示状态单元测试 | NOT TESTED |
| P17-N02 | 时长：成功 3 s、提醒 5 s、重要提醒 30 s；持续故障文字 30 s 后收起，标记保留到 `SaveState` 离开 `Failed`；待决定不过期 | Automated | 待建：注入时钟的时长测试 | NOT TESTED |
| P17-N03 | 到期回执的 id 与当前提示不一致时被忽略；被替换提示的定时器不影响新提示 | Automated | 待建 | NOT TESTED |
| P17-N04 | 成功不替换正在显示的提醒或重要提醒；提醒与重要提醒可以替换成功 | Automated | 待建 | NOT TESTED |
| P17-N05 | 防抖、失焦、收进托盘触发的保存，不产生进行中或成功提示；手动 Ctrl+S 成功显示“已保存”3 s | Automated | 待建 | NOT TESTED |
| P17-N06 | 保存失败与持久化工作线程停止都进入 `SaveState::Failed`，显示持续故障；之后保存成功即解除 | Automated | 待建 | NOT TESTED |
| P17-N07 | 待决定提示按可用选项给出按钮：两个选项两个按钮，只有一个选项一个按钮；不查找文字中的 `[F7`；F6/F7 按键语义不变 | Automated | 待建 | NOT TESTED |
| P17-N08 | 进行中提示（导出、图片准备、保留本地、手动保存、恢复）由同一操作的结果结算 | Automated | 待建 | NOT TESTED |
| P17-N09 | 现有发出点按报告附录归类，没有未归类的发出点；未归类的技术性失败按重要提醒 | Automated | 待建：发出点归类测试 | NOT TESTED |
| P17-N10 | 提示条在 Source、Preview、Split 都可见，Split 下横跨两个窗格；提示变化不触发 Source 或 Preview 的重新排版 | Automated | 待建 | NOT TESTED |
| P17-N11 | 点击工具栏标记，重新显示持续故障文字，并重新计 30 s | Automated | 待建 | NOT TESTED |
| P17-N12 | 视觉：浮层位置与对比度；Light/Dark；40% 透明度；Source 的最后一行可被瞬时提示暂时盖住（USER 接受） | Manual | macOS 在 `mac-test` 虚拟机中，Windows 在宿主上 | NOT TESTED |

## M2 Preview 文末留白

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P17-E01 | 文档高 ≤ 视口高 − 提示条高：不滚动，滑块隐藏 | Automated | 待建 | NOT TESTED |
| P17-E02 | 否则可滚范围为 `文档高 + R − 视口高`，`R = max(视口高 / 2, 提示条高)`；绘制、滚轮、滑块比例与隐藏、视图切换使用同一末端 | Automated | 待建 | NOT TESTED |
| P17-E03 | Split 同步在留白区不回跳，也不越过 Source 的末端 | Automated | 待建 | NOT TESTED |
| P17-E04 | Source 的末端行为不变 | Automated | 现有 Source 滚动测试保持通过 | NOT TESTED |

## M3 Preview 复制语义（第一期）

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P17-C01 | 只选中一个公式（块级或行内），复制不带分隔符的 TeX | Automated | 待建 | NOT TESTED |
| P17-C02 | 选区在一个代码块或 Raw HTML 块内部：复制选中的代码文本，不带围栏，也不带引用或列表前缀 | Automated | 待建 | NOT TESTED |
| P17-C03 | 只选中一张图片，复制 alt 文字（现行行为不变） | Automated | 现有 `contracts.rs:92` 保持通过 | NOT TESTED |
| P17-C04 | 跨对象选区复制源码切片，保留 `**`、`#`、链接的方括号与圆括号、列表符与引用符 | Automated | 待建 | NOT TESTED |
| P17-C05 | 切开围栏代码块：复制开围栏行、选中的行、闭围栏行；没有闭围栏时原样切片 | Automated | 待建 | NOT TESTED |
| P17-C06 | 切开行内结构：复制整个结构；蓝框不变 | Automated | 待建 | NOT TESTED |
| P17-C07 | 在落后的显示帧上复制已有选区，读该帧自己的源码快照，不读当前文档 | Automated | 待建 | NOT TESTED |
| P17-C08 | Ctrl+A：只含一个公式的文档得到不带分隔符的 TeX；混合文档得到对应的源码 | Automated | 待建 | NOT TESTED |
| P17-C09 | 位置不可信时：位置上移的段落吸附整段；转义、实体与软硬换行按 span 边界取整；列表序号吸附到所属项起点；得不到合法边界时不写剪贴板并报复制失败 | Automated | 待建 | NOT TESTED |
| P17-C10 | 不变式：固定种子的随机选区下，跨对象结果的每个字节都取自该帧源码快照，并保持源码顺序 | Automated | 待建：固定种子的性质测试 | NOT TESTED |
| P17-C11 | 复制成功显示“已复制”3 s；失败显示提醒，不修改文档或选区 | Automated | 待建 | NOT TESTED |
| P17-C12 | 资源：复制映射带来的 Preview 构建时间与内存变化在 `10` 的预算之内 | Automated | 待建：基线与新包对比 | NOT TESTED |

## M4 复制侧条

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P17-R01 | 侧条只出现在代码块、Raw HTML 块与块级公式的背景右缘，与背景同高，不含横向滚动条轨道 | Automated | 待建 | NOT TESTED |
| P17-R02 | 侧条不改变排版，不与正文、背景、滚动条重叠；整根不在内容区内时不画 | Automated | 待建 | NOT TESTED |
| P17-R03 | 点击复制该对象的完整内容（公式不带分隔符）；命中优先于开始选区；按下后拖动超过 3 DIP，既不复制也不开始选区 | Automated | 待建 | NOT TESTED |
| P17-R04 | 悬停变亮并切换光标，只在进出侧条时重绘 | Automated | 待建 | NOT TESTED |
| P17-R05 | 在落后的显示帧上点击侧条，复制该帧显示的内容 | Automated | 待建 | NOT TESTED |
| P17-R06 | 视觉：宽度（暂定 8 内容 DIP，验收后固定）、低对比、Light/Dark、40% 透明度、高缩放与窄窗下的隐藏 | Manual | macOS 在 `mac-test` 虚拟机中，Windows 在宿主上 | NOT TESTED |

## 落地时更新

Phase 17 落地时，下列现行条款、验收行与测试随之改写。在此之前它们保持有效，不标记 `Deprecated`。

- **Plan：**
  - `06` 选择规则中复制读显示投影与公式复制带分隔符的两句、横向滚动一节的“复制读取完整 projection”；
  - `07` 复制反馈中“成功复制不显示”的部分；
  - `09` 中“用户可见的错误仍是诊断文字”。
- **Features：** `docs/features/00_v1_product_behavior.md` 中标注“下一版本起改变”的三处。
- **验收行：**
  - `00_v1_acceptance.md` 中关于公式复制保留源码的描述（约第 672–674、689 行）；
  - `phase-03.md` 中复制成功不改变呈现的说明（约第 26–30 行）；
  - `phase-05.md` 的 P05-A12、P05-M02，以及表格与引用中复制公式的案例（约第 117 行）。
- **测试：**
  - `crates/stickymd-render/src/preview/pipeline/horizontal_tests.rs:154`：全选单个块级公式，从 `$$x^2$$` 改为 `x^2`；
  - Preview 复制与剪贴板反馈的现有测试（`phase-03`、`phase-05`、`phase-10` P10-A10、`phase-14` P14-A28 所引）逐项复核。
