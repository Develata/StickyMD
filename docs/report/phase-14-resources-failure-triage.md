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

## `0a2aa67` Candidate Follow-up

source `0a2aa673aa9e19ce13b859768eb33254f1d70b5d` 的完整 Resources 重跑暴露两项低频
window harness 失败：一次停靠窗口已处于 Left 3-DIP sensor，但 10 秒内未收到展开事实；另一次
Preview/image 压力循环最后切回 Source 后，`config.toml` 在 10 秒内未出现
`view_mode = "source"`。二者均没有伴随产品崩溃、数据损坏或 reducer 单测失败。

为拆分产品状态与验收输入，新增独立 `view-mode` reducer。每轮都验证 Preview/Source durable
config，首个 Preview 和最终 Source 另以真实 Ctrl+A/C clipboard probe 验证画面投影。对照结果：

| Path | Independent runs | Stress | Result |
| --- | ---: | --- | --- |
| View mode | 3 | 1 Preview -> Source | PASS 3/3；首次初始化 `+69 handles / +2 USER` |
| View mode | 5 | 100 Preview -> Source | PASS 5/5；`+69..70 handles / +2 USER` |
| Config-only controls | 3 | opacity 200 | PASS 3/3；`+0..2 handles / +0 USER` |
| Left sensor | 3 | collapse/reveal 1000 | PASS 3/3；无对象增长 |
| Image/view reduction before fix | 3 | reload 100；image Preview -> Source 100 | FAIL 3/3 at image 60/16/34；Preview alt text remained selected |
| Image/view reduction after fix | 3 | reload 100；image Preview -> Source 100 | PASS 3/3；`-3..-4 handles / +3 USER` |

1 次与 100 次 view-mode 的对象增量相同，证明约 69 个句柄是首次 Preview 字体/渲染子系统的
稳定初始化集合，不是按切换次数增长的泄漏；正式 window Resources 在 measurement baseline 前已
执行 Preview/image warm-up，因此仍比较相同初始化集合。

剩余根因位于 smoke Win32 input bridge：view control 原先只伪造 client move/down/up。即使改成
同步 `SendMessageW`，它也只保证 native wndproc 返回；winit 仍把输入事实交给应用事件循环，而产品
按自己最后消费的 `CursorMoved` projection 做 hit-test。在这段边界中真实 cursor event 可以覆盖伪造
位置，使按下事件看到正文/边缘坐标而漏掉 Source。失败时 clipboard 稳定保留 8-byte Preview 图片
alt text，证明 runtime 未进入 Source，而不是 config 写入失败。

最终修正让 Source/Split/Preview qualification 走真实物理 cursor：按共享 toolbar geometry 计算控件
中心，验证命中 HWND、pointer cursor、foreground/active/focused/capture，再执行真实按下/释放。
sensor reveal 原先把 client rectangle 外坐标的 `WM_MOUSEMOVE` 当成 tracked leave，但 winit 消费的
离开事实是 `WM_MOUSELEAVE`；现在在物理 cursor 已离开后显式投递。普通内容激活只在远离
toolbar/resize 边界的安全区域允许 8 px 桌面 jitter，并把实际观察坐标投影给产品。所有路径仍有
有界 deadline，真实窗口、配置与 projection 仍分别验证，没有增加产品 test IPC 或放宽产品 gate。

- product runtime delta：0。
- runtime dependency delta：0。
- verification tooling：新增 typed `view-mode`/image stress，并修正 native input delivery。
- manual Tier A/B/C：保持 `NOT TESTED`。

后续完整 lifecycle reduction 把 clean reload、conflict、image/view 各 100 次合并到同一 copied-Release
实例。它同时暴露一次环境焦点在 source observation 与真实 Ctrl+A 之间被夺走的竞态；projection
probe 现在在每次 mutation 前用真实正文点击重建 foreground/active/focused，并在相同 10 秒上界内
对 input-route loss 重试。Enter/F6 也改为真实物理按键，不再以 queued key message 代替用户输入。
修正后该完整短路径 PASS 2/2，末值为 `-5/-3 handles` 与 `+1/+2 USER`，没有增长。

最终 `phase-14.ps1 -Resources -ResourceModule window` 完整通过：

- window stress：collapse 1000、tray 100、controls 100、clean reload 100、conflict 100、image 100；
- object gate：warm baseline 与末值均为 `435 handles / 19 GDI / 29 USER`；
- private bytes：`16,408,576 -> 17,309,696`，低于 8 MiB growth gate；
- visible Source idle CPU p95 `0.003906%`；collapsed p95 `0.006510%`；tray p95 `0%`；
- startup-to-paper：median `350.151 ms`，max `374.939 ms`，低于 550 ms hard boundary；
- 5 个 visible/collapsed/tray resource repetitions 全部完成。

## Repeated Desktop Jitter Disposition

USER 随后批准独立重复桌面实验的经验处置：样本至少 100，成功率严格 `>98%` 可直接
`PASS WITH RECORDED JITTER`；严格 `>95%` 但不超过 98% 时为
`USER VERIFICATION REQUIRED`；其余 FAIL。Rust CLI 使用整数比例比较，避免浮点边界漂移，并以
命名测试固定 99/100、98/100、96/100、95/100 边界。

该路径只接受集中 classifier 明确识别的 physical cursor、foreground/focus、Windows desktop
scheduling 抖动。任一 canonical content、persistence、crash、cleanup、identity、resource/performance
hard gate 或未知失败都会把整组判为 blocking FAIL。完整 Resources 本身不被放宽；只有独立
copied-Release reducer 达到样本门槛后才能形成 jitter disposition。95–98% 仍以非零退出码等待
USER 对 exact evidence 作出明确决定。该政策不宣称是严格的正态三西格玛统计推断。
