# Phase 14 Candidate Defect Remediation

## Status

Implementation corrected; fresh exact-candidate qualification and USER manual confirmation required.

## Observed Defects

1. 同一逻辑行仅选择数个字符时，selection 背景会覆盖其它逻辑行。
2. `\[...\]` 转换为 `$$...$$` 后 canonical text 与磁盘已改变，但 Source 画面要切换视图后才刷新。
3. 已完成的 Split Preview 切换为 Preview-only 后，viewport 改变而 clean generation 未 relayout，
   画面可能长期停留在 skeleton。

这些事实均来自旧 exact candidate 的真实人工操作；该候选已经失效，不能继续作为 release evidence。

## Root Causes and Corrections

| Defect | Root cause | Correction | Complexity |
| --- | --- | --- | --- |
| Selection overpaint | `LayoutRun::highlight` 被用于 selection 范围外的 logical runs | 只向 selection 起止 logical line 范围内的 runs 请求 highlight | O(visible runs)，减少无效计算 |
| Source stale after conversion | 直接 splice 的新 `BufferLine` 保持 empty shaping/layout cache；buffer dirty state 不会发现它 | 只对 replacement logical-line range 建立 layout；full resync 使用 cosmic-text 公共 rich-text setter | ordinary line edit O(affected lines)，不退化为全文 rebuild |
| Preview skeleton after mode switch | same clean generation 被误认为无需工作，忽略 Split/Preview viewport width 变化 | visible mode change 对 clean generation 发出 typed `Relayout` job | 复用 semantic tree，仅重新 layout/paint |

没有新增 runtime dependency，没有引入第二份 document authority，也没有让 smoke/test channel 进入产品
runtime。

## Dock Coverage Correction

旧 copied-Release shell smoke 的真实 move/collapse/reveal 路径只驱动 Left；它不能证明 Top/Right 的
真实 Win32/winit bridge。确定性 reducer/geometry tests 原本覆盖三边，但人工 G2 从未开始，故 Top、
Right、真实 corner/timing 一直应保持 `NOT TESTED`。

修正后的 copied-Release smoke 驱动：

- Left、Top、Right 的 snap -> manual collapse -> 3-DIP sensor reveal；
- top-left 与 top-right 两个物理角落均选择 Top；
- reducer 在 Left、Top、Right 上分别验证失焦后的精确 700 ms 收起边界与 focus/IME guard；
- deterministic geometry regression 明确验证 `Top > Left > Right`，包括 Left/Right exact tie。

Docked 窗口获得键盘焦点时按产品契约保持展开；失焦后才进入 700 ms deadline。因此“贴边后仍在
当前窗口内操作”不会立即收起。copied-Release smoke 不以合成桌面失焦冒充人工时序证据，真实焦点、
动画观感和跨 DPI 行为仍由 G2 人工矩阵验收。

自动化不冒充真实 100/125/150/200% DPI、肉眼动画或真实失焦计时；这些 G2 项在新候选人工运行前
继续是 `NOT TESTED`。

## Required Requalification

任何修复后的 tracked SHA 都是新 candidate。必须重新执行 Phase 14 Release/package、headless CI、
Runtime、Performance、Resources，并由 USER 重新完成受影响的 G1/G2 人工验收。旧 EXE/ZIP hash、
旧 dynamic receipt 与旧手工观察不得迁移为新候选 PASS。
