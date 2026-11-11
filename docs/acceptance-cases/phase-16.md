# Phase 16 Acceptance Matrix

本矩阵验证 macOS 宿主（[`11` Phase 16 节](../plan/11_testing_and_release.md#phase-16-macos-host)，[`02` macOS 节](../plan/02_positioning_and_scope.md#macos-host)）。它不是资格化 suite，不产生 exact-candidate 证据。未运行的项保持 `NOT TESTED`。

原生 POSIX 层位于 `crates/stickymd-posix`；`phase 16` 同时运行它与 macOS 宿主的测试。登记、原生边界与 CI 投影由 `stickymd-smoke` 测试和 `phase 00` 检查。

## M0 基线

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P16-B01 | `stickymd-core`、`stickymd-render`、`stickymd-app`、`stickymd-desktop` 的测试在 macOS 全部通过 | Automated | `cargo test --locked`（macOS 27.2，`baseline` 全部通过） | AUTOMATED PASS |
| P16-B02 | 没有 `head` 表的字体不进入 macOS 字体库，Windows 不受影响 | Automated | `system_fonts::tests`（macOS 下运行） | AUTOMATED PASS |

## M1 存储与单实例

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P16-S01 | 真实存储通过 `storage_conformance!` 全部行 | Automated | `persistence::storage_conformance`（5 行全部通过） | AUTOMATED PASS |
| P16-S02 | 已存在目标：guard 不符返回 Conflict，目标不变；`ForceOverwrite` 跳过 guard | Automated | `persistence::storage::tests` | AUTOMATED PASS |
| P16-S03 | 首次创建：目标突然出现时 Conflict，不覆盖 | Automated | `persistence::storage::tests` | AUTOMATED PASS |
| P16-S04 | 目录同步失败报保存失败，保持 dirty；重试不与自己冲突 | Automated | `persistence::storage::tests` | AUTOMATED PASS |
| P16-S05 | 保留 canonical：内容未变改名；内容已变移回并报 `Changed`；移回失败报失败，文件不丢 | Automated | `persistence::storage::tests` | AUTOMATED PASS |
| P16-S06 | 卷不支持 `RENAME_EXCL` 时提示后退出，不回退 | Automated | exFAT 映像 | AUTOMATED PASS |
| P16-S07 | Program Directory：bundle 内取 `.app` 所在目录，开发构建取可执行文件父目录 | Automated | `paths::tests` | AUTOMATED PASS |
| P16-S08 | 同目录第二实例取锁失败，在任何 durable 文件创建前退出 | Automated | `stickymd-posix::instance::tests` | AUTOMATED PASS |
| P16-S09 | 第二实例经 socket 唤醒第一实例；socket 路径不超过 104 字节 | Automated | `stickymd-posix::instance::tests` | AUTOMATED PASS |
| P16-S10 | 受管图片：未引用不移动不删除，`sync` 报 deferred | Automated | `assets::storage::tests` | AUTOMATED PASS |
| P16-S11 | 监视：外部修改产生 hint；`note/` 被移走转 watcher failure | Automated | `platform::macos::file_watch::tests` | AUTOMATED PASS |
| P16-S12 | 新建笔记、没有换行或两种换行一样多的已有笔记用 LF；放弃恢复后重建的缺失笔记也用 LF（`05` 2026-11-10 修订） | Automated | `startup::bootstrap::tests`；`stickymd-core` 的 `line_ending`、`persistence`、`document` 测试；`stickymd-app` 的 `flow::recovery::tests` | AUTOMATED PASS |
| P16-S13 | 临时文件另有名称（硬链接）时拒绝，正文不被清空 | Automated | `stickymd-posix::file::tests` | AUTOMATED PASS |
| P16-S14 | 运行中 `note/` 被换成另一个目录：资产与配置仍落在原目录，不写入新目录；目录被删除重建后两者保持失败，不写入重建的目录 | Automated | `persistence::adapter::tests` | AUTOMATED PASS |
| P16-S15 | 资产临时名已被占用时不删除；失败只清理本次创建的文件 | Automated | `assets::storage::tests` | AUTOMATED PASS |

## M2 窗口

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P16-W01 | 无 Dock 图标、不在 Cmd+Tab | Manual | 人工。VM 外部观察：`UIElement`、Dock 无此项；Cmd+Tab 画面未查（[2026-11-10 VM 验收](../report/2026-11-10-macos-vm-acceptance.md)） | NOT TESTED |
| P16-W02 | 左、右、上贴边收起留 2 DIP，上边贴屏幕顶边 | Manual | 人工。VM 外部观察通过（[2026-11-10 VM 验收](../report/2026-11-10-macos-vm-acceptance.md)） | NOT TESTED |
| P16-W03 | 悬停 100 ms 展开，不抢焦点；提前离开取消 | Manual | 人工。VM 外部观察通过（[2026-11-10 VM 验收](../report/2026-11-10-macos-vm-acceptance.md)） | NOT TESTED |
| P16-W04 | Top 悬停展开后鼠标停在顶边不收回，离开该区域 500 ms 收回 | Manual | 人工。VM 外部观察通过（[2026-11-10 VM 验收](../report/2026-11-10-macos-vm-acceptance.md)） | NOT TESTED |
| P16-W05 | 切换桌面、全屏桌面：位置不变、可见，重绘正常 | Manual | 人工 | NOT TESTED |
| P16-W06 | 手动收起后切走再切回，细条仍在最上层、能悬停展开（S7） | Manual | 人工 | NOT TESTED |
| P16-W07 | 隐藏到托盘后再次打开 `.app`，窗口显示并取得焦点 | Manual | 人工。VM 外部观察通过（[2026-11-10 VM 验收](../report/2026-11-10-macos-vm-acceptance.md)） | NOT TESTED |
| P16-W08 | 托盘：显示/隐藏、置顶、退出 | Manual | 人工。VM 外部观察通过（[2026-11-10 VM 验收](../report/2026-11-10-macos-vm-acceptance.md)） | NOT TESTED |
| P16-W09 | 透明度 40–100 生效 | Manual | 人工。VM 外部观察通过（[2026-11-10 VM 验收](../report/2026-11-10-macos-vm-acceptance.md)） | NOT TESTED |
| P16-W10 | 光标闪烁不产生帧；反色可见 | Manual | receipt: [2026-11-10 VM 验收](../report/2026-11-10-macos-vm-acceptance.md) 的 USER 确认（2026-11-11，`0fa8754` 的 release，本机桌面）：光标可见、闪烁正常。不产生帧由同一二进制在 VM 中的唤醒测量佐证（约每 30 s 一次） | MANUAL PASS |
| P16-W11 | Cmd+Q：干净时退出；保存失败时取消 | Manual | 人工。VM 外部观察：干净时退出通过；保存失败时取消未测（[2026-11-10 VM 验收](../report/2026-11-10-macos-vm-acceptance.md)） | NOT TESTED |
| P16-W12 | 系统终止：`osascript` 退出、注销、导出对话框打开时退出 | Manual | 人工 | NOT TESTED |
| P16-W13 | 感应条 2 DIP（S8）的几何测试 | Automated | `stickymd-app` 单元测试 | AUTOMATED PASS |
| P16-W14 | S7 的 reducer 测试：手动收起后恢复焦点不撤销，进入展开路径后撤销 | Automated | `stickymd-app` 单元测试 | AUTOMATED PASS |
| P16-W15 | Windows 桌面回归（S2、S4、S6、S7；S9 透明度弹窗内容随 DPI 缩放，只影响 >100% DPI；宿主共享第一步移动的导出（含图片）、配置保存、预览图片、托盘菜单） | — | `0653d2e` 的 release（100% DPI）在 Windows 11 25H2 上跑本地桌面 preflight G4-01、G4-02、G4-03、G3-02，全部通过：S2 的撤销/重做与剪贴板键、S4、S6（本机 `work.y` 等于显示器顶边）、S9 的 100%、导出含图片、配置写入、托盘菜单。其余（S7、S9 高 DPI、配置失败路径、预览图片等）USER 2026-11-11 决定不测，遇到问题再测（[Windows 回归](../report/2026-11-11-windows-p16-w15-regression.md)） | USER 豁免 |
| P16-W16 | Windows 上的 2 DIP（S8） | — | USER 2026-11-10 决定不单独回归 | USER 豁免 |
| P16-W17 | 光标图层自带闪烁时 shell 不按闪烁周期唤醒；图层失败后改由 shell 与帧闪烁；没有自带闪烁的宿主（Windows）照旧 | Automated | `shell::phase15_host_tests::phase16_an_overlay_that_blinks_itself_needs_no_blink_wake`、`phase16_the_event_loop_waits_for_blinks_only_when_the_shell_blinks`（事件循环实际等待的时刻与是否翻转） | AUTOMATED PASS |

## M3 输入、剪贴板、导出

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P16-K01 | Cmd 键表：保存、导出、剪贴板、撤销重做、查找替换、视图缩放、行首行尾 | Automated | 键表单元测试 | AUTOMATED PASS |
| P16-K02 | Cmd 键在真实窗口生效 | Manual | 人工。VM 外部观察通过（[2026-11-10 VM 验收](../report/2026-11-10-macos-vm-acceptance.md)） | NOT TESTED |
| P16-I01 | 系统简体拼音：`07` 的 13 项 | Manual | 人工 | NOT TESTED |
| P16-I02 | 微信输入法：`07` 的 13 项 | Manual | 人工 | NOT TESTED |
| P16-I03 | 长按字母：不弹重音菜单，按系统设置连发，没有替换残留的多余字符 | Manual | 人工 | NOT TESTED |
| P16-C01 | 文件 URL + PNG + 文本：读文件 | Automated | pasteboard 测试 | AUTOMATED PASS |
| P16-C02 | PNG + TIFF + 文本：读原编码 PNG | Automated | pasteboard 测试 | AUTOMATED PASS |
| P16-C03 | 只有 TIFF + 文本：转 PNG，像素正确 | Automated | pasteboard 测试 | AUTOMATED PASS |
| P16-C04 | TIFF 超过长度或尺寸限额时在解码前拒绝 | Automated | pasteboard 测试 | AUTOMATED PASS |
| P16-E01 | 保存面板：取消、确定、导出内容正确 | Manual | 人工。VM 外部观察通过（[2026-11-10 VM 验收](../report/2026-11-10-macos-vm-acceptance.md)） | NOT TESTED |
| P16-F01 | 中文显示 PingFang SC，代码 Menlo | Manual | 人工。VM 字体日志为 PingFang SC 与 Times New Roman；日志不报代码字体（[2026-11-10 VM 验收](../report/2026-11-10-macos-vm-acceptance.md)） | NOT TESTED |

## M4 打包与资源

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P16-P01 | 打包脚本产出 ad-hoc 签名的 `.app`，`codesign --verify` 通过 | Automated | 脚本 | AUTOMATED PASS |
| P16-R01 | `10` macOS 节的内存基线 | Automated | `mac-resources`（VM）：Source、Preview、Split 中位 47.1、50.4、50.4 MiB，均低于临时硬门；Hidden 未测（[2026-11-10 VM 验收](../report/2026-11-10-macos-vm-acceptance.md)） | NOT TESTED |
| P16-R02 | 空闲 CPU（含光标闪烁） | Automated | `mac-resources`（VM）：按周期开关光标图层时 Split 中位 0.103%，超过 `10` 的 0.1% 硬门。闪烁改由系统合成器播放后，`0fa8754` 的 release 在 Source、Preview、Split 中位 0.001%、0.002%、0.001%，低于 0.05% 目标；同一二进制的光标可见与闪烁已由 P16-W10 确认（[2026-11-10 VM 验收](../report/2026-11-10-macos-vm-acceptance.md)） | AUTOMATED PASS |
| P16-R03 | 下载的包在 App Translocation 下提示后退出 | Manual | 需要签名身份 | NOT TESTED |

多屏与混合 DPI 按 USER 决定跳过，保持 `NOT TESTED`。
