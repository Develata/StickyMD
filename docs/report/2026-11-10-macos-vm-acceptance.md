# macOS 虚拟机验收（2026-11-10）

在 Tart 虚拟机里运行 `stickymd-smoke mac-gui`（外部观察的图形验收）与 `mac-resources`（资源测量）的结果与过程。只记录这台虚拟机上观察到的事实；它不是资格化 suite，也不是 USER 的人工确认。

## 环境

| 项 | 值 |
| --- | --- |
| 虚拟机 | Tart 2.41.1，`mac-test`，克隆自 `ghcr.io/cirruslabs/macos-golden-gate-vanilla@sha256:b35695ce30453bcc1db961a476740be72a44c2a2e464b73ece1ff0fcac12a873` |
| 系统 | macOS 27.2（26B123），`VirtualMac2,1`，Apple M6（Virtual），4 vCPU，8 GiB |
| 显示 | 主显示 1024×768 点，scale 2（2048×1536 像素） |
| 会话 | admin 自动登录，桌面未锁；输入源 ABC；`kern.hv_vmm_present=1` |
| 权限 | USER 在虚拟机里给 `/usr/libexec/sshd-keygen-wrapper` 开了辅助功能（macOS 27 的页面名为 Device Control and Data Access）；PostEvent 随之生效；录屏未授权 |
| 驱动 | 经 SSH 运行，unwind release 构建 |

## 图形验收

最终一次运行：driver `8efed53`（unwind 构建 SHA-256 `ede4415513e78e90603a2740cb276c59ed21d09bb54dfa33bb1390f0102a6758`），app 为 `f664f03` 的 release 包（可执行文件 SHA-256 `e3675643d02e8c55f28ff05c0adfa62b272e1a72256bdc83decf9135cad4fa47`，bundle manifest `2b1446adb0fa1678d271c34b791f23dc8a988f8a68eeee83162ae03dcda46af5`）。证据 `mac-gui.json` 的 SHA-256 为 `ca2ed87f50db0dce67dc15a0858f013ea668f8fb209160ec813ebe7173a49f82`，保存在本机被忽略的 `dist/evidence/mac-vm-2026-11-10/mac-gui/`。结果 PASS 8、FAIL 0、NOT_TESTED 12，退出码 0。

| 行 | 结果 | 观察 |
| --- | --- | --- |
| P16-W01 | 部分 | `lsappinfo` 类型为 `UIElement`，Dock 的 AX 列表里没有 StickyMD；Cmd+Tab 画面未检查 |
| P16-W02 | PASS | 左、右、上贴边各收起为 2 DIP 细条；上边层级 27（菜单栏之上）；配置记下 `dock_edge` |
| P16-W03 | PASS | 悬停起动在 100 ms 附近（夹在采样区间内，容差 −10/+70 ms），约 200 ms 展开完毕；前台仍是 Finder；离开 500 ms 后收回 |
| P16-W04 | PASS | 鼠标停在屏幕顶边超过 500 ms 不收回；离开后按时收回 |
| P16-W07 | PASS | 托盘「隐藏」后约 0.67 s 窗口离屏；再次 `open` 同一 `.app`，仍是同一 PID，取得前台 |
| P16-W08 | PASS | 托盘三项；隐藏、显示回原位与原透明度、置顶层级 0→3→0、配置同步；退出后进程结束、正文已保存 |
| P16-W09 | PASS | 透明度 40/70/100 的窗口 alpha 与配置一致 |
| P16-W11 | 部分 | Cmd+Q 干净退出、正文不变；保存失败时取消退出未测（需要故障注入） |
| P16-K02 | PASS | 真实 Cmd 键：保存（自动保存之前写盘）、撤销重做、全选、复制剪切粘贴、行首行尾、查找替换、视图切换、缩放 |
| P16-E01 | PASS | 保存面板取消、确定，导出的 Markdown 字节正确 |
| P16-F01 | 部分 | 字体日志为 PingFang SC 与 Times New Roman（`02` 的 macOS 默认）；日志不报代码字体，Menlo 与逐字形外观未检查 |
| 其余 | NOT_TESTED | W05、W06、W10、W12、W15、I01–I03、R03 按设计跳过，原因写在证据里 |

## 资源测量

app 为 `a332aae` 的 release 包（可执行文件 SHA-256 `daab657592f110c1bd76266b08cf9f217bb35c1ef19bb7dfd908ad89d66c85f2`）；此后到 `f664f03` 的产品改动只有换行规则。每种视图 5 次，均有效；启动后等 30 s，测 60 s 以上。证据 `mac-resources.json` 的 SHA-256 为 `193db863e50e7c55d16a42afda99a4c255074e9ad08e7964782339d3452732c1`，保存在 `dist/evidence/mac-vm-2026-11-10/mac-resources/`。

| 视图 | 内存 中位 / 最大 | 临时硬门 | 空闲 CPU 中位 / 最大 | 中断唤醒 / 空闲唤醒（次/秒，中位） |
| --- | --- | --- | --- | --- |
| Source | 47.05 / 47.11 MiB | 72 MiB | 0.073% / 0.083% | 1.82 / 1.65 |
| Preview | 50.36 / 50.50 MiB | 96 MiB | 0.001% / 0.002% | 0.03 / 0.03 |
| Split | 50.38 / 50.41 MiB | 128 MiB | 0.103% / 0.104% | 1.82 / 1.73 |
| Hidden | 未测 | 48 MiB | — | — |

`10` 的空闲 CPU 目标为 ≤ 0.05%，超过 0.1% 为硬门失败。Source 超过目标、低于硬门；Split 超过硬门。Preview 没有光标，几乎为零，说明这部分开销来自光标闪烁（约每 0.6 s 一次唤醒）。

### 空闲 CPU 的去向（采样，推断）

用保留符号的同源 release 构建，在虚拟机里以 Split 与 Source 各空闲 35 s 后用 `/usr/bin/sample` 采 15 s（1 ms 间隔）。主线程约 8 800 个样本中只有个位数不在等待中，统计上很薄，以下是推断：

- 忙碌样本都在 winit 的 run loop observer（`control_flow_end_handler`）之下：`sync_native_caret_overlay` → `CaretOverlay::toggle` → `CaretLayer::toggle` 的 `CA::Transaction::commit`，即 `10` 选定的光标图层开关，没有整窗重绘。
- 同一次唤醒里还调用 `update_window_title`、`preview_layout_is_current` 与一次 `Window::inner_size`，样本里都很轻。
- 主机探针给出的图层开关是 0.037%；虚拟机里 Source 约为其两倍。差距更可能来自虚拟机里每次 CA 提交（发往 WindowServer 的 mach 消息）更贵，未在实机上核对。

可能的改法是把闪烁交给 render server：给光标图层加一个无限重复的不透明度动画，输入时重置相位，闪烁期间 app 不再唤醒。这会改变 `10` 选定的实现方式，尚未决定。

## 过程中发现并修复的问题

驱动（不是产品）的问题，各自在真实虚拟机里复现后修复：

- 启动时要求进程只有一个窗口，但 AppKit 会给进程挂一个从不显示的 500×500 窗口；启动时只数屏幕上的窗口（`b9f16a9`）。
- `lsappinfo info` 不接受 `pid=N`；先用 `lsappinfo find` 取 ASN（`b9f16a9`）。
- 用 AX 打开状态栏菜单返回 -25204（回复要等菜单追踪结束），菜单其实已打开；记为未确认，由之后的观察判定（`b9f16a9`）。
- 托盘命令有菜单闪烁、淡出与窗口动画；改为 2 s 内轮询效果并记录用时（`b9f16a9`）。
- F01 把西文字体期望写成 Menlo；按 `02` 改为 Times New Roman（`b9f16a9`）。
- macOS 中途弹出的 Tips 窗口挡住点击；按下前检查落点下的前端窗口属于产品（`8efed53`）。
- 拖动按下后立即移动，产品开始原生拖动时已经错过一步 25 DIP；按下后停 60 ms 再移动（`8efed53`）。

产品行为的问题：

- macOS 上没有换行的已有笔记按回车写入 CRLF。USER 决定平局改用平台的首次创建换行（`f664f03`，`05` 已修订）。

环境问题：

- 虚拟机开机约一分钟后自动下载并准备 macOS 27.2.1 更新，Preboot 卷涨到约 22 GB，磁盘一度只剩 160 MiB，一次运行因写证据失败中断。更新准备失败后自行清理，之后未再发生。未改虚拟机的更新设置。
- 在虚拟机里调用 `swift`、`nm` 会弹出安装命令行工具的对话框；已关闭。虚拟机不装开发工具。

## 未完成

- 空闲 CPU：Split 超过硬门，Source 超过目标（见上节）。USER 在主机桌面上看到的 Source 1.38% 在虚拟机里没有复现，原因未知。
- Hidden 内存、W01 的 Cmd+Tab 画面、W11 的保存失败、F01 的 Menlo 与字形外观、W05、W06、W10、W12、I01–I03、R03。
- 这些结果来自外部观察；`11` Phase 16 的 M2 出口要求真实桌面项由人工确认，`acceptance-cases/AGENTS.md` 也不允许自动化代替人工验收，所以 `phase-16` 的人工行保持 `NOT TESTED`，只在证据列记下这次观察。P16-R02 超过硬门，记为 `BLOCKED`。
