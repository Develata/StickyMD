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

这些事实均来自旧 exact candidate 的真实人工操作；该候选已经失效，不能继续作为 release evidence。

## Root Causes and Corrections

| Defect | Root cause | Correction | Complexity |
| --- | --- | --- | --- |
| Selection overpaint | `LayoutRun::highlight` 被用于 selection 范围外的 logical runs | 只向 selection 起止 logical line 范围内的 runs 请求 highlight | O(visible runs)，减少无效计算 |
| Source stale after conversion | 直接 splice 的新 `BufferLine` 保持 empty shaping/layout cache；buffer dirty state 不会发现它 | 只对 replacement logical-line range 建立 layout；full resync 使用 cosmic-text 公共 rich-text setter | ordinary line edit O(affected lines)，不退化为全文 rebuild |
| Preview skeleton after mode switch | same clean generation 被误认为无需工作，忽略 Split/Preview viewport width 变化 | visible mode change 对 clean generation 发出 typed `Relayout` job | 复用 semantic tree，仅重新 layout/paint |
| Real drag never commits Dock | winit 的真实 move loop 在自身 WndProc 内处理 `WM_ENTERSIZEMOVE/WM_EXITSIZEMOVE`；旧 `with_msg_hook` 只观察 event-loop queue，因而只对 smoke 人工 `PostMessageW` 的消息生效 | drag/resize 成功后立即建立 guard；真实 winit Left release 统一调用 `complete_window_drag`，移除无效的 move-size hook authority | 每次拖动结束 O(1)，无轮询、无额外线程或 runtime dependency |

没有新增 runtime dependency，没有引入第二份 document authority，也没有让 smoke/test channel 进入产品
runtime。

## Dock Coverage Correction

旧 copied-Release shell smoke 通过 `PostMessageW(WM_ENTERSIZEMOVE/WM_EXITSIZEMOVE)` 与
`SetWindowPos` 模拟拖动。这条路径恰好让 event-loop message hook 收到消息，遮蔽了真实 winit move
loop 无法提交 `DragEnded` 的产品缺陷；它属于 harness false positive，不能证明真实拖动与自动收起。

修正后的 copied-Release smoke 驱动：

- 用真实桌面指针完成 Left、Top、Right 拖动与 snap；
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

## Required Requalification

任何修复后的 tracked SHA 都是新 candidate。必须重新执行 Phase 14 Release/package、headless CI、
Runtime、Performance、Resources，并由 USER 重新完成受影响的 G1/G2 人工验收。旧 EXE/ZIP hash、
旧 dynamic receipt 与旧手工观察不得迁移为新候选 PASS。
