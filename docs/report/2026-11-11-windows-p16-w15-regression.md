# Windows 桌面回归 P16-W15（2026-11-11）

验证 `feat/macos-host` 对共享层的改动没有改变 Windows 产品行为（只允许已批准的 S7、S8、S9）。本报告只记录这台机器上的事实。它不是资格化 suite，不产生 exact-candidate 证据；被测 exe 是 Local Preflight Build。

## 环境

| 项 | 值 |
| --- | --- |
| 源码 | `feat/macos-host` @ `0653d2e969263508a77cb2f99d7d941b55c11bed`，已跟踪文件无改动 |
| 系统 | Windows 11 家庭版 中文版 25H2，10.0.26200 |
| 显示 | 单显示器 2560×1440，100%（96 DPI），工作区 `y = 0`（任务栏在底部） |
| 被测 exe | `cargo build --locked --release -p stickymd-win`，`target\release\stickymd-win.exe` SHA-256 `d2e4f32136764e70fb938048df7f43b5799ba18c97bbe629bdaa5c5c12142bd2` |
| 桌面驱动 | `stickymd-smoke` 的本地 preflight（`#[ignore]` 测试，`STICKYMD_PREFLIGHT_EXE` 指向上面的 exe），在 `0653d2e` 的干净 worktree 中编译运行；harness 测试二进制 SHA-256 `43a7d4a371e3bd06c733500803363dca0e0ea9aa3898b3e8e7808c6654a0460e`。每项把 exe 复制进 `%TEMP%\stickymd-c2d-preflight-*` 的新目录后运行，不碰 USER 的便签目录 |
| 运行方式 | 前台工具调用，独占桌面；USER 运行前关闭了自己的 StickyMD，运行期间未碰鼠标键盘 |

## 自动化

在仓库根目录（PowerShell）：

| 命令 | 退出码 | 末行 |
| --- | --- | --- |
| `tools\smoke\phase-00.ps1` | 0 | `StickyMD smoke PASS: phase-00` |
| `tools\smoke\phase-07.ps1` | 0 | `StickyMD smoke PASS: phase-07` |
| `tools\smoke\phase-08.ps1` | 0 | `StickyMD smoke PASS: phase-08` |
| `tools\smoke\phase-15.ps1` | 0 | `StickyMD smoke PASS: phase-15` |

它们只说明测试仍然通过，不代替桌面回归。

## 桌面 preflight

| 测试 | 结果 | 通过的检查 |
| --- | --- | --- |
| `qualification::g4::preflight::c2d_tray_local_preflight`（G4-01） | ok，159 s，输入清理 `None` | 20/20：`tray-icon-present`、`tray-menu-items`、`tray-hide-*`、`tray-show-*`（含取得焦点）、`tray-topmost-controls-sync`、`tray-topmost-sensor-keeps-config`、`tray-quit-exits`、`tray-quit-saves-latest`、`tray-quit-save-failure-{title,retains-text,keeps-visible-process,retry}`、`close-hides-window`、`close-keeps-process`、`alt-f4-hides-without-exit`、`sole-process` |
| `qualification::g3::export_local_preflight`（G3-02） | ok，7 s | 4/4：`export-publishes-file`、`export-copies-asset-once`、`export-rewrites-local-asset-only`、`export-leaves-working-note` |
| `qualification::g4::preflight::c2d_dock_local_preflight`（G4-02） | ok，159 s，输入清理 `None` | 53/53：左、右、上（Pin OFF）与右（Pin ON）的 snap、reveal 100 ms、leave 500 ms、collapse、focus guard、reexpand；`covered-sensor-real-hover`；`manual-collapse-button`；Esc 各项；透明度弹窗滑块、数字 Enter/失焦提交与 Esc 取消（写入 `opacity`）；`pin-off-config`（写入 `always_on_top = false`）、`pin-on-reaches-hwnd`、`pin-survives-autohide`；Preview/Split 滚动各项 |
| `qualification::g4::preflight::c2d_search_local_preflight`（G4-03） | ok，22 s，输入清理 `None` | 15/15：`traditional-undo`、`traditional-redo`、`ctrl-insert-copies-selection`、`shift-insert-pastes`、`shift-delete-cuts`、`shift-insert-{dib,file-drop}-publishes-asset`、`preview-ignores-traditional-edits`、Search 的打开、替换、撤销、Esc 返回、视图快捷键保持会话 |

## 逐项结论

| 检查项 | 结论 | 依据 |
| --- | --- | --- |
| S2 键表 | 已覆盖部分通过 | G4-03：撤销/重做、传统剪贴板键、查找/替换入口与视图快捷键行为符合契约。行首行尾、文首文末未在桌面单独测：USER 豁免 |
| S4 退出屏障 | 通过 | G4-01：干净时托盘退出直接退出；有未保存内容时先保存最新文本；保存失败时取消退出，窗口保持可见、文本保留、标题提示，重试后成功退出 |
| S6 顶边感应条 | 通过（本机不能区分） | G4-02 的上边 snap、reveal、leave、collapse、reexpand 全部通过。本机任务栏在底部，`work.y` 等于显示器顶边 0，无法区分“按显示器顶边”与“按 `work.y`”；任务栏在顶部的布局未测 |
| S7 手动收起期间恢复焦点 | USER 豁免 | 切换桌面类场景未测 |
| S9 透明度弹窗 DPI 缩放 | 100% 通过；>100% USER 豁免 | G4-02 在 100% 下完成弹窗的拖动、输入与提交。“与改动前逐像素相同”未由本次比对，依据是像素钉住测试。125%、150%、200% 未测 |
| 导出（含图片） | 通过 | G3-02：受管图片复制一次、只改写本地资产引用、工作笔记不变。导出失败的提示文字未测：USER 豁免 |
| 配置保存 | 写入通过；其余 USER 豁免 | G4-02 观察到 `opacity` 与 `always_on_top` 写入 `note\config.toml`；G4-01 的 `tray-topmost-sensor-keeps-config` 通过。重启后保持、解析失败改名、改名失败保留原文件未在桌面测 |
| 预览图片 | USER 豁免 | 本地图片显示、网络图片占位、64 MiB 拒绝未在桌面测 |
| 托盘菜单 | 通过（隐藏态文字未单独看） | G4-01：图标存在，可见窗口时菜单恰为三项（显示/隐藏、置顶、退出）且顺序正确；显示/隐藏、置顶切换、退出走 S4 屏障均通过。窗口隐藏时“显示”文字未在本次单独确认 |

## USER 豁免

USER 原话（2026-11-11）：“这种很麻烦的测试，并且对我自己使用没有什么大的意义的项目，你可以注明懒得测，以后碰到问题再测，毕竟这种情况遇到得比较少，这个repo目前也没什么别人要用”。

USER 选择 S9 只验 100%，高 DPI 记未测；按上句一并归入豁免。

豁免项（懒得测，遇到问题再测）：S7；S9 的 125%、150%、200%；S2 的行首行尾、文首文末桌面验证；导出失败提示文字；配置重启后保持、解析失败改名、改名失败保留；预览图片三项；托盘隐藏态文字；任务栏在顶部时的 S6。

## 未做

- G4-05（身份与第二实例）、G3-04（用户资产安全长流程）不在 W15 检查项内，未运行。
- 没有修改产品代码；没有发现回归。
- 本地 preflight 不写 `dist/evidence`，不是 exact-candidate 证据。
