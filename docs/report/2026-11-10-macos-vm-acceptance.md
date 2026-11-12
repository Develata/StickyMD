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

## 更正与补充（2026-11-10，提交 `af0fe03` 之后）

- **图形验收的数字**：上表的 W03、W07、W08 数字取自修复前的一轮（gui-4），而上文引用的证据哈希属于最终一轮（gui-8）。以 gui-8 为准：
  - W03：悬停起动夹在 94–126 ms，207 ms 展开完毕（期限 304 ms）；离开后起动夹在 497–531 ms，657 ms 收回（期限 707 ms）。
  - W04：起动 97–129 ms，215 ms 展开完毕；离开后起动 492–534 ms，649 ms 收回（期限 702 ms）。
  - W07：托盘「隐藏」后 664 ms 窗口离屏，含菜单闪烁、淡出与窗口动画。
  - W08：隐藏 654 ms；显示回原位与原透明度 485 ms；置顶层级 0→3 用 381 ms，取消 394 ms。
  - W02：左、右、上收起分别用 457、475、455 ms。
- **拖动停顿是推断**：原因“产品处理按下时才开始原生拖动，已错过一步 25 DIP”是推断，未证实。依据是失败时窗口恰好少走 25 DIP（一步）。旧时序下 W02 两次通过、两次失败；加 60 ms 停顿后连续 4 次通过（3 次单项、1 次整套）。
- **P16-R02 改回 `NOT TESTED`**：`mac-resources` 按工具说明只报告 CPU、不做判定，也不更新 R01/R02。这次读数只在虚拟机里取得，而且只比硬门高 3%。行的状态改回 `NOT TESTED`，超门的观察留在证据列，是否按失败处理由 USER 决定。
- **主机上的 1.38%**：它接近 `10` 记录的整窗提交（1.94%），而不是图层开关（0.037%）；虚拟机里的采样走的是图层开关路径。因此主机上可能另有整窗提交路径在工作，这只是推断，需要在实机上复测或采样。
- **同类问题的排查**：`EditorCoordinator::empty` 与 `LineEnding` 的 `#[default]` 仍为 CRLF，只有测试调用，生产路径碰不到。
- **未编译的部分**：`stickymd-win` 的构建脚本需要 `winresource`，在 macOS 上无法交叉检查。`f664f03` 对它的改动已逐行核对过，仍需在 Windows 或 CI 上构建并运行 `cargo test -p stickymd-win`。

## Resolution：第 6 轮 codex-reviewer 审查（2026-11-10）

审查 `a332aae..d953c92`，6 条发现逐条核实：

| 发现 | 核实 | 处理 |
| --- | --- | --- |
| BLOCKER：P16-R02 是已知的硬门失败，不应记 `NOT TESTED` | 属实。`11` 第 325 行：`BLOCKED` 为“自动化或人工验证已知失败，或环境阻止执行”。按原始 ticks 重算，Split 五次为 0.0916%–0.1042%，中位 0.1025% | 改回 `BLOCKED`，注明测量对象为 `a332aae` 的 release、当前源码未复测。上文“改回 `NOT TESTED`”一条作废 |
| 落点检查跳过所有 Dock 窗口，矩形包含不等于系统的点击命中 | 属实 | 改用系统辅助功能的命中查询 `AXUIElementCopyElementAtPosition`，要求命中元素属于产品 PID；去掉按窗口列表推断的逻辑 |
| 轮询超时后才见到的成功仍判通过 | 属实（`until` 只返回样本，调用方不看用时） | 只有在期限内返回且满足条件的样本才算达成；W02、W07、W08 的检查都改用这个结果；补测期限前、期限上、跨期限的假后端测试 |
| “展开完毕”实为“完整进入屏幕” | 属实：W04 在 y=12 时记为完成，静止位置是 y=30 | 完成时刻改为回看：期限后再采 200 ms，最后一个样本为静止位置，完成时刻为其后全部相同的第一个样本；补测“在屏幕上但未到位”的假后端。上文 W03、W04 的“展开完毕”应读作“首次完整进入屏幕” |
| 60 ms 停顿的原因写成了事实 | 属实（代码注释与 README） | 改为推断；不停顿的快速拖动列为未诊断 |
| `stickymd-win` 没有构建证据 | 属实 | 无法在 macOS 上处理，仍需 Windows 或 CI |

按新规则重跑（driver 构建 SHA-256 前缀 `cc4a1d24`，app 同上）：PASS 8、FAIL 0、NOT_TESTED 12。W03 悬停后 256 ms 到达静止位置（期限 304 ms），离开后 650 ms 收回到位（期限 727 ms）；W04 分别为 250 ms（期限 327 ms）与 666 ms（期限 732 ms）。证据 SHA-256 `525850f36de1b21eea8215e0af3a7d70218c57f4fc80a65d2bc548411529ff65`，保存在 `dist/evidence/mac-vm-2026-11-10/mac-gui-r6/`。

## Resolution：光标闪烁改由系统合成器播放（2026-11-11）

USER 批准按上文建议修改（`09` macOS 节、`10` macOS 门已记录）。共享层给 `CaretOverlay` 加 `blinks_itself()`，默认 `false`，Windows 不变；macOS 返回 `true`，shell 不再按 550 ms 唤醒，光标层显示时挂离散的不透明度关键帧动画（亮、灭各 550 ms），由系统合成器播放，每次显示从“亮”开始。

同一虚拟机（自动下载已按 USER 决定关闭），未提交的工作区构建，app 可执行文件 SHA-256 `240d104b2a82a741743a830b812e0e8eaf3357bd32f6aaa61bbdbc7692cff41f`：

- debug 构建以 Split 运行 25 s，进程存活，stderr 只有字体日志，没有 Objective-C 类型或异常错误。
- `mac-resources`，每种视图 5 次，均有效（证据 SHA-256 `e27dd4b8d6b615dd91973444d0c66af005afb8bf33458cbbfa1630eb5a6d05a2`，保存在 `dist/evidence/mac-vm-2026-11-10/mac-resources-r2/`）：

| 视图 | 内存 中位 / 最大 | 空闲 CPU 中位 / 最大 | 中断唤醒 中位（次/秒） |
| --- | --- | --- | --- |
| Source | 47.24 / 47.28 MiB | 0.001% / 0.002% | 0.033 |
| Preview | 50.53 / 50.77 MiB | 0.002% / 0.002% | 0.033 |
| Split | 50.58 / 50.64 MiB | 0.001% / 0.001% | 0.033 |

- `mac-gui` 整套：PASS 8、FAIL 0、NOT_TESTED 12（证据 SHA-256 `34b355e46388b064d31ceefa10b4e63899d769cb778ee600950b5cbfab2c5088`，`dist/evidence/mac-vm-2026-11-10/mac-gui-r7/`）。

未验证：光标是否真的可见且在闪。虚拟机没有录屏权限，外部观察看不到图层内容；空闲 CPU 低同样可能是光标没有显示。P16-R02 因此改为 `NOT TESTED`，待 W10 人工确认光标可见、按约 0.55 s 闪烁后再判定。

## Resolution：第 7 轮 codex-reviewer 审查（2026-11-11）

审查 `0fa8754`（光标闪烁改由系统合成器播放），结论无阻断；Core Animation 调用的参数类型、所有权与离散关键帧规则未发现问题，plan 09 全文摘要的更新经重算属实。三条发现：

- 光标是否可见并在闪需要人工确认：属实，P16-R02 已为 `NOT TESTED`，等 W10。
- 测量对象未与提交绑定：从干净工作区的 `0fa8754` 重新打包，可执行文件 SHA-256 仍为 `240d104b2a82a741743a830b812e0e8eaf3357bd32f6aaa61bbdbc7692cff41f`，且打包后到提交之间没有代码改动，测量对象即该提交。
- 新测试只验证判定函数：属实。把事件循环的唤醒计算提为 `advance_and_next_wake`，`about_to_wait` 直接调用它，新增测试检查三种情形下实际等待的时刻、是否推进闪烁与是否翻转。

## 人工收据：光标闪烁（2026-11-11）

USER 在本机桌面运行 `dist/macos/StickyMD.app`（`0fa8754` 的 release，可执行文件 SHA-256 `240d104b2a82a741743a830b812e0e8eaf3357bd32f6aaa61bbdbc7692cff41f`，与上文 VM 资源测量为同一二进制），点进正文后确认：光标可见，闪烁正常。据此 P16-W10 记 `MANUAL PASS`；P16-R02 的 VM 读数（0.001%–0.002%）对应的是在闪的光标，记 `AUTOMATED PASS`。`04d7396` 只把唤醒计算提为方法并补测，行为不变，未再做人工复查。

## Resolution：宿主共享后的最终二进制复跑（2026-11-11）

对象：`10127b2` 的 release。用 `apps/stickymd-mac/packaging/build-app.sh` 在干净工作区重新打包，与 `dist/macos/StickyMD.app` 逐字节相同（`diff -r` 无差异）。可执行文件 SHA-256 为 `b13088953a90e4df8ed57f5835f5ddd14641c6774e0bc096035df94e98060b56`。驱动程序为 unwind 构建，SHA-256 前缀 `c7b22bb4fe0eef59a1dc`。用的是同一台虚拟机，自动下载已关闭。

主机连不上虚拟机是因为 macOS 的“本地网络”隐私权限：路由和 ARP 都正常，但从 Claude Code 进程发出的连接被内核以 `No route to host` 拒绝；同一时刻由 launchd 启动的 `nc` 能连通 22 端口。按 USER 的要求，SSH 主连接改由 Terminal.app 打开，测试命令只复用它的本地套接字。这与产品无关。

- `mac-resources`：每种视图 5 次，全部有效，结论 `VISIBLE_SCENARIOS_PASS`（`acceptance_status` 仍为 `INCOMPLETE`，与上一轮相同）。证据 SHA-256 `aa20c4e02dbe8fcf6611eda981e134ae6375dc46b2330facbe683e065a942dfb`，保存在 `dist/evidence/mac-vm-2026-11-10/mac-resources-r3/`。

| 视图 | 内存 中位 / 最大（上限） | 空闲 CPU 中位 / 最大 | 中断唤醒 中位 / 最大（次/秒） |
| --- | --- | --- | --- |
| Source | 46.96 / 47.16 MiB（72） | 0.001% / 0.001% | 0.033 / 0.050 |
| Preview | 50.19 / 50.39 MiB（96） | 0.001% / 0.001% | 0.033 / 0.033 |
| Split | 50.36 / 50.52 MiB（128） | 0.001% / 0.001% | 0.033 / 0.033 |

- `mac-gui` 整套：PASS 8（W02、W03、W04、W07、W08、W09、K02、E01）、FAIL 0、NOT_TESTED 12。W03 悬停后 261 ms 到达静止位置（期限 305 ms），离开后 654 ms 收回到位（期限 730 ms）；W04 分别为 279 ms（期限 325 ms）与 664 ms（期限 727 ms）。证据 SHA-256 `036b3529bd971931fc63f93b78768557528c4fe1b8cc2528fd66c64928e3511c`，保存在 `dist/evidence/mac-vm-2026-11-10/mac-gui-r8/`。

结论：宿主共享的两步没有改变本轮虚拟机能观察到的 macOS 行为，资源读数与 `0fa8754` 那一轮持平。这不覆盖 Windows；Windows 桌面回归仍是 P16-W15。

## 人工收据：macOS 桌面人工项（2026-11-11）

USER 在本机桌面运行 `dist/macos/StickyMD.app`（`10127b2` 的 release，可执行文件 SHA-256 `b13088953a90e4df8ed57f5835f5ddd14641c6774e0bc096035df94e98060b56`，与上文最终复跑为同一二进制），逐项试过 W01–W09、W11、W12、K02、E01、F01、I01–I03。USER 原话：“其它的我试过了，都没什么问题”“我都人工试过了，确实没什么问题”。

据此记 `MANUAL PASS`：W01、W02、W03、W04、W05、W06、W07、W08、W09、W11、W12、E01、F01、I01、I02、I03。

不记的两项：
- K02：USER 只用到撤销 Cmd+Z，问到“没有下一步的快捷键吗”、Cmd+F 只出现查找。按 `07` 第 213 行，重做是 Cmd+Shift+Z，替换是 Cmd+Option+F，这两个键 USER 未试，K02 待确认后再记。
- R03：需要签名身份，ad-hoc 签名的包无法构成这一场景，保持 `NOT TESTED`。

同时报告的另一问题不属于本矩阵：`\[ … \]` 中独占一行的 `=` 点 `$` 按钮没有反应。在共享流程中复现，两宿主相同：分隔符转换按 Preview 语义解析，裸 `=` 使上一行成为 Setext 标题，`\[` 不再是数学节点，等号连接只处理 `$$` 块，结果为 no-op。不是 macOS 回退；按 `07` 当前契约属于未覆盖的情形。

补记（同日）：USER 得知重做为 Cmd+Shift+Z、替换为 Cmd+Option+F 后，原话“把 K02 也记为通过”。K02 据此记 `MANUAL PASS`，二进制同上。

## Resolution：新增 W06、W11、W12、F01 的自动化与整套复跑（2026-11-11）

按 USER 批准的方案（“按方案做”），`mac-gui` 新增四个场景，产品代码不变：

- **W06**：Esc 手动收起后切到 Finder，再只经 AX 取回焦点，不重新打开 bundle。从请求焦点那一刻起，每 20 ms 采样一次，持续 800 ms：窗口不得展开，细条层级不得降低；样本间隔超过 60 ms 记为 NOT_TESTED。最后检查悬停仍能展开。切换 Space 不驱动。
- **W11**：在测试副本中先写入旧字节，再去掉 note 目录的写权限，制造保存失败。这是 USER 批准的故障注入，由 `fault.rs` 经不跟随链接的描述符修改权限，守卫在清理前（包括栈展开时）恢复权限。检查顺序：
  1. 全选后输入新文本，按 Cmd+Q：程序应取消退出，标题为“退出已取消：保存失败”，磁盘仍是旧字节；
  2. 恢复写权限后按 Cmd+S，新文本落盘，标题恢复；
  3. 再按 Cmd+Q，干净退出。
- **W12**：按 PID 发系统终止请求（`NSRunningApplication.terminate`），不按名字。
  1. 先测保存失败时请求被拒；
  2. 再打开导出对话框，并把它对准会话中的 `export.md`，然后发请求；
  3. 对话框没有关闭时，由驱动手动取消，确认关闭后再计时，看程序是否退出。

  注销不驱动。
- **F01**：Source 视图下 Menlo 不应被映射进进程（反向对照），切到 Preview 排出代码后应被映射。这只说明 Preview 加载了 Menlo，不说明代码字形用的是它。

所有按键注入前都核对接收方：产品是前台应用，且焦点窗口是目标窗口（便签窗口或导出对话框）。输入文字时逐字核对。这是 `11` 物理输入一节的要求，原驱动只核对鼠标落点。

驱动经 codex-reviewer 三轮审查修正（发现与修正见本节后面的审查记录）。最终整套运行 r12 的数据：

- **被测程序**：`10127b2` 的 release，可执行文件 SHA-256 `b13088953a90…`，与上文相同。
- **驱动**：SHA-256 `37b49e5a8e925d7069755129284e701d77db78b3d6fdc0d42113c9ce5b84a361`，记录在证据目录的 `driver.sha256`。
- **结果**：PASS 9、FAIL 1、NOT_TESTED 10。
- **证据**：`mac-gui.json` 的 SHA-256 为 `39b85576c54032c10dd982a523b605b51acf31d0c80a0270642cecdae6b35f5d`，保存在 `dist/evidence/mac-vm-2026-11-11/mac-gui-r12/`。

各行结果：

- **W06**：除切换 Space 外全部通过。
  - Esc 后第一次观察到完全收起在 79 ms；
  - 切走后与取回焦点后，各 800 ms 内的最大样本间隔为 36 ms 与 31 ms；
  - 悬停 250 ms 后展开。
- **W11**：全部通过。
- **F01**：Menlo 映射检查通过；逐字形检查仍为 NOT_TESTED。
- **W12**：
  - 保存失败时请求被拒：通过；
  - 导出对话框打开时，请求没有关闭对话框；手动取消、确认关闭后，10 s 内程序也没有退出。两项记为失败；
  - 对准的 `export.md` 没有生成。

  根因与修复候选见 [RISK：导出对话框打开时，系统的退出请求没有生效](RISK-macos-modal-termination.md)。P16-W12 因此改为 `BLOCKED`。
- **其余行**：与上一轮相同。中间几轮里，W04 有一次“离开后开始收回”的采样区间跨过了判定上限，记为无法判定，不算失败；W04 的代码没有改，最终一轮通过。

调试中还修正了一个驱动问题：对话框弹出期间，AX 读取会返回 -25204（暂时无法完成），轮询改为把它当作“这一次没看到”，不再中止场景。

中间运行 r9、r10、r11（`mac-gui-r9`…`mac-gui-r11`）用的驱动在审查前，或在审查修正的中途，只作过程记录：

- W06：r9 只在等待结束时各采样一次；r10 改为连续采样，但请求焦点后先等 100 ms 才开始；r11 才从请求那一刻起采样并统计间隔。
- 按键接收方：r9、r10 不核对，r11 只核对到应用级。
- 三轮都没有把导出对话框对准 `export.md`。

### 审查记录（codex-reviewer，2026-11-11）

| 轮次 | 发现（均已核实属实） | 处理 |
| --- | --- | --- |
| 1 | BLOCKER：故障注入后栈展开会跳过权限恢复 | 改为守卫：声明在会话之后，先于会话析构；补真实目录的栈展开测试，并核对 panic 内容 |
| 1 | 按路径改权限会跟随符号链接 | 先核对会话目录身份，再以 `O_NOFOLLOW` 与 `O_DIRECTORY` 打开 note，在描述符上改权限 |
| 1 | F01 把字体被映射说成被选用 | 检查改名为“加载”，注明不证明选用 |
| 1 | 空的旧笔记查不出“先截断再失败”的保存 | 旧笔记改为非空，比较精确字节 |
| 1 | 标题读不到时，“标题已恢复”会空过 | 读不到记为 NOT_TESTED |
| 1 | W06 只采样两个时刻 | 改为连续采样 |
| 2 | BLOCKER：按键注入前不核对目标（`11` 第 303 行） | 每次按键前核对前台应用与焦点窗口，输入文字时逐字核对 |
| 2 | W06 在焦点请求后有一段没有采样 | 焦点请求不再等待；统计样本间隔，超过 60 ms 记为 NOT_TESTED |
| 2 | 手动取消后没有确认对话框已关闭 | 确认关闭后才开始计时，否则记为 NOT_TESTED |
| 2 | RISK 报告漏了 `preventsApplicationTerminationWhenModal` | 补入并在临时构建上验证 |
| 3 | BLOCKER：“没有导出”查的文件不是对话框会写的文件 | 发请求前把对话框对准会话中的 `export.md` |
| 3 | BLOCKER：RISK 报告把旧驱动的探针说成确认过关闭 | 改以 r12 为关闭后的证据，探针只用于证明回调没有被调用 |
| 3 | 标题轮询中途读不到时会误判失败 | 只有截止时读到了、且没有取消状态才算失败 |
| 3 | 按键只核对到应用级 | 同时核对焦点窗口，输入文字时逐字核对 |
| 3 | 本节旧数字取自另一轮运行 | 改为引用 r12 的证据，并单独记录驱动哈希 |

仍未核实：Automation 授权在其他环境里是否会弹窗；真实注销；切换 Space；逐字形的字体选择。

## Resolution：W12 修复后的复跑（2026-11-12）

按 USER 批准的方案 A（“按方案 A 修”）修复导出对话框打开时的系统终止，详见 [RISK 报告](RISK-macos-modal-termination.md) 末节。修复后 release 的可执行文件 SHA-256 为 `5cd37a465990f295e9a47fbe751c23b969bdcaaed0fbc273cd081550494bc792`。

在此之前，第 4 轮审查发现驱动还有问题，已改：

- 导出对话框原先只要窗口标题不是便签就放行，现在按 AX 标识正向识别（`save-panel`）；未知窗口和无标题窗口一律拒绝。
- “前往文件夹”的回车，现在只在焦点是路径输入框、且框里正是刚设好的路径时才发出。
- 对准导出文件后，回读文件名和位置弹出框加以确认。
- 取消标题轮询中途读到空标题时，按“没有观察到”处理。

证据都在 `dist/evidence/mac-vm-2026-11-12/`。各目录的 `driver.sha256` 记有驱动哈希。

- **`mac-gui-r13`（驱动 `7848baa9…`）**：PASS 9、FAIL 0、NOT_TESTED 11，`mac-gui.json` SHA-256 `4a996392a7bdff21d815d231d58bc68843a4b3753d747996f6feaa79d56258f4`。
  - W12 只剩注销一项未测，其余全部通过，因此这一行判为 NOT_TESTED；
  - E01 通过；
  - 其余行与 r12 相同。
- **`mac-gui-r13-w12-e01`**：用最终驱动 `a1bc2a7f…` 只重跑用到对准步骤的 W12 与 E01，结果同上，`mac-gui.json` SHA-256 `771c1fe4bfd777b9cef0c5a3a8aae1973ba4c4db80c1f8773b75be912374a839`。
- **`mac-resources-r4`**：每种视图 5 次全部有效，证据 SHA-256 `a10586f2d177cf3f935f98c06cba3d00cb3c6a01559401149ef6a18499d50b0d`。读数如下表，与修复前持平。

| 视图 | 内存 中位 / 最大 | 空闲 CPU 中位 / 最大 |
| --- | --- | --- |
| Source | 47.11 / 47.20 MiB | 0.001% / 0.001% |
| Preview | 50.46 / 50.56 MiB | 0.002% / 0.002% |
| Split | 50.46 / 50.60 MiB | 0.001% / 0.001% |

**更正**（第 4 轮审查指出，已核实）：上一节 W06 的“79 ms”是从 Esc 按键操作返回算起，到第一次观察到完全收起为止；从 Esc 按下算起是 173 ms。这两个数都不能当作动画本身的耗时。

**人工收据**：上文的人工收据都针对修复前的可执行文件 `b13088953a90…`。按 USER 批准的复用规则，人工收据绑定 app 的 SHA-256，产品有任何改动都要重新确认。

## Resolution：注销的 SIGTERM 修复与复跑（2026-11-12）

r13 之后，真实注销暴露了另一个问题：loginwindow 过了不可撤销点才向 StickyMD 发退出请求，而且不等回复；用户启动的实例若还没退出，launchd 会以 SIGTERM 结束它，退出屏障可能被截断。详见 [RISK：注销时 launchd 结束仍在运行的 StickyMD](RISK-macos-logout-ui-element.md)。USER 批准方案 A：处理 SIGTERM。修复后 release 的可执行文件 SHA-256 为 `4e19774d9ccb9f1542ad0e69a165466e777df688915add029eb36783c6ebc793`。

证据都在 `dist/evidence/mac-vm-2026-11-12/`：

- **`mac-gui-r14`**：被测程序仍是 `5cd37a46…`，驱动 `6db1c899…` 开始把运行记入 `mac-gui` 账本。结果与 r13 相同（PASS 9、NOT_TESTED 11），`mac-gui.json` SHA-256 `7e70db4d034c7b38313d36f2034746dfee450e2834528db87f712a6620f8da9e`。这是账本的第一条运行记录。
- **`mac-gui-r15`、`mac-gui-r15-w12`**：在 W12 中真的发了注销请求，会话结束，驱动也被结束。这两轮只作为上述 RISK 的证据，不是验收运行。
- **`mac-gui-r16`**：中间构建 `a943e45b…` 与驱动 `94ea7f0e…`，PASS 9、FAIL 0、NOT_TESTED 11，`mac-gui.json` SHA-256 `6009e2a570e47a2da0d6ee77e99719f2b25190b246ba65c70a487a4a61837b54`。中间构建出自重做前的提交；它与最终构建只差两条 stderr 文本和注释，驱动只差文字。
- **`mac-gui-r17` 到 `r20`、`mac-resources-r6`、注销探针**：都在 `4e19774d…` 上运行，结果与数据见 RISK 报告末节。概括如下：
  - W12 的系统退出请求与 SIGTERM 检查在 r17、r19、r20 中全部通过；
  - E01 在这台变慢的 VM 上暴露了驱动的固定等待，驱动修复后通过；
  - W03 两次全套各超期限 2 ms、6 ms，单独跑两次都通过；
  - 资源测量与修复前持平；
  - 真实注销时两种启动方式都正常退出；进程冻住时，修复后的版本不再被 SIGTERM 结束，约 1 s 后才被 SIGKILL 结束。
- **VM 变慢**：r16 之后，这台 VM 的保存对话框从约 0.3 s 变成约 0.8–1.3 s 才出现，退出请求的处理也慢了几倍。原因没有查明。

**人工收据**：产品又改动了，按复用规则，所有人工行都要在 `4e19774d…` 上重新确认。

## 人工收据：W12 真实注销（2026-11-12）

USER 让 Claude 直接操控 VM 完成 W12 的注销检查（“你直接操控 vm 就好了”），看过结果后批准记为通过（“Record as pass”）。所以这份收据是 USER 认可的脚本化观察，不是 USER 亲手操作。被测程序 `4e19774d9ccb9f1542ad0e69a165466e777df688915add029eb36783c6ebc793`，Tart 虚拟机（macOS 27.2，26B123）。

**做法：**

- 脚本 `w12-case.sh` 经 SSH 运行，注销后 SSH 会话仍在。
- 每种情况用一个新的程序目录，用 `open` 启动，与 Finder 走同一条 LaunchServices 路径；再打开一次 `.app`，让窗口取得焦点（W07 的行为）。
- 用辅助程序 `w12keys` 经 CoreGraphics 发按键，注入方式与 `mac-gui` 驱动相同。
- 然后向 loginwindow 发注销请求，事后读系统日志与文件。
- 每种情况之前重启 VM，以取得新的图形会话。

| 情况 | 系统日志 | 磁盘 |
| --- | --- | --- |
| 输入后立刻注销 | 08:55:11.715 loginwindow 发退出请求；3 ms 后程序开始处理，+127 ms 回复 `YES`；launchd 记为 `(0, 0, 0)` | 注销请求前仍是旧字节，之后是输入的文本 |
| 导出对话框打开时注销 | 08:59:15.249 发请求；3 ms 后 `applicationShouldTerminate:` 回 `NSTerminateCancel`（对话框按取消结束），注销照常继续；屏障随后保存，约 0.21 s 时 launchd 记为 `(0, 0, 0)` | 两行都已落盘；没有生成导出文件 |
| 保存失败（笔记目录只读）时注销 | 09:04:16.111 发请求；`NSTerminateLater`，8 ms 后回 `NO`，程序继续运行。09:04:17.387 launchd 移除服务（推断此时发 SIGTERM，被忽略）；09:04:18.395 记为 `(2, 9, 9)`，被 SIGKILL 结束 | 仍是此前保存的内容，完整，笔记目录里没有残留的临时文件；故障期间输入的那行丢失，这无法避免 |

**证据：** `dist/evidence/mac-vm-2026-11-12/w12-logout-manual/`，SHA-256：

- `w12-case.sh`：`d58086cfce469b5516e7ec3c8f1b07e8fe4962e4d54994da95f888b906b49c73`
- `main.rs`（`w12keys` 的源码）：`0ad4897fa6f20bc99166e0d9a49ed4805da159856becfd60ee962a7cd4755e0a`
- `w12keys`：`b11c33b7f2cbee00c877624bf4bf6a13533e24f6acfdca6391820f5b834d6dd8`
- `text/system-show.log`：`b40c2ebe8610867192b5d17bab022b3dd6dbe574675e307986b1a692c9e3d731`
- `dialog/system-show.log`：`cef3407b7d280e6585069639934bf052cdac5179531745f5215b0bcc59d55b89`
- `fault/system-show.log`：`bf397acbd405a07fbb445a750a69d83a8059438192575e7e81eec5136e27e883`

**没有观察的：**

- 窗口标题：没有人看，所以保存失败时有没有显示“退出已取消：保存失败”不知道。直接发 SIGTERM 时的标题由 `mac-gui` 的 W12 检查覆盖。
- `osascript` 退出与导出对话框打开时的系统退出请求，由 `mac-gui` r17、r19、r20 的 W12 自动化覆盖。

**账本：** `mac-ledger receipt` 只会把环境记为“USER desktop”，与这次不符，所以这份收据没有写入账本，只记在这里。

**附带：** 第一次尝试用 `osascript` 经 System Events 发按键，在 VM 里弹出了自动化权限提示。提示被关掉，没有回答，既没有授予也没有拒绝。

## 重跑：`320f785` 的 release（2026-11-12）

`2febab1`（数学按钮）与 `320f785`（Comrak 0.59）都改了产品代码，因此在新身份下重跑。

**构建与身份。** 都在干净工作区构建：

- app：用 `apps/stickymd-mac/packaging/build-app.sh` 打包，可执行文件 SHA-256 `a9f4354b80c56a9434a36aca4351707f257c1bb950fdf4d617b4d73404ec9996`。
- 驱动：`CARGO_PROFILE_RELEASE_PANIC=unwind CARGO_TARGET_DIR=target/unwind cargo build --locked --release -p stickymd-smoke`，SHA-256 `7232899214933b8a084a8fb8e7b10328227971762faa1d9797e8af3d640162ac`。

**环境。** Tart 虚拟机 `mac-test`：

- 镜像 `ghcr.io/cirruslabs/macos-golden-gate-vanilla@sha256:b35695ce30453bcc1db961a476740be72a44c2a2e464b73ece1ff0fcac12a873`；
- 系统 macOS 27.2（26B123）；
- 以 `tart run --no-graphics` 启动，每次运行前等 1 分钟负载降到 1.0 以下。

### `mac-gui` r21

结果是 PASS 9、FAIL 0、NOT_TESTED 11。证据 `dist/evidence/mac-vm-2026-11-12/mac-gui-r21/`，SHA-256 `59ffd5ed22c1d9ee829a7b571ba0b4f0f0593489fabb24dede4cd07009396e08`。

- **PASS：** W02、W03、W04、W07、W08、W09、W11、K02、E01。与 r19 比，只有 W03 由 FAIL 变为 PASS；r19 那次是计时超出期限 2 ms，见上文。
- **W12：** 23 项检查中 22 项通过；只有 `logout` 为 NOT_TESTED，按设计由下文的真实注销覆盖。
- **账本：** 已写入账本（`mac-ledger record`，20 行）。`mac-ledger plan` 显示，W07、W08、W09、W11、K02、E01 等自动化行在当前身份下由 r21 的 PASS 判定；需要 USER 的人工行为 W01–W06、W10、W12、I01–I03、F01、R03。
- **验收表：** 已据此更新六个自动化行。

### 资源 `mac-resources-r7`

每种视图 5 次，全部有效；Hidden 仍未测。证据 SHA-256 `7bc08d677eea0217e900f1bb48a0d5fdc5a62993e198197af3fdf32e6a5c8cab`。

| 视图 | 内存中位 | 空闲 CPU 中位 | `4e19774d` 的 r6 |
| --- | --- | --- | --- |
| Source | 47.1 MiB | 0.0004% | 47.1 MiB，0.0007% |
| Preview | 50.4 MiB | 0.0013% | 50.3 MiB，0.0019% |
| Split | 50.3 MiB | 0.0005% | 50.3 MiB，0.0008% |

与修改前持平。按工具说明，这只是观察，不更新 R01、R02。

### W12 真实注销（`a9f4354b…`，待 USER 认可）

做法与上节“人工收据：W12 真实注销”相同：

- 同一个 `w12-case.sh` 与 `w12keys`（SHA-256 与上节一致）；
- 每种情况前重启虚拟机；
- 用 `open` 启动后再打开一次 `.app` 取得焦点，用 CoreGraphics 发键，再向 loginwindow 发注销请求。

| 情况 | 系统日志 | 磁盘 |
| --- | --- | --- |
| 输入后立刻注销 | 14:40:07.742 loginwindow 发退出请求；不到 1 ms 程序开始处理，+68 ms 回复 `YES`；launchd 记为 `(0, 0, 0)` | 注销请求前仍是旧字节，之后是输入的文本 |
| 导出对话框打开时注销 | 14:42:14.788 发请求；1.5 ms 后 `applicationShouldTerminate:` 回 `NSTerminateCancel`（对话框按取消结束），注销照常继续；约 0.39 s 时 launchd 记为 `(0, 0, 0)` | 两行都已落盘；没有生成导出文件 |
| 保存失败（笔记目录只读）时注销 | 14:44:37.990 发请求；`NSTerminateLater`，14 ms 后回 `NO`，程序继续运行；14:44:39.280 launchd 移除服务（推断此时发 SIGTERM，被忽略）；14:44:40.289 记为 `(2, 9, 9)`，被 SIGKILL 结束 | 仍是此前保存的内容，完整；`.trash` 为空，笔记目录里没有残留的临时文件；故障期间输入的那行丢失，这无法避免 |

三种情况的行为与 `4e19774d…` 上的收据一致。

**证据：** `dist/evidence/mac-vm-2026-11-12/w12-logout-a9f4/`，SHA-256：

- `text/system-show.log`：`eae064996d6d74285e518aa4ca60595742c0bac6e2fbcfdfcbcd34fd3737d05d`
- `dialog/system-show.log`：`3f2dcb490893883b85d69d10d5517fcf27731a137209cbe697efc2bd7561f1ff`
- `fault/system-show.log`：`ebd5132fd7adf37378c1933bcf3484863d1afb656d4fef9e1d11fe93a2ed7a0b`

**收据状态：** 人工收据绑定 app 的 SHA-256，所以 P16-W12 在新身份下要由 USER 认可这次脚本化观察后，才能改为 `a9f4354b…` 的收据；在那之前，表中保留 `4e19774d…` 的收据。

**USER 认可（2026-11-12）：** USER 看过上面三种情况的结果后回复“记为通过”。P16-W12 改用 `a9f4354b…` 的这份收据，`4e19774d…` 的收据保留为“此前”。与上一份一样，这是 USER 认可的脚本化观察，不是 USER 亲手操作；`mac-ledger receipt` 只能把环境记为“USER desktop”，所以没有写入账本，只记在这里。

## 更正：人工行状态与 W12 账本（2026-11-12 复审）

- **人工行状态。** codex 复审指出，W01–W06、W10、I01–I03、F01 的收据都来自旧的 app 身份（`10127b2` 等），表中却仍是 `MANUAL PASS`。依据有两条：[plan 11](../plan/11_testing_and_release.md) Phase 16 规定“app 变了，收据不再成立”；`MANUAL PASS` 的定义是“当前候选上”已执行。所以这些行现改为 `NOT TESTED`，旧收据保留为“此前”，等 USER 在 `a9f4354b…` 上确认。
- **W12 账本。** 上文说 `mac-ledger receipt` 只能把环境记为“USER desktop”，所以没有登记，这个判断不对。按 `tools/stickymd-smoke/src/mac_ledger/run.rs` 的说明，环境字段只记录在哪台机器上登记，不证明 USER 在那里测试；测试范围以收据原话为准。
  - 现已用 `mac-ledger receipt --app dist/macos/StickyMD.app --rows W12 --quote "记为通过"` 登记。
  - 登记内容：app `a9f4354b…`，环境“USER desktop, recorded on macOS 27.2.1 (26B7131)”，即登记所在的这台 Mac。被测的是上文所述的 VM 脚本化注销。
  - 登记后 `mac-ledger plan` 不再要求确认 W12。
- **修复提交之后。** 复审修复改了产品代码，W07、W08、W09、W11、K02、E01 与 W12 的证据仍对应 `a9f4354b…`，按 plan 11 改为 `NOT TESTED`，原证据保留为“此前”。新构建在 VM 中重跑、W12 由 USER 重新确认后再更新。

## 重跑：`74b3d74` 的 release（2026-11-13）

复审修复（`9088c57`、`74b3d74`）改了产品代码，所以在新身份下重跑一遍。做法与“重跑：`320f785` 的 release”相同：同一镜像、同一系统（macOS 27.2，26B123），`tart run --no-graphics`，每次运行前等负载降到 1.0 以下。

- **app：** 在干净工作区用 `build-app.sh` 打包，可执行文件 SHA-256 `4ce8a2268b9bc26ae5a87b489c74cb7b68fa020f09b270730a4a931706affd50`。
- **驱动：** 没有变化，仍为 `7232899214933b8a084a8fb8e7b10328227971762faa1d9797e8af3d640162ac`。
- **`mac-gui` r22：** PASS 9、FAIL 0、NOT_TESTED 11，逐行与 r21 相同。
  - W12 的 23 项检查中 22 项通过，只有 `logout` 未测。
  - 证据 SHA-256 `7408b06c0561ea53542c01cdbdaa3f8bec77f55178d9a987b1dccb55cb0b85ff`，保存在 `dist/evidence/mac-vm-2026-11-12/mac-gui-r22/`。
  - 已写入账本。
  - W07、W08、W09、W11、K02、E01 依 r22 恢复为 `AUTOMATED PASS`。
- **`mac-resources-r8`：** 每种视图 5 次，全部有效。
  - Source、Preview、Split 的内存中位为 47.0、50.0、50.2 MiB，空闲 CPU 中位为 0.0007%、0.0009%、0.0007%，与 r7 持平。
  - 证据 SHA-256 `fb534d0fb01109a3029474dd56998aed965043f50fb58e3474eaad49abf73da1`，保存在 `dist/evidence/mac-vm-2026-11-13/mac-resources-r8/`。

### W12 真实注销（`4ce8a226…`，待 USER 认可）

| 情况 | 系统日志 | 磁盘 |
| --- | --- | --- |
| 输入后立刻注销 | 16:20:47.094 发请求；3 ms 后开始处理，+130 ms 回复 `YES`；launchd 记为 `(0, 0, 0)` | 注销请求前是旧字节，之后是输入的文本 |
| 导出对话框打开时注销 | 16:22:46.390 发请求；7 ms 后回 `NSTerminateCancel`，注销照常继续；约 0.24 s 时 launchd 记为 `(0, 0, 0)` | 两行都已落盘；没有生成导出文件 |
| 保存失败时注销 | 16:24:34.853 发请求；`NSTerminateLater`，8 ms 后回 `NO`；16:24:35.993 launchd 移除服务（推断发 SIGTERM，被忽略）；16:24:37.001 记为 `(2, 9, 9)` | 仍是此前保存的内容；`.trash` 为空，没有残留的临时文件 |

三种情况与 `a9f4354b…`、`4e19774d…` 上的结果一致。

**证据：** `dist/evidence/mac-vm-2026-11-13/w12-logout-4ce8/`。`w12-case.sh`、`main.rs`、`w12keys` 的 SHA-256 与前两次相同；三个系统日志的 SHA-256：

- text：`189a5d27acd6f3d3ae6255a5ec7643e0dcce67af248f32a2b58161e566db7efe`
- dialog：`b6c713b129336e3a092914823fc1bdd050ce9b411565159da02c2b2314218ed5`
- fault：`2fec963783e42c237dfda15347d45befd08ed77715cd0ae20b7c55b22f02fc31`

**收据状态：** P16-W12 在新身份下仍为 `NOT TESTED`，等 USER 认可这次观察后再登记收据（`mac-ledger receipt`）并更新验收表。

**USER 认可（2026-11-13）：** USER 看过上面三种情况后回复“记为通过”。已用 `mac-ledger receipt --app dist/macos/StickyMD.app --rows W12 --quote "记为通过"` 登记到 `4ce8a226…`，P16-W12 改为 `MANUAL PASS`，此前的收据保留为“此前”。与前两份一样，这是 USER 认可的脚本化观察，不是 USER 亲手操作。

## 人工收据：macOS 桌面人工项（`4ce8a226…`，2026-11-13）

USER 在本机桌面运行 `/Users/develata/gitclone/StickyMD/dist/macos/StickyMD.app`，即 `74b3d74` 的 release，可执行文件 SHA-256 `4ce8a2268b9bc26ae5a87b489c74cb7b68fa020f09b270730a4a931706affd50`，签名校验通过。本机系统为 macOS 27.2.1（26B7131）。

验完 `mac-ledger plan` 列出的人工行后，USER 回复：“macos pass，没什么问题”。

- **覆盖的行：** W01–W06、W10、I01–I03、F01，行定义见 `docs/acceptance-cases/phase-16.md` 第 38–43、47、62–64、70 行。
- **不在其中：** R03 需要签名身份，仍为 `NOT TESTED`。
- **登记：** 已用 `mac-ledger receipt --app dist/macos/StickyMD.app --rows W01,W02,W03,W04,W05,W06,W10,I01,I02,I03,F01 --quote "macos pass，没什么问题"` 登记。
- **验收表：** 这些行改为 `MANUAL PASS`，此前的收据保留为“此前”。
- **账本复核：** 登记后 `mac-ledger plan` 仍列出 I01–I03、R03。这符合账本规则：`tools/stickymd-smoke/src/mac_ledger/decide.rs` 的 `NEVER_REUSED` 规定，这几行的每次确认只代表那一次操作，以后不复用，与 Windows 相同。本次收据对 `4ce8a226…` 有效；下次 USER 确认时，这几行仍要重新做。

## R01 Hidden 与 R03（2026-11-13）

以下都在同一个程序 `4ce8a226…`（`74b3d74` 的 release）上运行，虚拟机为 macOS 27.2（26B123），scale 2。

### R03：App Translocation

做法见 [`tools/manual/macos-r03-translocation.md`](../../tools/manual/macos-r03-translocation.md)。证据在 `dist/evidence/mac-vm-2026-11-13/r03-translocation/`。

- **准备：** 把 ad-hoc 签名的包打成 zip，加上隔离标记（`0083;…;Safari;`），用“归档实用工具”解压；解出的包也带上了同一隔离标记。虚拟机的 Gatekeeper 评估是关闭的。
- **启动：** 不移动包，直接用 `open` 从下载目录启动。进程路径为 `/private/var/folders/…/AppTranslocation/94D4445C-…/d/StickyMD.app/Contents/MacOS/StickyMD`，系统挂载了一个只读的 nullfs。
- **提示：** 约 5 秒后（期间系统在做 AMFI 与 TCC 检查），stderr 写出“当前目录不可写，请先用访达把 StickyMD.app 移到别处。”与“cannot create a file: Read-only file system (os error 30)”。同时屏幕上出现 StickyMD 拥有的提示窗口（模态面板层级，410×439）。
- **关掉提示：** 从 SSH 启动时 StickyMD 不会自动成为前台程序，按回车不起作用。再 `open` 一次同一个包后，系统激活了已在运行的实例，没有起第二个进程；按回车后进程退出，launchd 记为 `(0, 0, 256)`，即退出码 1。
- **磁盘：** 下载目录前后一致，没有任何写入，也没有 `note/`。
- **限制：** 只观察了 Gatekeeper 评估关闭的情况。
  - 打开时 ad-hoc 包会先被拦下，要选“仍要打开”；之后的行为未验证。
  - 从访达双击启动时，提示框是否自动到前台，也未观察。
- **证据 SHA-256：**
  - `r03-case.sh`：`a1aa1e215da7de1ec36ede23ae78d987fe93bee032728e2a89c1b2f5ffacb9a2`
  - `stderr.log`：`8ea92cfcaff8e90ec3a30785ccd22eed9bcd78169abca2554de9b94984cae0c3`
  - `system-show-final.log`：`2a91356192944a10ddb5f2965463b4b9173306eddf4046c21cd03142f38dd3f4`
- **状态：** P16-R03 是人工行，这次观察要由 USER 认可，才能作为收据。在那之前保持 `NOT TESTED`。

### R01：Hidden 内存

`mac-resources` 现在会测 Hidden（`c7df8f6`，审查后修正于 `360d140`）：

- 以 Source 打开；
- 经辅助功能选择菜单栏的“隐藏”，等窗口离开屏幕；
- 从那时起预热 30 秒，再像可见场景一样采样；
- 两次采样时都复查窗口仍在屏幕外。

| 运行 | 驱动 | Source | Preview | Split | Hidden |
| --- | --- | --- | --- | --- | --- |
| r9 | `01bd8c6c…`（`c7df8f6`） | 47.0 / 47.1 | 50.4 / 50.4 | 50.5 / 53.3 | 53.0 / 53.1 |
| r10 | `6e18c9fa…`（`360d140`） | 47.0 / 47.2 | 50.1 / 50.5 | 52.8 / 53.0 | 55.4 / 55.7 |

单位 MiB，中位 / 最大，每种 5 次，全部有效。

- **证据：** r9 SHA-256 `f38a3eaff689bdfd77f8eb6ee560a4655e3000cd3762dece6d180f4bee9fe3b7`，r10 SHA-256 `3cb02e577317385e000007b61ae284efbf3f1dc9954dd5e12908bbbd3573e8f9`，都保存在 `dist/evidence/mac-vm-2026-11-13/`。
- **对照探针：** 用一个未提交的工具构建，把“隐藏”换成两次“置顶”：同样经辅助功能打开菜单，但窗口一直可见。Source 升到 53.3 / 53.5 MiB。证据在 `probe-menu-only/`，SHA-256 `87dde39047eac378cad4face2f49f571bbad1b28bec8f10d435562c3aebe53aa`。

**结论：**

- Hidden 比 `10` 的临时硬门 48 MiB 高 5–8 MiB。
- 增量主要来自打开菜单：AppKit 的菜单与辅助功能对象留在进程里。隐藏窗口本身不释放内存。
- 用鼠标打开菜单时会少多少，没有分开测量：虚拟机里新构建的辅助程序没有发事件的权限。

`10` 规定，临时硬门在首次实测后由 USER 定最终值。在 USER 决定之前，P16-R01 记为 `BLOCKED`。

### 同一轮的 `mac-gui`

r23（驱动 `81110f45…`）与 r24（驱动 `6e18c9fa…`）都是 PASS 9、FAIL 0、NOT_TESTED 11，逐行与 r22 相同。两次都已写入账本。证据 SHA-256：

- r23：`ffca2b10e784e7305328886bdcd23dfbb48f0f9fe341bd1e205164bb1db329b4`
- r24：`96f40413d6776ae8dec2dc14ec45b3bfe8bd650861549c0300934222510c9554`

验收表的六个自动化行改引 r24。

**R03 收据（2026-11-13）：** USER 看过上面 R03 的观察后回复“记为通过”。已用 `mac-ledger receipt --app dist/macos/StickyMD.app --rows R03 --quote "记为通过"` 登记到 `4ce8a226…`，P16-R03 改为 `MANUAL PASS`，限 Gatekeeper 评估关闭的条件。R03 的确认每次只代表那一次（`NEVER_REUSED`），下次仍要重新确认。
