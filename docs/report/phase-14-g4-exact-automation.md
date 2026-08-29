# Phase 14 G4 Exact-Candidate Automation

## Background

USER 批准把 13 个原人工事实收敛成五个 G4 exact-candidate 自动组；后续另加入 G4-06 真实输入法功能组，
当前完整 G4 共六组。它们的主判定对象是可读取的
Win32 窗口/托盘/文件状态、精确 reducer 时间边界、物理快捷键结果、源码投影和 canonical path
identity，不依赖视觉审美或真实输入法候选框，因此适合形成可重复回归。

## Approved Groups

| Group | Bound cases | Exact assertions |
| --- | --- | --- |
| G4-01 | P12-M06..M09 | tray 菜单恰为显示/隐藏、置顶、退出；close 隐藏且保留 HWND/文本；show 恢复同 HWND；dirty quit 保存并退出 |
| G4-02 | P12-M10,M13..M17 | Left/Top/Right、无 Bottom、24/25 DIP、nearest/tie `Top > Left > Right`、3 DIP、700/100/500 ms、focus/IME/Pin guard |
| G4-03 | P12-M27 | Ctrl+Insert、Shift+Insert、Shift+Delete、Undo/Redo、Preview 只读、DIB/file-drop 图片路径 |
| G4-04 | P12-M31 | 真实 `$` toolbar conversion、源码即时投影、inline-code/literal safety、单次 Undo |
| G4-05 | P12-M44 | 真实 junction canonical identity、第二实例唤醒同一 HWND、note/config bytes 与 mtime 不变 |
| G4-06 | P14-A30 | Microsoft Pinyin / WeType 真实 composition、commit/cancel、selection replace、Search 与一次 Undo；结束恢复输入 profile/mode |

`P12-M11/M12` 包含 mixed-DPI 真实显示器感应条事实，不由主屏自动化冒充，继续留在 G2 人工组。

## Architecture Boundary

G4 复用 G3 的 std-only Rust exact lifecycle：candidate ZIP/EXE identity、每组独立复制目录、独占
StickyMD 进程、bounded wait 与 JSON receipt。Rust 持有全部 PASS/FAIL 规则；PowerShell UIA 只打开、
枚举和点击原生 tray menu。G4-02 的 deterministic reducer 单测和 copied-Release 物理窗口路径同时
执行，避免只验证 reducer 或只依赖不稳定桌面观测。

## Evidence Contract

`g4-exact-qualification.json` 必须绑定 source commit、harness commit、clean worktree、version、Windows
build、EXE SHA-256、ZIP SHA-256 与六个有序 `PASSED` 结果。单组诊断、dirty worktree、旧 candidate、
缺项、重复/乱序或任意失败均不能参与 readiness。G3/G4 必须串行，不能并发争抢 clipboard、tray、
窗口焦点或鼠标。

## CI Boundary

GitHub-hosted CI 只运行 CLI parser、receipt validator、matrix/governance 和纯状态单测；真正 G4 lane
只在解锁、独占的可信 Windows qualification host 显式运行：

```powershell
.\tools\smoke\phase-14.ps1 -G4
```

## Development Evidence

2026-08-28 在旧 exact candidate `eba0a18e3b34300be99c17c6faea76d9f69696b0` 上完成 G4-01..05
开发态端到端运行，五组均为 `PASSED`。首次 G4-01 真实运行发现 UIA 会把每个 native top-level
window 的 synthetic `SystemMenuBar/System` 项混入 process-scoped MenuItem；证据显示产品的三项 tray
menu 已正确出现。Helper 随后只排除 parent automation id 为 `SystemMenuBar` 的该非产品节点，重跑
G4-01 与完整 G4 均通过。完整 G3 随后也再次五项通过，证明共享 tray helper 未回归资产/恢复路径。
这些开发态收据记录 dirty tree，只用于 harness 调试，不参与 readiness；新 source freeze 后仍须生成
新 candidate 并依次收集 clean G3/G4 receipts。

2026-08-29 的 G4-01 诊断排除了产品 tray lifecycle 缺陷：clean exact targeted G4-01 通过；使用真实
paper HWND 后，隐藏态 native menu 在约 1 秒内暴露“显示 / 置顶 / 退出”；同一隔离候选连续十次
close -> tray Show 均通过。旧失败只有 `Timed out waiting for StickyMD Show tray item`，没有记录右键后
menu 是否真正打开。现有 helper 在 `SetCursorPos` 后直接发送物理右键，再只等待目标文本，因此桌面输入
被抢或 Explorer 未路由该次 click 时会把“菜单未打开”误报成“显示项缺失”。修复只增强 UIA adapter：
回读 cursor、以 process-scoped product menu 出现确认打开、失败时至多重试一次、Escape 后确认 popup
关闭，并输出 menu/geometry 诊断；产品 tray、window flow 与依赖图不变。

修复后的开发态定向回归使用同一 `4b76f89` 产品候选运行完整 G4-01，结果 `PASSED`，证据写入
`dist/evidence/g4-01-tray-uia-helper-dev.json`。该 receipt 明确记录 `worktree_dirty=true`，只证明 helper
修复能够完成真实菜单检查、Close/Show/Exit 生命周期与 durable-save 断言，不参与 readiness。新的 clean
source/harness freeze 后仍须重新生成 exact candidate 并收集 clean targeted G4-01 receipt。

## Architecture Drift

None. 产品 runtime、Document authority、Windows shell authority 与依赖图均未改变；本次只提升验证
覆盖和 release evidence 的可重复性。
