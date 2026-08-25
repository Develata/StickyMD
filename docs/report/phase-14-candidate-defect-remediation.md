# Phase 14 Candidate Defect Remediation

## Status

Implementation corrected; fresh exact-candidate qualification and USER manual confirmation required.

## Observed Defects

1. 同一逻辑行仅选择数个字符时，selection 背景会覆盖其它逻辑行。
2. `\[...\]` 转换为 `$$...$$` 后 canonical text 与磁盘已改变，但 Source 画面要切换视图后才刷新。
3. 已完成的 Split Preview 切换为 Preview-only 后，viewport 改变而 clean generation 未 relayout，
   画面可能长期停留在 skeleton。
4. 真实鼠标将窗口拖到 Left/Top/Right 后，窗口看似贴边但未提交 Dock 状态，因此失焦后不会自动
   收起；旧 runtime smoke 却报告通过。
5. 已从 Left 停靠展开的窗口直接拖到 Top/Right 时，第一次拖动只会解除旧 Left Dock；必须再轻拖
   一次才会提交新边并自动收起。
6. 内容缩放可以改变并重置，但非 100% 缩放下顶部工具栏图标的绘制位置与实际点击位置分离。
7. Preview 中只选择代码块或 raw HTML 的一个逻辑行时，selection 背景会同时覆盖同一多行
   text span 的其它逻辑行。

这些事实均来自旧 exact candidate 的真实人工操作；该候选已经失效，不能继续作为 release evidence。

## Root Causes and Corrections

| Defect | Root cause | Correction | Complexity |
| --- | --- | --- | --- |
| Selection overpaint | `LayoutRun::highlight` 被用于 selection 范围外的 logical runs | 只向 selection 起止 logical line 范围内的 runs 请求 highlight | O(visible runs)，减少无效计算 |
| Source stale after conversion | 直接 splice 的新 `BufferLine` 保持 empty shaping/layout cache；buffer dirty state 不会发现它 | 只对 replacement logical-line range 建立 layout；full resync 使用 cosmic-text 公共 rich-text setter | ordinary line edit O(affected lines)，不退化为全文 rebuild |
| Preview skeleton after mode switch | same clean generation 被误认为无需工作，忽略 Split/Preview viewport width 变化 | visible mode change 对 clean generation 发出 typed `Relayout` job | 复用 semantic tree，仅重新 layout/paint |
| Real drag never commits Dock | winit 的真实 move loop 在自身 WndProc 内处理 `WM_ENTERSIZEMOVE/WM_EXITSIZEMOVE`；旧 `with_msg_hook` 只观察 event-loop queue，因而只对 smoke 人工 `PostMessageW` 的消息生效 | drag/resize 成功后立即建立 guard；真实 winit Left release 统一调用 `complete_window_drag`，移除无效的 move-size hook authority | 每次拖动结束 O(1)，无轮询、无额外线程或 runtime dependency |
| Direct Dock-to-Dock needs two drags | edge resolution 在检查新 snap candidate 前，先以旧边的 16-DIP detach 判定直接返回 `None` | 不同的新 snap edge 在同一次 `DragEnded` 中优先；只有未命中新边时才执行旧边 detach | 固定三个候选边，时间/空间均 O(1) |
| Zoom moves toolbar paint only | toolbar 绘制错误复用了 `DPI × content zoom` 的 document scale，而命中测试按正确的 window DPI 布局 | 显式分离 document/shell scale；Shell 只用 DPI，document projection 才叠加 content zoom | 每帧 O(1)，无新增分配或缓存 |
| Preview multiline selection overpaint | `cosmic-text` glyph range 相对各自 `BufferLine`，Preview 却把它直接写成整段 clipboard projection 的全局 byte range | layout 时单向累加 logical-line byte base，再投影 glyph range；wrapped run 复用同一 base | O(lines + glyphs)，额外空间 O(1) |

没有新增 runtime dependency，没有引入第二份 document authority，也没有让 smoke/test channel 进入产品
runtime。

工具栏回归同时由两层自动化持有：像素级 renderer test 验证 50/100/300% 下 active control 的绘制矩形
与 DPI hit rectangle 相同；copied-Release runtime smoke 在三个缩放值下实际点击 Source/Split/Preview 并
观察 durable view-mode acknowledgement。真实字体/图标观感仍由 G1 人工矩阵判定，不由截图或模拟点击
冒充人工 PASS。

## USER Rendering Stress Qualification Hardening

USER 提供的压力文档已固化为
`crates/stickymd-render/tests/fixtures/rendering-stress.md`。源附件 SHA-256 为
`C27DD8ABAA714FDA3AD925831C79D58204A2733158E9ABAA55BC7F4581469FC6`；本地
`E:/obsidian/test测试.md` 的 SHA-256 为
`F59540160B94B201ED42CCBB1B1F1E1D3C242604DD9228D1A646CFAFA4DBBBF2`。CI 不依赖该机器绝对路径。

适配只限于明确边界：Obsidian callout、WikiLink 与 Mermaid 保持 literal/CommonMark 行为；
`\require`、自定义 `\def`、CD/extarrows 等宏包依赖改写为当前 RaTeX 支持的标准
array、arrow、bra/ket、derivative 与 norm 等价形式，不降低到“出错也算 PASS”。独立的
恶意错误公式仍由原有 Phase 6 failure-isolation fixture 持有。

新回归验证 32 个 math node / 27 个唯一 `(literal, display)` cache key 全部经过
RaTeX parse/layout/native raster；同时覆盖 320/900 px、50/100/300% 内容缩放、Light/Dark、
`f32::MAX` 滚动 clamp、顶部/底部本地图片懒加载、远程图片不进入 image source，
多行代码 span 的逻辑行选择隔离，以及 copied-Release Preview/Split 字节级 source safety。像素非纯色、几何有界与进程存活是
自动化事实；数学美学、文字可读性和人眼视觉质量仍保持 `NOT TESTED`。

该 copied-Release 检查同时接入独立 `zoom` resource module；它不再依赖三边 Dock 物理拖动，桌面鼠标
被 USER 或其它程序移动时不会遮蔽本项结果。完整候选仍会运行组合矩阵，日常缺陷回归可只运行 Zoom。

## Native Drag Interop Hardening

将 copied-Release smoke 从伪造窗口消息改为真实物理输入后，锁定的 winit 0.30.13 Windows 实现还
暴露出一个上游参数错误：`handle_os_dragging` 将栈上 `POINTS` 地址作为
`WM_NCLBUTTONDOWN.lParam` post 到 event queue。Win32 合同要求 `lParam` 直接包含 signed screen
`x/y` 的 low/high words。该错误会使 move/resize anchor 取决于无关栈地址，无法作为 exact-candidate
窗口资格化基础。

修正保持在现有 `platform/windows/native_message.rs` 薄适配器中：

- winit message hook 只消费 `WM_NCLBUTTONDOWN` 这一种 malformed queued message；
- 使用 queued `MSG.pt` 中由 Windows 记录的 message-time screen point，不再进行第二次 desktop query；
- 正确打包 signed coordinate 后，以同 HWND/message/wParam 同步 `SendMessageW`；
- `handled=true` 阻止原 malformed message 再被 winit dispatch；
- nested dispatch 仍进入 winit/Win32 原生 move-size 生命周期，`WM_EXITSIZEMOVE` 与产品
  `complete_window_drag` authority 不变。

没有 fork winit、没有引入 beta dependency，也没有在 StickyMD 实现第二套 drag state machine。
新增 unsafe 仅限 Windows adapter，每个 block 均记录 pointer、HWND 与同步调用不变量；对应单元测试
固定验证负坐标的 signed packing。smoke 的绝对鼠标坐标按 virtual desktop 映射，覆盖负坐标环境；
真实 GUI 路径仍要求独占输入桌面，USER 同时移动鼠标会使它明确失败而不是扩大容差通过。

## Dock Coverage Correction

旧 copied-Release shell smoke 通过 `PostMessageW(WM_ENTERSIZEMOVE/WM_EXITSIZEMOVE)` 与
`SetWindowPos` 模拟拖动。这条路径恰好让 event-loop message hook 收到消息，遮蔽了真实 winit move
loop 无法提交 `DragEnded` 的产品缺陷；它属于 harness false positive，不能证明真实拖动与自动收起。

修正后的 copied-Release smoke 驱动：

- 用真实桌面指针完成 Left、Top、Right 拖动与 snap；
- 连续执行 Left -> Top -> Left -> Right，不插入 Floating 中间态，证明一次拖动即可直接换边；
- 将 foreground 真实交给 Windows shell，分别等待 700 ms auto-collapse 与 140 ms animation，再以
  3-DIP sensor reveal；
- top-left 与 top-right 两个物理角落均选择 Top；
- reducer 在 Left、Top、Right 上分别验证失焦后的精确 700 ms 收起边界与 focus/IME guard；
- deterministic geometry regression 明确验证 `Top > Left > Right`，包括 Left/Right exact tie。
- Left/Top 在 Pin OFF 下走完整路径，Right 在 Pin ON 下走同一路径；回到 Floating 后确认配置置顶
  仍存在，再关闭 Pin 并确认 HWND 不再 topmost。

Docked 窗口获得键盘焦点时按产品契约保持展开；失焦后才进入 700 ms deadline。因此“贴边后仍在
当前窗口内操作”不会立即收起。copied-Release smoke 现在覆盖真实 foreground loss，但不把一次机器
上的调度时刻冒充跨机器的肉眼精确计时；精确 699/700 ms 边界由虚拟时钟 reducer tests 持有，真实
动画观感和跨 DPI 行为仍由 G2 人工矩阵验收。

自动化不冒充真实 100/125/150/200% DPI、肉眼动画或真实失焦计时；这些 G2 项在新候选人工运行前
继续是 `NOT TESTED`。

## Semantic Conversion Linearization

静态 CRT candidate 的首次 Phase 14 headless run 稳定暴露了既有算法缺陷：1 MiB / 1000 math
node 的 delimiter conversion 对每个 replacement 逆序调用一次 `String::replace_range`，每次都搬移
后续大段字节，复杂度接近 `O(document bytes * formula count)`。连续两次测得 p95 分别为
`303.7952 ms` 与 `278.6936 ms`，超过原有 `50 ms` hard gate。

修正改为按已排序 source range 单向构建最终字符串：原文未修改区间与 replacement 各复制一次，
复杂度降为 `O(document bytes + formula count)`；不引入 unsafe、缓存或 dependency。重叠 range 与
非法 UTF-8 range 在构建任何结果前返回 typed error。相同 Release fixture 修正后为：

```text
median = 3.0808 ms
p95    = 4.0727 ms
max    = 5.3591 ms
```

该变化只优化现有 semantic conversion execution，不改变 Comrak authority、delimiter 语义、
DocumentState mutation gateway 或一次 Undo contract。

## Required Requalification

### Qualification input activation hardening

首次 exact-candidate campaign 的 Runtime 通道暴露了两处 harness-only 缺陷，产品 runtime 与
artifact 本身未改变：

- Split preview 的 posted client click 能路由应用内命中测试，却不能取得 Windows foreground / active /
  focused 状态；后续真实 `Ctrl+A/C` projection probe 因此按 fail-closed 规则拒绝注入。
- zoom smoke 通过 posted `WM_KEYDOWN/WM_KEYUP` 模拟 `Ctrl+加减号`，但 winit 快捷键判断依赖真实
  modifier state。posted message 不可靠地更新该状态，导致运行结果取决于启动焦点或外部键盘状态。

修正后的 smoke 在进入 preview probe 或 zoom shortcut 前先以物理点击建立并验证 foreground、active、
focused、uncaptured 状态；zoom 使用成对的物理 Control + key down/up，且每次注入前再次 fail closed
检查目标 HWND。Phase 10 targeted copied-Release Runtime 与 Phase 14 串联 Runtime 随后均通过。
这不是对产品输入 contract 的放宽，也没有增加 product dependency；它消除了资格化对桌面偶然状态的
依赖。

任何修复后的 tracked SHA 都是新 candidate。必须重新执行 Phase 14 Release/package、headless CI、
Runtime、Performance、Resources，并由 USER 重新完成受影响的 G1/G2 人工验收。旧 EXE/ZIP hash、
旧 dynamic receipt 与旧手工观察不得迁移为新候选 PASS。

后续 USER 批准的 portable runtime 资格化加固同样改变 Release artifact identity；静态 MSVC CRT
与 PE import gate 见
[`phase-14-portable-runtime-hardening.md`](phase-14-portable-runtime-hardening.md)。
