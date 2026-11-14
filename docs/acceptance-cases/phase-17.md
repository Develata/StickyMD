# Phase 17 Acceptance Matrix

本矩阵验证下一版本的复制语义、复制侧条、提示条与文末留白
（[`11` Phase 17 节](../plan/11_testing_and_release.md#phase-17-copy-and-notices)）。

- 契约：`02` #copy-what-you-see、`04` #notice-state、`06` #preview-copy-semantics 与 #preview-copy-rail、`09` #notices 与 #vertical-scrollbars。
- 依据：[提议报告](../report/2026-11-14-copy-semantics-rail-notices.md)，USER 批准 2026-11-14。
- 设计已完成，代码尚未实现，所有行均为 `NOT TESTED`。它不是资格化 suite，不产生 exact-candidate 证据。

## M1 提示条

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P17-N01 | 定时提示到期或被清除后，露出派生层当时的提示（待决定 > 进行中）；待决定只在没有已选择、执行中的操作时派生 | Automated | 待建：提示状态单元测试 | NOT TESTED |
| P17-N02 | 时长：成功 3 s、提醒 5 s、重要提醒 30 s、故障展示 30 s，按墙钟计、被遮挡时不暂停；待决定不过期 | Automated | 待建：注入时钟的时长测试 | NOT TESTED |
| P17-N03 | 到期回执的 id 与当前提示不一致时被忽略；被替换提示的定时器不影响新提示 | Automated | 待建 | NOT TESTED |
| P17-N04 | 入栈：提醒、重要提醒、故障展示总是压到栈顶；成功只在栈空时压入、栈顶为成功时替换，否则丢弃；成功可暂时盖住派生层 | Automated | 待建 | NOT TESTED |
| P17-N05 | 防抖、失焦、收进托盘触发的保存，不产生进行中或成功提示；手动 Ctrl+S 成功显示“已保存”3 s | Automated | 待建 | NOT TESTED |
| P17-N06 | 保存失败与持久化工作线程停止都设置 `unresolved_failure`；重试进入 `Saving` 时标记不消失；之后某次保存成功才清除 | Automated | 待建 | NOT TESTED |
| P17-N07 | 待决定提示按可用选项给出按钮：两个选项两个按钮，只有一个选项一个按钮；不查找文字中的 `[F7`；F6/F7 按键语义不变 | Automated | 待建 | NOT TESTED |
| P17-N08 | 进行中提示（导出、图片准备、保留本地、手动保存、恢复）由同一操作的结果结算 | Automated | 待建 | NOT TESTED |
| P17-N09 | 每个发出点按 `09` #notices 的归类规则归类，无法归类的按重要提醒；报告附录只作对照 | Automated | 待建：发出点归类测试 | NOT TESTED |
| P17-N10 | 提示条在 Source、Preview、Split 都可见，Split 下横跨两个窗格；提示变化不触发 Source 或 Preview 的重新排版 | Automated | 待建 | NOT TESTED |
| P17-N11 | 点击工具栏标记，开始一段新的 30 s 故障展示；标记在之后某次保存成功前一直显示 | Automated | 待建 | NOT TESTED |
| P17-N13 | 外部冲突中选“保留本地”，首次写入失败（或失败种类改变）：故障原因显示 30 s，同时保留 F6/F7 按钮；之后回到冲突提示 | Automated | 待建 | NOT TESTED |
| P17-N14 | 故障种类从 `Failed` 变为 `WorkerUnavailable`：开始新的一段并换新 id，旧段的到期回执被忽略 | Automated | 待建 | NOT TESTED |
| P17-N15 | 提示条高 34 窗口 DIP，不随 Content Zoom 缩放；最小窗口 × 300% 缩放时仍不超过内容视口 | Automated | 待建 | NOT TESTED |
| P17-N16 | 冲突待决定时，上方显示任何一类定时提示，F6/F7 按钮都保留并可点击 | Automated | 待建 | NOT TESTED |
| P17-N17 | 两次 `Failed`（如建目录失败后发布失败，文字与 generation 不同），无论在展示期间还是展示到期后发生，都不重新开始展示；标记一直在 | Automated | 待建 | NOT TESTED |
| P17-N18 | 重要提醒显示中，复制失败：提醒压到栈顶立即显示；到期后若重要提醒仍在期限内，重新露出 | Automated | 待建 | NOT TESTED |
| P17-N19 | 故障展示期间手动保存成功：故障展示被移除、标记消失，随后显示“已保存”；自动保存成功同样移除故障展示，但不显示成功 | Automated | 待建 | NOT TESTED |
| P17-N20 | “保留本地”写入进行中，持久化工作线程停止：进行中被清除，派生层回到冲突待决定；再选需要持久化的选项，立即显示“持久化不可用”重要提醒 | Automated | 待建 | NOT TESTED |
| P17-N12 | 视觉：浮层位置与对比度；Light/Dark；40% 透明度；Source 的最后一行可被瞬时提示暂时盖住（USER 接受） | Manual | macOS 在 `mac-test` 虚拟机中，Windows 在宿主上 | NOT TESTED |

## M2 Preview 文末留白

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P17-E01 | 文档高 ≤ 视口高 − 提示条高：不滚动，滑块隐藏 | Automated | 待建 | NOT TESTED |
| P17-E02 | 否则可滚范围为 `文档高 + R − 视口高`，`R = 视口高 / 2`；绘制、滚轮、滑块比例与隐藏、视图切换使用同一末端 | Automated | 待建 | NOT TESTED |
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
| P17-C11 | 复制成功显示“已复制”3 s；正在显示提醒、重要提醒或故障展示时不显示；复制失败显示提醒；均不修改文档或选区 | Automated | 待建 | NOT TESTED |
| P17-C13 | 两端都在同一行内结构内：只取里面的文字（`**abcd**` 中选 `bc` 得 `bc`）；嵌套结构被切开时逐层外扩至没有被切开的结构 | Automated | 待建：精确字符串案例 | NOT TESTED |
| P17-C14 | 块边缘：`# abcd ###` 选 `abcd` 得原文；setext 标题选全部得含下划线的原文、选 `bc` 得 `bc`；`- abcd` 从项目符号选起得 `- abcd`、只选 `abcd` 得 `abcd`；懒续行引用 `> ab` 加 `cd` 选全部得原文，不补第二个 `>`；整张表得含分隔行的原文；引用式链接段落不补定义；嵌套强调 `x **ab *cd* ef** y` 选 `b c` 得 `b *cd*`、选 `x ab c` 得 `x **ab *cd* ef**` | Automated | 待建：精确字符串案例 | NOT TESTED |
| P17-C15 | 行内结构边缘端点算作在外：`x **abcd** y` 中选 `abcd` 得 `**abcd**`，选 `x abcd` 得 `x **abcd**`，选 `abcd y` 得 `**abcd** y`；行内代码同样处理 | Automated | 待建：精确字符串案例 | NOT TESTED |
| P17-C16 | 外扩校验：Comrak 0.59 对 URL 与 `)` 之间有换行的链接报出的终点不以 `)` 结束，此时端点吸附到所在块的整行范围，不复制未闭合的链接 | Automated | 待建：精确字符串案例 | NOT TESTED |
| P17-C17 | 规范化的行内代码（跨行、首尾空格被去掉）按不一一对应处理：内部端点取整到内容边缘，再按边缘规则带上反引号；未规范化的行内代码逐字节映射 | Automated | 待建：精确字符串案例 | NOT TESTED |
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
  - `crates/stickymd-render/src/preview/pipeline/horizontal_tests.rs:83`：单独选中的公式，复制结果不再带分隔符；
  - `crates/stickymd-render/src/preview/pipeline/horizontal_tests.rs:154`：全选单个块级公式，从 `$$x^2$$` 改为 `x^2`；
  - `crates/stickymd-desktop/src/shell/phase15_session_tests.rs:46`：失焦保存不再显示“正在保存”（对应 Phase 15 的失焦保存验收行）；
  - `crates/stickymd-desktop/src/shell/scroll_application/tests.rs:163` 及同文件断言旧末端（`document_height - frame.height`）的用例；
  - `crates/stickymd-desktop/src/scroll/scrollbar.rs:471-484`、`:527-531`：Preview 末端与滑块比例改用新的末端函数；文档高等于视口高时可以滚动；
  - 断言单一诊断槽位内容的测试（`shell/phase15_*_tests.rs` 等），改为断言提示状态；其中关于数据安全的断言（保存失败可见、内存文本保留）保持不变；
  - Preview 复制与剪贴板反馈的现有测试（`phase-03`、`phase-05`、`phase-10` P10-A10、`phase-14` P14-A28 所引）逐项复核。
