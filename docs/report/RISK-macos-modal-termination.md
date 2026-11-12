# RISK：导出对话框打开时，系统的退出请求没有生效（macOS，2026-11-11）

## 结论

`09` 的 macOS 映射规定：系统终止请求到达时，如果有模态窗口，就 `abortModal`（导出按取消处理）并回 `NSTerminateCancel`，退出屏障在对话框退栈后运行。在 macOS 27.2 上，这一支走不到。

- **现象（已核实）**：导出对话框（`NSSavePanel.runModal`）打开时，系统的退出请求不调用 `applicationShouldTerminate:`，对话框也不关闭。驱动手动取消对话框、并确认它已关闭之后，程序在 10 s 观察窗口内没有退出。
- **原因（已核实）**：`NSSavePanel` 的 `preventsApplicationTerminationWhenModal` 默认为真。AppKit 文档说明，这样的模态窗口会阻止程序终止，并且不询问 delegate。
- **修复候选（已在临时构建上验证）**：在对话框弹出前把这个属性设为假。之后，退出请求带着模态窗口到达现有的处理分支：对话框结束，没有导出，程序退出；请求前磁盘上还是旧字节，退出后已是新文本。

只影响 macOS 宿主，Windows 没有这条路径。

## 证据

环境：Tart 虚拟机，macOS 27.2。被测 release 是 `10127b2`，可执行文件 SHA-256 为 `b13088953a90e4df8ed57f5835f5ddd14641c6774e0bc096035df94e98060b56`。

除 `mac-gui-r12` 位于 `dist/evidence/mac-vm-2026-11-11/mac-gui-r12/` 外，其余证据都在 `dist/evidence/mac-vm-2026-11-11/w12-modal/` 下。驱动的完整 SHA-256 见各目录的 `driver.sha256`；早期运行没有这个文件，下表列出的是当时记录的驱动哈希前缀。

| 运行 | 被测程序 | 驱动 | 内容与结果 | `mac-gui.json` 的 SHA-256 |
| --- | --- | --- | --- | --- |
| `mac-gui-r12` | release | `37b49e5a8e92…` | 对话框已对准会话中的 `export.md`。请求后 3 s 内 AX 一直读到对话框开着且有应答，主线程没有卡住。手动取消后再次读到对话框已关闭，从那时起 10 s 内进程仍在运行；`export.md` 不存在 | `39b85576c54032c10dd982a523b605b51acf31d0c80a0270642cecdae6b35f5d` |
| `probe` | release 加一行 `should_terminate` 日志（`probe.patch`，可执行文件 `12cfb8d2…`，全文见 `probe-executable.sha256`） | `13150c9f…` | 两次请求都返回 `true`（`mac-gui.json`），`W12-stderr.log` 只有一行 `PROBE should_terminate modal=false`，对应第一次请求（保存失败那次）。这个驱动在手动取消后没有再确认对话框已关闭 | `36862629ee81900511ae165433b6e70fc0d6c409d58271c8203eee5ccc1918e5` |
| `candidate-r2` | release 加修复候选与日志（`candidate.patch`，可执行文件 `8c92500475661cac43e969ac14d0a36d3dd713e9cc146ac6f52339df65506bd6`） | `37b49e5a8e92…` | 日志依次为三行：`PROBE should_terminate modal=false`、`PROBE save panel prevents termination when modal: true`、`PROBE should_terminate modal=true`。请求后对话框立即关闭，进程随即退出，会话中的 `export.md` 不存在；请求前磁盘为旧字节，退出后为新文本 | `c34aa95bf929ee77e5665f7dba3f2fc7c49690c5937e37841061de59944f5fc0` |
| `gui-expC` | release | 临时实验驱动（未记录哈希） | 保存失败的终止被拒后，在不开对话框的情况下再发一次终止：立即退出，退出后磁盘为新文本 | `3edff55a84b48a2217854ea613d45f1e1b157c21bae8382088c7acf32a9237a2` |

更早的 `gui-new4`、`gui-expA` 现象相同，但它们的驱动既没有在手动取消后确认对话框已关闭，也没有把对话框对准 `export.md`，只作补充。

关于这些证据，有三点要说明：

- **机制的依据**：AppKit 头文件 `NSWindow.h` 对 `preventsApplicationTerminationWhenModal` 的说明是：默认 `YES`，打开文件面板等窗口默认为 `NO`。`candidate-r2` 读到保存面板上的值为 `true`。
- **磁盘观察不等于因果**：“旧字节、新文本”只是请求前后各读一次磁盘的结果。自动保存抢在退出前写入的可能排除不了，所以这不证明是退出屏障写的。
- **构建与还原**：两个探针的源码改动都已还原，没有提交。从补丁重新构建出的候选可执行文件与第一次逐字节相同；还原后重新打包的产品可执行文件仍为 `b13088953a90…`。

## 可选路径

| 方案 | 做法 | 代价 |
| --- | --- | --- |
| A（建议） | `dialog.rs` 在 `runModal` 前把保存面板的 `preventsApplicationTerminationWhenModal` 设为 `NO`。`should_terminate` 现有的模态分支不变：`abortModal`，导出按取消处理，退出在对话框退栈后运行 | 一行宿主代码；`09` 的行为文字不变，映射表补一句机制说明。注销走的也是退出 Apple Event，但没有实测 |
| B | 宿主自己注册 `kAEQuitApplication` 处理器，绕过 AppKit 的判断 | 要复制 AppKit 的终止流程；模态期间这个 Apple Event 会不会派发给处理器，没有核实 |
| C | 接受现状，把 `09` 改写为已知限制 | 注销会被打断，而且至少在 10 s 观察窗口内程序不会自行退出，与 `09` 的意图不符 |

## 对规格与验收的影响

- **P16-W12**：记为 `BLOCKED`（自动化已知失败）。USER 的人工收据保留在原处，但它与这项证据冲突，不能单独支撑通过。
- **采用方案 A 之后**：
  - app 的 SHA-256 会变，W12 用 `mac-gui --only W12` 复验；
  - 其他行是否需要重新确认，按复用提案（待批准）由 USER 决定。
- **未覆盖**：
  - 真实注销；
  - 关机；
  - 10 s 之后的行为；
  - Automation 授权在其他环境里会不会弹窗。本虚拟机里，我们没有授予过 Automation，但授权历史没有核实；每次调用都在约 0.1 s 内返回 `true`。

## Resolution：按方案 A 修复（2026-11-12）

USER 原话：“按方案 A 修”。改动如下：

- `apps/stickymd-mac/src/platform/macos/dialog.rs` 在 `runModal` 前把保存面板的 `preventsApplicationTerminationWhenModal` 设为 `NO`。
- `09` macOS 映射“系统终止”一行补上这一机制说明。
- `tools/stickymd-smoke/src/qualification/manual_observation/coverage/sources.rs` 更新 plan 09 的整份文档摘要（受约束的各节都没有变）。

修复后的 release 可执行文件 SHA-256 为 `5cd37a465990f295e9a47fbe751c23b969bdcaaed0fbc273cd081550494bc792`。

虚拟机复验，证据在 `dist/evidence/mac-vm-2026-11-12/`：

- `mac-gui-r13`（驱动 `7848baa9…`）：W12 能自动化的检查全部通过。请求一到对话框立即关闭，程序随即退出；对准的会话中 `export.md` 没有生成；请求前磁盘上是旧字节，退出后是新文本。
- `mac-gui-r13-w12-e01`（最终驱动 `a1bc2a7f…`，`mac-gui.json` SHA-256 `771c1fe4bfd777b9cef0c5a3a8aae1973ba4c4db80c1f8773b75be912374a839`）：W12 结果同上，E01 也通过。

codex-reviewer 审查（第 5 轮）没有发现阻断项，并核实了：

- objc2 调用的选择子与类型都正确；
- 模态分支的执行顺序是 `abortModal`、投递退出、回 `NSTerminateCancel`，再在对话框退栈后运行退出屏障；
- Cmd+Q 与托盘退出不受影响；
- Windows 没有改动。

仍未覆盖：

- 真实注销与关机；
- `show_error` 用的 `NSAlert`。它的窗口是否同样阻止终止没有核实：有些信息型提示默认就不阻止终止，而且致命启动错误可能出现在终止钩子安装之前。这不在本次批准范围内，留作后续。

## 更正（2026-11-12）

“可选路径”表中方案 C 的代价写着“注销会被打断”，这不成立：StickyMD 是 UI Element 应用，注销时 loginwindow 发退出请求，但不等回复，回 `NSTerminateCancel` 拦不住注销。方案 A 一行说“注销走的也是退出 Apple Event”，这一点已在 VM 中证实，StickyMD 处理了这个 Apple Event。详见 [RISK：注销时 launchd 结束仍在运行的 StickyMD](RISK-macos-logout-ui-element.md)。
