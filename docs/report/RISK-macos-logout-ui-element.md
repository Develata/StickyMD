# RISK：注销时 launchd 结束仍在运行的 StickyMD（2026-11-12）

## 结论

`09` 规定 macOS 的系统终止请求（注销、关机）走共享退出屏障。实测注销时，这条路径只成立一半：

- **退出请求会到，但不被等待。** loginwindow 向 StickyMD 发退出 Apple Event，退出屏障会运行。但 StickyMD 是 UI Element 应用（没有 Dock 图标，见 W01），不在普通应用的退出列表里。loginwindow 要等过了不可撤销点才给它发这个请求，而且不等回复。所以它既不能取消注销，也不能让注销等它。
- **还没退出的进程会被 SIGTERM 结束。** 用户启动的 StickyMD（Finder 或 `open`，由 launchd 托管）在退出请求发出约 1 s 后若仍在运行，launchd 会移除它的服务，进程随即被 SIGTERM 结束（证据 3）。产品不处理 SIGTERM，默认处置立即结束进程，屏障也就到此为止。

后果：

- 探针里笔记是干净的，屏障从处理退出事件到回复只用了几十毫秒（证据 3），赶在 SIGTERM 之前。有未保存文本、笔记很大或有资产事务时要多久，没有测。
- 屏障若超过约 1 s（例如笔记很大、磁盘慢，或资产粘贴事务尚未收敛），尚未落盘的编辑会丢失。原子替换保证磁盘上不会出现写了一半的文件。
- 保存失败时，回 `NO` 拦不住注销。进程被 SIGTERM 结束，用户看不到“退出已取消：保存失败”。这一点任何方案都改变不了，最多是在被强制结束前多给屏障一些时间。
- 导出对话框打开时注销照常完成（证据 1），`09` 所说的“这次注销被中断一次”不成立。这次没有观察到对话框是否按取消结束、屏障是否完成：驱动随后就被中止了。

只影响 macOS 宿主。关机未实测，推断走同一套流程。

## 证据（已核实）

环境：Tart 虚拟机（macOS 27.2，build 26B123）。被测的都是修复前的 release，产品源码从 `25a6269` 起到 `c929f23` 没有改动：

- `mac-gui` r15 用的是仓库目录里构建的可执行文件，SHA-256 `5cd37a465990f295e9a47fbe751c23b969bdcaaed0fbc273cd081550494bc792`；
- 注销探针用的是同一份源码在另一个目录构建的可执行文件，SHA-256 `a00d2148e8925283a90953a44cb4ee908db83077a04262fe6dc638a4dad892a9`。两者哈希不同，推断只是构建目录不同所致，未核对。

证据都在 `dist/evidence/mac-vm-2026-11-12/`。

1. **r15**：W12 中导出对话框打开时，从 SSH 向 loginwindow 发注销请求（`osascript`，`«event aevtrlgo»`），没有授权提示。注销完成，虚拟机停在登录窗口（自动登录只在开机时发生）。驱动被中止，`execution` 行记为 `cancelled by SIGINT/SIGTERM`，后面的检查没有运行。`mac-gui-r15/mac-gui.json` 的 SHA-256 为 `3aad175239993c739b827db2317230e28b69ce6ac7703622aac18189e74e46d8`。
2. **同一时刻 loginwindow 的日志**：`logout-probe/loginwindow-logout.log`，SHA-256 `1ae3f4729c9c37f39c155d5fb435f6e543e216cefca9557b131eea255cc0ec1c`。按行号：
   - 第 9 行收到 `kAEReallyLogOut`。
   - 普通应用的退出列表跳过 UI Element 应用：第 150 行起是 `App is UI Element, do not add it`，第 644–645 行是 `number to quit = 0`、`No Applications to quit, continue the logout sequence`。
   - 第 682 行进入 `PointOfNoReturn`，第 733 行进入 `StartUIElementsQuit`。
   - 第 947 行起：`Could not kill StickyMD, sending a quit event`，随后发出带 `kAEQuitPreserveState` 的退出 Apple Event（`appleevent sent, err = 0`）。
   - 驱动 `stickymd-smoke` 被当作私有进程处理：第 1101 行加入退出列表，第 1233–1237 行是 `not killable, attempting quit event` 和 `returned no to isRunning check so just kill the app`。驱动随后被信号中止，信号由谁发出没有查明。
3. **注销探针**（`a00d2148…`，笔记干净；探针脚本与输出保存在各自目录）：
   - `logout-probe-prefix/` 的 `logout-probe2.sh`：一个由 SSH 启动的实例。注销请求后 1 s 内以退出码 0 退出。
   - `logout-probe-prefix/` 的 `logout-probe3.sh`：SSH 启动（pid 738）与 `open` 启动（pid 743）各一个。两者都在系统日志里记有 `Handling Quit AppleEvent`，然后正常退出：
     - 743 在 18:59:04.731 处理请求，也就是 loginwindow 发出后 1 ms，约 48 ms 后回复，launchd 记为 `(0, 0, 0)`；
     - 738 在 18:59:05.104 处理请求，约 36 ms 后回复，退出码 0。
   - `logout-probe-stopped/` 的 `logout-probe4.sh`：两个实例都在注销请求前 SIGSTOP，无法回应。
     - `open` 实例（pid 760）：19:06:06.780 loginwindow 发出退出事件；19:06:07.926（1.15 s 后）launchd 移除它的服务；19:06:08.030 RunningBoard 记为 `termination reported by launchd (2, 15, 15)`，即被 SIGTERM 结束。日志没有直接记下 SIGTERM 的发出时刻，推断就在移除服务时。
     - SSH 实例（pid 757）：loginwindow 在 19:06:06.777 向它发出退出事件，之后没有被结束，日志里也没有它的服务移除记录；是否收到别的信号，探针没有记录。20 s 后探针发 SIGCONT，它在 19:06:26.901 处理了积压的退出事件，19:06:26.903 回复 `YES` 后退出。探针记下的状态 145（128 + SIGSTOP）是 shell 等待一个停止的子进程时的结果，不是退出码。
     - 两个实例都无法回应，会话照样在 1 s 内结束（`/dev/console` 的属主变为 `root`），说明 loginwindow 没有等退出事件的回复。
4. **代码**：`apps/stickymd-mac`、`crates/stickymd-posix`、`crates/stickymd-desktop` 中都没有 SIGTERM 处理。

## 可选路径

| 方案 | 做法 | 代价 |
| --- | --- | --- |
| A（建议） | macOS 宿主处理 SIGTERM：信号处理只做最小动作，由主线程把它当作系统终止请求，走同一退出屏障。SIGTERM 之后系统还会给一段宽限时间再强制结束，屏障可以用到这段时间结束为止。超时规则沿用 `09` 的“笔记尚未安全时……”条款 | 宿主新增一个信号接缝与测试；`09` 映射表的“系统终止”行要改写：写明 loginwindow 不等回复、launchd 随后发 SIGTERM，并删去“这次注销被中断一次” |
| B | 改为普通应用（Regular 激活策略），进入普通应用的退出列表。推断 loginwindow 会在不可撤销点之前请求它退出并等待回复，未测 | 会出现 Dock 图标和 Cmd+Tab 项，违反 W01 与产品定位，不建议 |
| C | 接受现状，`09` 写明注销时屏障只有约 1 s，超过的部分不保证 | 屏障慢时会丢掉未落盘的编辑 |

## 对验收的影响

- P16-W12 记为 `BLOCKED`：注销这一部分与 `09` 不符，屏障可能被 SIGTERM 截断，“这次注销被中断一次”也不成立。原因不在测试工具。
- 驱动不驱动真实注销，因为注销会结束驱动。方案 A 实施后，可以向产品进程直接发 SIGTERM 来验证保存与退出；真实注销仍用 SSH 探针另测，探针所在的 SSH 会话在注销后仍然存活。
