# Phase 14 Resources Failure Triage

## Classification

`QUALIFICATION HARNESS DEFECT`

没有证据支持 product reducer 或 platform projection defect。产品 runtime 与 runtime dependency
均未修改。

## Original Failure

旧 candidate source：`1d533357ac072605b350b0523f2957597341bc62`。

Phase 8 hidden-window resource matrix 在 112 个已完成 resource measurements 后失败：外部写入
`note.md` 后，harness 固定等待 350 ms，随即注入 Enter；最终磁盘仍为 25 bytes 的外部内容，
没有达到“一次换行插入”的预期。旧错误不包含 cycle、expected/actual bytes、projection ready 或
foreground/focus/geometry 事实。

## Reproduction and Reduction

使用每次新 portable directory 与 copied Release EXE 的 Rust CLI reducer：

| Path | Independent runs | Stress | Result |
| --- | ---: | --- | --- |
| Original combined | 10 | collapse 1000；tray 100；controls 100；reload 100 | PASS 10/10 |
| Collapse/expand | 10 | collapse 1000 | PASS 10/10 |
| Tray hide/show | 10 | tray 100 | PASS 10/10 |
| Controls | 10 | topmost/theme/opacity 100 | PASS 10/10 |
| Collapse + tray | 10 | collapse 1000；tray 100 | PASS 10/10 |
| Combined reduction | per count | 500、100、50、20、10、5、1 | PASS |
| Projection-gated combined | 10 | collapse 1000；tray 100；controls 100；reload 100 | PASS 10/10 |

首次 5/1 control reduction 的失败来自诊断器错误地假定 opacity 最终恒为 100；按奇偶正确期望
修正后均 PASS，不是产品失败。

## Root Cause

旧 harness 把固定 350 ms 当作 external reconciliation 和 source projection 完成条件。watcher、
I/O、主协调线程和 editor projection 是异步链路，经过固定时间不构成状态证明。随后使用按键改变
canonical document，导致原始失败无法区分“产品没有 reload”“projection 尚未同步”“窗口没有
输入焦点”。

新 gate 先等待窗口 `visible + stable geometry + foreground + active + focused`，再用真实 Windows
键盘输入执行 Ctrl+A/C，从 clipboard 读取 source projection，并逐字节对比规范化后的预期 UTF-8
文本；只有匹配后才收拢 selection 并继续 mutation。等待有 10 秒上界，失败会记录 typed stage、
cycle、bytes 与完整 shell facts。没有使用无限 retry，也没有以延长 sleep 作为修复。

## Change Scope

- product runtime delta：0。
- runtime dependency delta：0。
- verification tooling：新增 typed `qualification window-stress`、shell observation、clipboard
  projection gate，并由 `tools/smoke/phase-14.ps1 -WindowStress` 提供稳定入口。
- manual Tier A/B/C：保持 `NOT TESTED`。

## Candidate Consequence

verification tooling 是 tracked source；因此旧 source/EXE/ZIP receipt 不再代表当前仓库。新 freeze
commit 必须重新执行 Release/package、headless CI、Runtime、Performance 和完整 Resources。
Resources PASS 前不得开始正式 manual receipt；PASS 后只进入 USER-guided G1/G2/G3，仍不授权
Phase 15、tag、publish 或 push。

## Exact-candidate Follow-up

source `560ef02b332b91756d51e108e048f3dc955ebdf6` 的完整 Resources 通道随后报告窗口对象从
`{handles: 415, gdi: 19, user: 26}` 增长到 `{handles: 432, gdi: 19, user: 29}`，句柄增量 17
刚好超过 hard limit 16。分场景 reducer 给出：tray 100 次为 `+2 handles / +4 USER`，controls
100 次为 `+2 / +0`，external reload 100 次为 `+6 / +1`，collapse 1000 次无增长；相同组合路径
从已激活全部子系统后的 baseline 到末尾为净 `-5 handles`。这否定了随循环次数线性增长的产品
泄漏假设。

根因是旧 measurement baseline 只包含 Source 初始状态，而末值已经首次初始化 tray wake、toolbar
controls、external conflict、Preview/image decode 等稳定子系统，比较集合不相等。修正只在 baseline
前对每条被测路径执行一次有界 warm-up，随后仍执行原 1000/100/100/100 压力循环，且不改变
`8 MiB / +16 handles / +8 GDI / +8 USER` 门槛。定向 copied-Release window resource 通道在相同
hard limits 下通过；product runtime 与 runtime dependency delta 仍为零。

同一轮 Runtime 的 220x120 durable resize 失败首先归因于 harness：旧 helper 直接
`SetWindowPos` 并伪造 `WM_ENTERSIZEMOVE/WM_EXITSIZEMOVE`，绕过了产品由真实 winit resize 与
左键释放提交 durable geometry 的合同。新 helper 改用 `SendInput` 驱动真实右下角 pointer resize，
并等待 native move-size；输入目标、move-size、最终 HWND 几何和 durable config 均分别验证，桌面
被 USER 同时操作时会 fail closed，不会以扩大命中范围冒充 PASS。

物理路径同时暴露了独立的 upstream interop defect：锁定的 winit 0.30.13 在
`handle_os_dragging` 中把栈上 `POINTS` 的地址直接转换成 `WM_NCLBUTTONDOWN` 的 `LPARAM`，而
Win32 要求 low/high word 中的 signed screen coordinates。Phase 14 因此在已有
`native_message.rs` adapter 内消费这一个 malformed queued message，以 queued `MSG.pt` screen point
构造合法 `LPARAM` 并同步重发；其它消息仍由 winit 正常 dispatch。该修正新增少量 product
adapter runtime code，但不改变窗口 state authority、不新增 dependency，也不复制 move/resize
状态机。资源 baseline/warm-up 修正本身仍是 product runtime delta 0。
