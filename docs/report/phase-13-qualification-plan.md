# Phase 13 Qualification Plan

## Decision

Phase 13 是 evidence campaign，不是 feature phase。产品 runtime、架构边界、依赖图和性能实现
全部冻结；唯一允许的 tracked change 是 candidate freeze 前的 qualification tooling 与当前状态文档。

## Qualification Environment Gate

std-only smoke adapter 从当前进程所属 Windows session 读取：

- WTS active/unlocked session facts；
- input desktop 是否可打开；
- 同 session `explorer.exe` 是否存在；
- foreground window 是否可用；
- display count 是否大于零。

只有所有必要事实同时成立才输出 `VALID`。锁屏、断开的 session、不可访问 input desktop、缺少
交互 shell/foreground/display 都输出 `ENVIRONMENT_BLOCKED`。非 Windows 输出 `UNSUPPORTED`；
平台查询失败输出 `ERROR`。blocked/unsupported 映射为 `NOT_TESTED` 和非零退出码，既不是产品
FAIL，也不是 PASS。典型检查只执行常量数量的系统查询，目标小于 1 秒。

机器可读证据只记录状态、布尔事实与显示器数量；不记录窗口标题、用户名、桌面名称或路径。
WTS level-1 session flags 是锁定事实的主要来源，残留 LockApp 进程不单独构成 locked 判定。

## Campaign Ordering

Rust `qualification local` 持有固定顺序：Environment → Release → candidate receipt → all-ci →
Runtime → Performance → Resources → Readiness。Runtime 失败/blocked 会阻止昂贵的后续阶段。
Performance、Runtime、Resources 自身也带 environment preflight，避免绕开 campaign 时产生无效收据。

## Resource Partial Evidence

Resources 每个主要场景前重新检查环境。每个已完成场景后覆盖写入同一个 receipt，并附加
`INCOMPLETE` campaign result；只有全部场景完成并由 runner 写入最终 `acceptance readiness`
PASS 时，收据中才不存在 `INCOMPLETE`。readiness 仍要求所有 result 都是 `PASSED`，因此 partial
receipt 无法冒充完整资源 PASS。中途锁屏时保留最后 partial evidence 并立即停止。

## Manual Sessions

M1..M5 只减少重复 setup，不合并 case authority。recorder 始终生成 P12-M01..P12-M44 全集，
每项只允许 `MANUAL_PASS`、`MANUAL_FAIL`、`NOT_TESTED`，并在每次观察后落盘以减少中断损失。
旧 receipt 只有 source/EXE/ZIP 与当前 candidate 完全一致时才能续录。waiver 仍只能由 USER 通过
具体 `WAIVER-P12-Mxx` decision 提供。

## Evidence Integrity

Readiness 保留 Release/package、headless CI、Runtime、Performance、Resources 五类 exact automated
receipt，加上 manual、remote、downloaded-artifact 与 USER decisions。所有 Phase 13 local receipt
绑定 source commit 和 EXE；package/manual 还绑定 ZIP。source freeze 后不提交动态证据。

## Windows API Boundary

环境 adapter 仅位于 `tools/stickymd-smoke`，使用 WTS session query、input desktop、Toolhelp
process enumeration、foreground/display snapshot。没有 API 或状态进入 `stickymd-core`、
`stickymd-render` 或产品 Windows shell。

微软定义 `WTSQuerySessionInformationW(WTSSessionInfoEx)` 返回的 level-1 `SessionFlags` 可区分
lock/unlock；返回内存由 `WTSFreeMemory` 释放。相关实现的每个 unsafe block 都带局部 ownership /
pointer SAFETY invariant。

## Remaining Dynamic Work

exact candidate freeze、五类自动收据、M1..M5 人工观察、remote workflow、downloaded artifact 与
最终 readiness 都是 freeze 后的动态工作；其结果不得回填本报告。
