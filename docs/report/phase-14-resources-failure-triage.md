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
