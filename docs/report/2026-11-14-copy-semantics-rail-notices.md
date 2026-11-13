# 提议：复制语义统一、块复制侧条、提示分级与末尾留白（2026-11-14）

## 结论

USER 在 2026-11-14 提出“给代码块与公式加复制键”，讨论后收敛为四项提议。四项可以分别批准，实施上有依赖：

| 编号 | 提议 | 状态 |
| --- | --- | --- |
| P1 | 复制语义统一：按“起止点所见即所得”复制；Preview 改为源码切片（方案 A） | 方向已获 USER 同意；落地前须先定 R3 与陈旧帧规则 |
| P2 | 块复制侧条：代码块与块级公式右侧外接等高细条，点击复制块内容 | 外形已获 USER 同意；窄窗与高缩放下的降级规则、宽度待定 |
| P3 | 提示分级：提示带类别，按类别自动消失（最长 30 s）；三种视图共用一条窗口底部提示条 | 方向已获 USER 同意；本报告给出状态转换草案，待批准 |
| P4 | 末尾留白：让最后一行能离开提示条 | Preview 超滚可行，短文规则待定；Source 两条路线都有代价，建议先做原型测量 |

四项都要修订 `docs/plan` 的现行合同，属于骨架级。按 `AGENTS.md`，须经 USER 批准后先改 `docs/plan`，
再改 features、acceptance 与代码。本报告不是契约。

- **实施顺序建议：** P3 → P4 → P1 → P2。P2 依赖 P1 的对象内容规则和 P3 的“已复制”提示。
- **实施基线：** `feat/macos-host`。命令 `git rev-list --count main..HEAD` 得 112；
  `git diff --shortstat main...HEAD -- crates/stickymd-desktop crates/stickymd-render` 得 50 个文件，+3054 / −296。

### USER 原话（2026-11-14，本会话）

- 侧条：“左边是块，右边是复制的键，两个在位置上连接，但是完全没有遮挡关系”。
- 复制：“复制用A：源码切片……按你的建议做就可以”，以及“可以都按你的建议来”。
- 提示：“根据重要程度最多维持 30s，绝大多数或许 3-5s 就够”；待决定提示被覆盖“这个可以接受”。
- 留白：“允许滚动条多滚动大约 10 个回车（或者半个屏幕）的高度……待议”。

## 现状（代码事实，`feat/macos-host` @ `f6bb473`）

### 复制

- **Source 复制。** 直接复制选中的 canonical 原文字节，并要求版本号（generation）一致
  （`crates/stickymd-app/src/flow/editor.rs:376-389`、`:449-460`）。
- **Preview 复制。** 复制显示投影（projection）里选区对应的字节（`crates/stickymd-render/src/preview/selection.rs:218-221`），
  交给 `AppIntent::WriteClipboard`（`crates/stickymd-desktop/src/shell/preview_input.rs:50-58`）。
  - 代码块复制 `code.literal`，不带围栏（`crates/stickymd-render/src/preview/render_tree.rs:211-217`）。
  - 公式复制 `source_literal`，带分隔符（块级 `render_tree.rs:263-266`，行内 `:417-421`）；拿不到源码范围时退回
    `math.literal`（`crates/stickymd-render/src/preview/parser.rs:390-394`）。
  - 图片复制 alt 文本。图片的选区范围由 alt 的长度决定（`crates/stickymd-render/src/preview/image_layout.rs:43-45`），
    所以 alt 为空的图片范围长度为 0，与“没有选区”（`selection.rs:60-62`）无法区分。
  - 两条现行行为有测试锁定：全选单个块级公式得 `$$x^2$$`（`crates/stickymd-render/src/preview/pipeline/horizontal_tests.rs:154`），
    全选单张图片得 `ALT`（`crates/stickymd-desktop/src/testing/contracts.rs:92`）。
- **Preview 全选。** `Ctrl+A` 只设一个普通选区 `[0, 全文长度]`，没有全选标记（`preview_input.rs:44-46`、`selection.rs:211-216`）。
- **写剪贴板。** `write_text` 返回 `Result`，成功即 `ClipboardWritten`，空文本走 `NoOp`（`editor.rs:255-262`）。
  判断成功不需要回读剪贴板。
- **源码位置。**
  - 每个 span 带有可空的 `source_range`（`parser.rs:139-145`）。
  - 粗体、链接等行内结构在生成 span 时被展开，没有保留成结构（`render_tree.rs:331` 起）。
  - 列表序号 span 没有源码范围（`crates/stickymd-render/src/preview/render_tree/list.rs:37-42`）。
- **Comrak 0.59 段首引用定义的位置上移。** 已有检测器 `inlines_shifted`（`crates/stickymd-render/src/preview/source_map.rs:18-31`），
  结果 `shifted_blocks` 只存在于 `OwnedDocumentTree`（`crates/stickymd-render/src/preview/model.rs:42`），
  只有分隔符转换在用（`crates/stickymd-render/src/preview/semantic_conversion.rs:116`）。
  渲染树与显示投影都不携带它。详见 [RISK 报告](RISK-comrak-054-inline-positions.md)。
- **Preview 陈旧帧。** 预览的 debounce 随每次编辑重置（`crates/stickymd-app/src/flow/preview.rs:83-85`），
  所以连续输入期间旧帧可以一直保留，之后还要等后台构建。
  旧帧上可以复制已有选区，但改动选区后的重绘会被拒绝（`crates/stickymd-desktop/src/shell/preview_runtime.rs:100-102`）。

### Preview 几何与绘制

- 下列尺寸都以“内容 DIP”计，乘以内容缩放（DPI × Content Zoom）（`crates/stickymd-render/src/preview/layout.rs:139-140`、
  `crates/stickymd-desktop/src/shell/render.rs:23`）：
  - 页边距 24（`layout.rs:23`）；
  - 代码块与块级公式的块都占满内容宽度（`layout.rs:300-301`）；
  - 背景比文字区左右各外扩 7，代码块上下各外扩 4，公式上下各外扩 3（`layout.rs:371-390`）。
- 所以只有在 100% 缩放、内容宽度未被夹到下限时，背景右缘才在窗格右缘往里 17 处。
  - **反例：** 220 DIP 宽的 Split、300% 缩放时，页边距为 72 px，而 Preview 窗格只有约 90 px；
    内容宽度被夹为 1，背景右缘约 94 px，已经超出窗格。
- 横向滚动条轨道与文字区同宽，从文字底边往下 10（`crates/stickymd-render/src/preview/horizontal.rs:119`、`:164-170`）。
  它比背景窄，并伸出背景下沿：代码块 6，公式 7。
- 纵向滚动条在窗格右侧 20 DIP 宽的窄槽内（`crates/stickymd-desktop/src/geometry.rs:13`），不随内容缩放。
  轨道从窗格内容右缘往外 2 DIP 处开始，宽 10 DIP，离窄槽外缘 8 DIP（`crates/stickymd-desktop/src/scroll/scrollbar.rs:60-65`）。
- 每帧整帧绘制。可见块的范围包含仍有部分在视口内的长块（`crates/stickymd-render/src/preview/paint.rs:162-178`）。
  每帧产出超宽块的横向命中目标，可作为新命中目标的先例（`paint.rs:185-199`、`horizontal.rs:111`）。
- 普通悬停不会触发 Preview 重绘（`crates/stickymd-desktop/src/shell/pointer.rs:130-165`）；拖选会（`preview_input.rs:131`）。
- macOS 现在通过 IOSurface 呈现帧（`dbdd820`、`apps/stickymd-mac/src/host.rs:259-262`）。
  `10` 第 89 行的 softbuffer 基线（`present` p50 2.4 ms）是历史数据，不能代表现在的成本。

### 诊断提示

- **单一槽位。** 诊断提示是一个文字槽位，新消息通常直接覆盖旧消息（`crates/stickymd-desktop/src/shell/shell_effects.rs:75`）。
  启动降级消息是例外，会拼接在一起（`crates/stickymd-desktop/src/shell/startup.rs:36-44`）。
  产品代码中没有找到清除它的路径。
- **只在 Source 窗格可见。** 文字只画在 Source 窗格底部，高 34 内容 DIP（`crates/stickymd-render/src/source/rendering.rs:468-480`），
  并且是 Source 重绘判断键的一部分（`render.rs:40`、`:96`）。
  工具栏橙色标记只看 `diagnostic.is_some()`（`render.rs:130`、`crates/stickymd-desktop/src/controls/toolbar_paint.rs:73-80`），
  没有独立的故障状态。
- **决定按钮靠字符串判断。** 外部修改冲突与未完成保存的恢复，都是带 `[F6 …] [F7 …]` 的槽位文字
  （`crates/stickymd-app/src/flow/note_io/reconcile.rs:101`、`crates/stickymd-desktop/src/shell/mod.rs:250`）。
  按钮的绘制与命中靠查找 `[F7`（`rendering.rs:481`、`crates/stickymd-render/src/source/projection.rs:493-503`）。
  F6/F7 按键直接派发持久化决定，不看提示文字，但仍受 IME 等前置过滤（`crates/stickymd-desktop/src/shell/input.rs:144-153`）。
- **已有的提示：**
  - 保存：“正在保存 generation N…”与“已保存”（`crates/stickymd-app/src/flow/note_io/save.rs:70`、`:106`）；
  - 保存失败的说明（`crates/stickymd-app/src/flow/note_io/receipts.rs:75` 起）；
  - 导出：“导出完成”（`crates/stickymd-app/src/session/io.rs:82-97`）。
- **没有的提示：** 复制成功、“已替换 N 处”与“已转换 N 处”。前一项已确认；后两项只做了检索，没有找到。

### 滚动末端

- **Preview。** 末端由 StickyMD 自己计算，但分散在多处：
  - 绘制，`paint.rs:169`；
  - 视图切换与同步，`crates/stickymd-desktop/src/shell/scroll_application.rs:81`；
  - 滑块比例到偏移，`crates/stickymd-desktop/src/scroll/scrollbar.rs:248-249`；
  - 滚轮，同文件 `:259`；
  - 滑块比例与隐藏，同文件 `:281` 起。
- **Source 滑块。** 滑块范围由 StickyMD 计算，依赖 `height_px` 与 `content_height()`
  （`crates/stickymd-render/src/source/scrollbar.rs:51-85`、`crates/stickymd-render/src/source/geometry.rs:256-258`）。
- **Source 滚轮与光标跟随。** 两者的末端都在 cosmic-text 0.19 内部，StickyMD 没有另外钳制
  （`crates/stickymd-render/src/source/geometry.rs:26-35`）：
  - `shape_until_scroll` 只在剩余内容不足一屏、且 `scroll.line > 0` 时往回拉（cosmic-text 0.19 `src/buffer.rs:644-646`）。
    所以只有一个很长折行逻辑行的文档（`scroll.line = 0`）不会被拉回。
  - `shape_until_cursor` 有往回和最小往前两条路径（同文件 `:493` 起）。
  - buffer 高度一变就标记 `RELAYOUT`（同文件 `:826-828`），下次排版时清掉所有行的布局（`:444-450`）。

## P1：复制语义统一（方案 A：源码切片）

### 规则

- **R1 对象内容。** 规范化后的选区如果完全落在一个对象的内容范围内（不含列表序号与容器前缀），只复制内容：
  - **公式**（块级与行内；公式是原子对象，即“恰好这个公式”）：不带分隔符的 TeX，即 `math.literal`。
  - **代码块与 Raw HTML 原文块：** 恰好是被选中的那部分代码文本，不带围栏，也不带引用或列表前缀。
  - **图片：** 保持现行的 alt 文本，`contracts.rs:92` 不变。要改成复制源码，需要能表达零长度原子对象的选区，不在本提议范围内。
- **R2 源码切片。** 其他情况，把两个端点映射为该 Preview 帧所属 generation 的源码字节位置，复制这段源码。
  `**`、`#`、`[文字](链接)`、`- `、`> `、`![alt](路径)` 都随之带上。
- **R3 被切开的结构：** 见待定 1。
- **R4 读帧自己的快照。** Preview 复制读取该帧 generation 的源码快照（`Arc<str>`），不读当前文档。
  陈旧帧上复制的也就是屏幕上看到的内容。陈旧帧上可以做哪些操作，见待定 2。
- **R5 全选无特例。** `Ctrl+A` 仍是普通选区，不跨层传递全选标记。
  - 只有一个公式的便签，全选按 R1 得 `x^2`；Source 全选得 `$$x^2$$`。`horizontal_tests.rs:154` 随之改变。
  - 只有一个列表项包着代码块的便签，全选包含了列表序号，所以按 R2 处理。
- **R6 位置不可信时保守处理：**
  - 端点在 `shifted_blocks` 段落内：吸附到该段的整行范围。
  - 端点在显示与源码不一一对应的 span 内（转义、实体、软换行显示为空格、硬换行）：按 span 边界向外取整。
  - 端点在没有源码范围的生成文字上（列表序号等）：吸附到所属列表项或块的源码起点。

### “统一”的边界

统一的是用户规则，取文本的路径仍然不同。

- Source 是原样切片：在引用里的代码块中从 `aa` 选到 `bb`，Source 复制得 `aa\n> bb`。
- Preview 按 R1 得 `aa\nbb`。

### 后果

- 只选中一个行内公式按 Ctrl+C，即得不带分隔符的 TeX，取代被放弃的“行内公式复制键”。
- 带图片的范围复制成 `![alt](assets/…)`。相对路径粘贴到别处会失效，与 Source 复制一致。

### 新工作（推断）

Render 需要为每个 generation 生成并保留一份“复制映射”，经 `PreviewFrame` 的语义方法使用；App 不读其内部。
依赖方向允许这样做，不需要反向依赖。映射内容：

- span 到源码的范围，以及该 span 是否一一对应；
- R3(b) 需要的结构开、闭定界符范围（粗体、链接等现在被展开了，需要另行保留）；
- 对象的内容范围（代码块、HTML 块、公式、图片）；
- 传递过来的 `shifted_blocks`。

`LaidOutBlock` 现在没有代码内容范围（`layout.rs:38-47`）；构建时可由 `selection_text` 的前后长度差得到，需要排除列表序号（`layout.rs:154-247`）。

### 成本（推断，未实测）

- 复制时：每个端点做一次二分查找再切片。
- 每次构建：多出 O(span + 结构) 的映射与内存，另外保留一份源码快照，生命周期与 Preview 帧相同。

### 不采用

方案 B“只补包装”：保留显示文本复制，只在选区盖住整个对象并超出时补分隔符或围栏。它仍是两套语义。

### 待定

1. **被切开的结构（R3）。** 例如选区从段落拖进代码块的第 3 行。
   - (a) **拖动时就吸附到整个结构**，并画出来。实现简单，但“段落加代码块的前 3 行”从此选不出来，长代码块会被整块带上。
   - (b) **复制时用结构自己的定界符包住选中部分。** 代码块取开围栏行、选中的行、闭围栏行；粗体取 `**`、选中文字、`**`。
     选区显示不变，复制的全是源码字节。
     - 适用范围：围栏代码块，以及强调、粗体、删除线、行内代码、行内链接与图片。
     - 得到的是保留定界符的源码片段，**不保证单独粘贴后仍能渲染**：引用式链接缺定义；表格数据行缺表头和分隔行；
       缩进代码没有围栏；未闭合的围栏没有闭围栏可取；嵌套容器还需要每行前缀。这些情况退回 (c)。
   - (c) **原样切片：** 可能产生只开不闭的围栏或 `**`。

   **建议 (b)，范围以外退回 (c)。** 讨论中最初提议的是 (a)，发现上述限制后改为 (b)，交 USER 确认。
2. **陈旧帧上的操作。** 建议在陈旧帧上允许复制已有选区和点击侧条，行为与 R4 一致；新的拖选沿用现行规则，即重绘被拒绝。
3. **表格单元格内的选区。** 建议按 R2 切片。现有修正 `table_cell_map` 只处理 `\|` 造成的列号偏移（`parser.rs:141-143`、
   `source_map.rs:111`），单元格内逐字节的映射仍属新工作。

## P2：块复制侧条

- **对象。** 代码块（含 Raw HTML 原文块）与块级公式。行内公式由 P1 的 R1 覆盖；表格、图片不加侧条。
- **几何**（以内容 DIP 计，100% 缩放时）：
  - 侧条外接于块背景右缘，与背景同高，不包含横向滚动条轨道。轨道比背景窄并伸出下沿 6 或 7；
    侧条如果拉到轨道底部，会和轨道之间留出约 7 的缺口。
  - 宽约 8，待视觉验收。放在背景右缘与窗格内容右缘之间约 17 的空白里，不改排版，不与正文、背景、滚动条重叠。
- **降级。** 只有侧条整根都落在 Preview 窗格内容区内时才画。高缩放、窄窗或深层缩进时不画，仍可用选区复制。
  这与上文的 300% 反例对应。
- **天然悬浮。** 侧条与块等高，块露出多少，侧条就露出多少；横向滚动时它贴着固定的背景边。
- **命中与交互。**
  - 可点区域就是看得见的那块矩形，先不向块内延伸。
  - 侧条的命中优先于开始选区。
  - 沿用 3 DIP 的点击/拖动阈值（`crates/stickymd-desktop/src/preview/input.rs:12`）：在侧条上按下后拖动，既不复制也不开始选区。
- **视觉。** 常显、低对比，效果是“框的右侧略粗一点”；悬停时变亮并切换光标；没有图标。
  40% 透明度下可能看不清，USER 接受。
- **复制与反馈。** 内容同 R1；结果用 P3 的“已复制”提示。
- **实现位置（推断）。** render 每帧产出可见侧条目标（照横向目标的写法）；desktop 处理悬停与点击；剪贴板走 `WriteClipboard`。
- **成本（推断，未实测）。** 每帧多画 O(可见块) 个小矩形；只在鼠标进出侧条时重绘一次。IOSurface 路径下的实际耗时还没测。

## P3：提示分级与统一提示条

### 类别与时长

| 类别 | 例子 | 时长 |
| --- | --- | --- |
| 成功 | 已复制、导出完成、已保存 | 3 s |
| 一次性失败 | 无法打开导出位置选择器、窗口拖动失败、导出失败、启动降级 | 5–10 s；重要的最多 30 s |
| 进行中 | 正在准备图片…、正在导出当前快照…、正在保存… | 由同一操作的结果结算 |
| 持续故障 | 保存失败（直到之后的保存成功）、持久化工作线程已停止 | 文字最多 30 s 后收起，标记保留到解除 |
| 待决定 | 外部修改冲突、未完成保存的恢复（F6/F7） | 不过期 |

### 状态转换草案

1. 每条提示带身份、类别与操作标识。到期的定时器只对自己那条提示生效；被替换的提示，其定时器回调直接忽略。
2. “进行中”只由同一操作的完成或失败结算，结算后换成对应的成功或失败提示。
3. 无关的成功提示遇到失败、进行中、持续故障或待决定提示时，不显示；不排队，也不保留历史。
   这把 07:41-42 对复制成功的规则推广到所有成功提示。
4. 失败提示总会显示。
5. **待决定来自持久化层的权威状态，不存在槽位里。**
   - 它可以被其他提示暂时覆盖，这是 USER 接受的。
   - 覆盖它的提示一消失，只要决定仍未做出，就重新显示它。
   - 否则，覆盖的提示自动消失后，屏幕上什么都不剩，F6/F7 却仍然有效。
6. **持续故障同样来自持久化层的权威状态**，不再用 `diagnostic.is_some()` 驱动标记。
   - 文字收起后，标记保留到故障解除；点击标记可重新显示文字，所以原因和重试方法始终能看到。
   - 被覆盖后同样恢复。
   - 这修订了 `09:138`“用户可见的错误仍是诊断文字”，并让“不静默吞掉保存错误”在自动消失后仍然成立。
7. **定时器：** 每条提示设一次定时唤醒，走现有调度器（`03:113` 的可控时钟）；没有动画。

### 呈现

- 把提示条从 Source 渲染器里移出，由 Shared Desktop 在窗口底部画一条浮层；搜索浮层已经是这么画的。
- 三种视图都可见。Split 下一条横跨两个窗格。
- 类别取代查找 `[F7` 来决定画不画按钮。
- 提示文字离开 Source 的重绘判断键。只要 P4 不改 Source 视口，提示出现或消失就不再迫使 Source 重绘；P4 的影响见下。

### 已有提示的迁移

- “正在保存 generation N…”按“进行中”处理，“已保存”按“成功 3 s”处理。
  自动保存频繁，是否干脆不显示“正在保存…”，待 USER 定。
- “导出完成”改为 3 s。

### 新增提示

- 已复制：所有复制路径。
- “已替换 N 处”与“已转换 N 处”：被改的地方可能不在屏幕上。
- **原则：** 只为结果不在屏幕上的成功操作给提示；失败一律提示。

## P4：末尾留白（提示条遮挡最后一行）

USER 提出：允许滚动到文末之后再多滚约半屏或约 10 行，单位取决于哪种更好实现，属于待议。

### 单位

用像素，按视口高度的比例计算，下限不小于提示条高度。Preview 的块高不固定，“行”在那里没有意义。

### Preview：可行

- 须把上面列出的 5 处末端计算合并成一个函数（绘制、视图切换与同步、滑块、滚轮、滑块比例与隐藏），只改其中两处不够。
- **短文规则待定。** `max_scroll = max(0, 文档高 + 留白 − 视口高)` 有反例：文档高 400、视口 600、留白 300 时，
  它允许再滚 100，而底部本来已经空着 200。
  - 候选规则：文档高 ≤ 视口高 − 提示条高时不滚；否则用上式。
  - 缺点：在这个阈值附近，可滚距离会突然跳变。

### Source：两条路线都有代价，建议先做原型测量

- **(i) 提示条显示期间缩小视口：**
  - 必须改 Source 投影自己的高度权威，而不只是 cosmic-text buffer 的高度；滑块范围依赖 `height_px` 与 `content_height()`。
  - 每次提示出现或消失都会让所有行重新布局（`RELAYOUT`），长文的代价未知。
  - 正文不只在文末移动。
  - 需要显式规则：缩小前光标可见、缩小后会被挡住时，揭示光标；否则保持顶部锚点。
    缩小本身不能保证光标留在提示条上方：只调整大小，不会调用光标跟随（`projection.rs:454-480`、`geometry.rs:127`）。
  - 重新布局后要重新定位 IME 候选框（`crates/stickymd-desktop/src/shell/presentation.rs:85`）；也不能触发 Split 同步。
- **(ii) 在 cosmic-text 之外记一段尾部偏移：**
  - 须在绘制、命中、光标、选区、IME 候选框等以 `padding()` 为原点的地方统一平移。
    `source/geometry.rs` 与 `source/rendering.rs` 中约 11 处（推断）。
  - 还要加“光标距底边至少一个提示条高度”的跟随规则。
  - 改动面大，并碰到 IME 定位，而 IME 正确性优先级最高。

**建议：** 先给 (i) 做原型，在 100 KiB、1 MiB 和单个超长折行逻辑行上测重新布局的耗时，并检查 IME 候选框位置，再决定选 (i)、(ii)，
或者接受短时提示暂时盖住最后一行。

## 设计哲学（提议写入 `02`，“‘极简’是什么意思”一节之后）

> ### 复制：起止点所见即所得
>
> 复制的范围由用户看到的起点与终点决定，Source、Preview 与复制键遵循同一条用户规则（取文本的路径各自实现）：
>
> - **起止点都落在同一个公式或代码块的内容里：只复制内容**，不带分隔符或围栏。要复制一个公式或一段代码，用户要的是里面的东西；
>   不带包装的内容也是各种粘贴目标（LaTeX 文档、公式编辑器、其他 Markdown 工具）都能接受的最小公分母，怎么包装由目标决定。
> - **范围超出单个对象：保留 Markdown 渲染符号。** 这些符号是通用的，粘贴到 Markdown 环境中多数仍能被渲染；
>   去掉它们，只会留下只能用肉眼读的文字。引用式链接、表格片段等不能单独成立的结构除外。
>
> 所见即所得指起止点，而不是 Preview 的像素。这顺应“复制一个东西就得到那个东西”的习惯，省去额外的心智负担；
> 一条规则覆盖两种视图与复制键，用户不必分别记忆。

## 外部惯例（一手来源，2026-11-14 核对）

- **MathJax：** 右键菜单 “Copy to Clipboard → Original Form” 复制 `mathItem.math.trim()`，即两个分隔符之间、去掉首尾空白的 TeX。
  依据：`ts/ui/menu/Menu.ts` 的 `copyOriginal()`；`ts/input/tex/FindTeX.ts` 的 `findEnd` 把开、闭分隔符单独存放。
- **KaTeX copy-tex：** 选区复制时默认给每个公式加上 `$…$` 或 `$$…$$`，**只选一个公式也会加**，并不判断是否混有文字。
  依据：`contrib/copy-tex/katex2tex.ts` 的 `defaultCopyDelimiters`。
- **结论：** 两者是不同交互的不同选择，不构成统一惯例。R1“单个对象不带分隔符”是 StickyMD 的产品选择，不是已核实的行业惯例。
- Typora、Obsidian、Notion 的做法未核实。

## 对规格的影响（批准后需修订）

- **`02`：** v1 Scope 新增一条；新增上面的设计哲学一节。
- **`06`：**
  - 复制合同 `:217`（选区字节映射到显示投影）、`:226`（复制只读显示投影）、`:228`（公式复制带分隔符）；
  - `PreviewFrame` 的语义方法 `:230`（新增侧条目标与对象内容复制）；
  - 横向滚动一节中“复制读取完整 projection”（`:366`）；
  - 陈旧帧上的复制与侧条操作。
- **`07`：** `:41-42`，复制成功不提示。
- **`09`：**
  - 垂直滚动条 `:57-65`（P4 的末端与短文规则）；
  - 提示条的呈现与类别（新增条目）；
  - `:138`“用户可见的错误仍是诊断文字”。
- **`10`：** 不新增预算。验收时确认 Preview 帧时间、Source 输入延迟与内存门不变。
- **投影文档：**
  - `docs/features/00_v1_product_behavior.md:28` 与 Preview、查找替换、自动保存各节；
  - 新增验收行；
  - `coverage-matrix`。

## 验证计划

- **P1 单元测试：**
  - R1–R6；
  - 零长度 alt 的图片；
  - 引用或列表中的代码块（与 Source 的差异）；
  - 列表序号端点；
  - 引用式链接、表格片段与未闭合围栏的 R3 退回；
  - 引用定义上移的段落；
  - 转义、实体与软、硬换行；
  - 陈旧帧复制。
- **P2：** 侧条命中、阈值与可见区间；高缩放与窄窗降级。
- **P3：** 定时器身份、转换表 1–7、待决定与持续故障的恢复。
- **P4：** Preview 5 处末端一致；短文规则；Source 原型在长文与单个长折行行上的重新布局耗时、光标揭示与 IME 候选框位置。
- **GUI：** `mac-test` VM 中做视觉验收；Windows 宿主另测。
- **资源：** 改动前后对比 Preview 帧时间、Source 输入延迟与内存基准。本报告的成本均为推断，没有实测。

## 审查记录

2026-11-14，codex-reviewer（GPT，high，只读）对初稿做了对抗审查，提出 3 个阻断项与 12 个应修项，均已核对并改入本稿：

- **阻断项：**
  - alt 为空的图片无法单独选中；
  - 进行中提示与成功提示的规则自相矛盾；
  - Source 缩小视口不能自动保证光标可见。
- **应修项：**
  - 重新布局的成本；
  - cosmic-text 往回拉的条件；
  - Preview 末端分散在 5 处；
  - 复制映射的数据链不完整；
  - 引用内代码在 Source 与 Preview 下的差异；
  - R3(b) 结果合法性的保证过强；
  - 标记不能证明保存错误未被吞掉；
  - 陈旧帧持续时间被低估；
  - 几何依赖缩放；
  - macOS 改用 IOSurface 后旧基线已过时；
  - 外部惯例的表述；
  - 若干绝对化描述。

本报告没有编译、运行测试或做 GUI 操作；所有反例都是源码推演。
