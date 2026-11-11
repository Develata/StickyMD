# Phase 16 宿主共享：两步一次完成

## Completion State

In Progress。USER 2026-11-11 批准把 [宿主共享执行报告](../report/2026-11-10-host-shared-execution.md) 的两步一次完成；`stickymd-posix` 只放 macOS 实现，Linux 分支留到 Debian。

代码已完成（2026-11-11）：第二步 `663b996`（搬移与登记）、`1991344`（`sys` 接缝）；第一步托盘 `168b747`、图片读取 `904a83b`、配置 `6dea3be`、导出 `f86e603`。与计划的出入：
- 托盘放在 `stickymd-desktop::tray`；图片读取的两个纯函数放在 `stickymd-render::image`，与 `MAX_ENCODED_IMAGE_BYTES` 同处；配置与导出放在 `stickymd-app`；共享场景在 `stickymd-desktop::testing::contracts`。
- 导出的错误类型共享后，宿主各自的 `From<ExportError> for StorageFailure` 违反孤儿规则，转换随之移入 `stickymd-app`，两宿主原本相同。
- 粘贴未动：两宿主各有两个与 `stickymd-app` 重复的纯函数测试，删去会改变 Windows 测试名，收益小，保留。
- 每一步的 Windows 测试由 CI 运行；Windows 桌面回归加入 P16-W15。

尚待：虚拟机里用最终二进制重跑、codex-reviewer 审查全部范围、P16-W15。

## Goal

同一个修复不再在两个（以后三个）宿主里各维护一份；Windows 产品行为不变；macOS 的原生 POSIX 代码搬到 Debian 能直接复用的位置。

## Inputs

- [宿主共享执行报告](../report/2026-11-10-host-shared-execution.md)：方案 B（纯流程进共享层）与方案 D（`stickymd-posix`）。
- `03` 规则 3（不把每个文件操作包成接口）、规则 5（不能先查路径、再按字符串删除或移动）、规则 8（共享 crate 禁止 `unsafe`）。
- 两宿主五组重复文件的逐项比对（2026-11-11，结论摘在下文）。

## 约束

- Windows 宿主现有测试的断言不变，测试名不变：Windows 发布资格化按名引用 `phase7_export_release_baseline` 等基准，`phase-15` 按路径引用配置测试。
- 不新增公共 Capability Port。需要时停下另报。
- 安全保证不同的代码不合并（见“不合并”）。
- 每一步都要通过：macOS 测试与 clippy、Windows 目标的交叉 clippy（`apps/stickymd-win/build.rs` 已允许非 Windows 主机做类型检查）、CI 的 Windows 测试、`phase 00`。

## 第一步：纯流程进共享层

共享契约测试的做法：场景函数（含断言）放在 `stickymd-desktop` 的 `testing` 特性下，与 `storage_conformance!` 同处；两宿主保留原测试函数与名字，函数体改为调用场景函数并传入本宿主的构造闭包。

| 能力 | 移入共享层 | 留在宿主 |
| --- | --- | --- |
| 托盘 | 菜单 ID、`command_for_menu_id`、菜单文字（`show_hide_label`、置顶、退出、提示）、`TrayIconRgba` 与 `tray_icon()` 像素。放 `stickymd-desktop`（与标题投影同属交互外壳的呈现；`stickymd-app` 只持有 `TrayView`/`TrayCommand`） | `TrayController`、`TrayAdapterError`、派发线程（依赖 `tray-icon`） |
| 配置 | `stickymd-app::config`：`load_config_from(read, preserve)` 读取裁决、`preserve_with(path, stamp, move_no_replace)` 保留循环（移动由宿主闭包完成，移动本身是裁决，不预查）；固定字节常量与 `populated_config()` | 读文件、时钟、原子发布、`move_file_no_replace`、`save_config_in`（macOS 按目录句柄） |
| 图片读取 | `is_network_placeholder`、`read_limited`（64 MiB 有界读取） | “打开”：macOS 经持有的目录 `openat`+`O_NOFOLLOW_ANY`，Windows 经路径 `NtCreateFile` |
| 导出 | 暂存与临时文件命名、资产目录名序列、`copy_hashing`（64 KiB 复制并算 SHA-256，`stickymd-app` 增加 workspace 依赖 `sha2`）、`ExportError<P>`（宿主以类型别名代入自己的原子发布错误） | 编排 `export_snapshot` 与全部文件动作 |
| 粘贴 | 已共享（`prepare_and_store_paste`）。删去两宿主中与 `stickymd-app` 重复的纯函数测试，确认 `stickymd-app` 已有等价覆盖 | 不动 |

不合并（安全保证不同）：工作笔记判定（macOS 比对目录与文件身份，Windows 比对路径）；自有暂存文件删除（Windows 按句柄，macOS 先比身份再按名）；暂存目录与 Markdown 临时文件清理；本地图片打开；`save_config_in` 与 `save_config`；`AssetStorage::apply_effect` 与 `.trash` 策略；原子发布原语；`LocalImageSource` 持有物（句柄与路径）。

不做：导出编排整体共享（约八个文件动词进接口，违反规则 3）；图片读取整段共享（需新增 `LocalImageOpener` 公共 Port）。

顺序：托盘 → 图片读取 → 配置 → 导出，每个能力一次提交，两宿主同一提交；`git diff --color-moved` 核对搬移。

## 第二步：`crates/stickymd-posix`

- 内容：`apps/stickymd-mac/src/platform/macos/{file.rs, atomic_file.rs, instance.rs}`：目录句柄与 `*at` 调用、文件身份、原子发布、`flock` 单实例与 socket 唤醒。先原样 `git mv`（一次提交，零编辑），再加接缝（第二次提交）。
- 接缝：五处 macOS 专有调用收进 `sys/macos.rs`：`F_FULLFSYNC`、`renamex_np(RENAME_EXCL)`、`O_NOFOLLOW_ANY`、`F_GETPATH`、`confstr(_CS_DARWIN_USER_TEMP_DIR)`。其余代码只调 `sys`。Linux 对应项（`fsync`、`renameat2(RENAME_NOREPLACE)`、`openat2(RESOLVE_NO_SYMLINKS)`、`/proc/self/fd`、`XDG_RUNTIME_DIR`）只写在文档里，不写代码。非 macOS 目标上 crate 为空。
- 登记（与建 crate 同一提交）：工作区成员；`unsafe` 只许在本 crate 与 `apps/stickymd-mac/src/platform/`（治理检查）；`#![deny(unsafe_op_in_unsafe_fn)]`、`SAFETY`、`plan_ref`；CI 选择、投影与 headless 运行器；按模块复用测试的指纹归属；`AGENTS.md`、`03` 的目录与 crate 表。
- 不搬：`file_watch.rs`（`notify`）、存储适配（`persistence`、`assets`、`config` 的宿主实现）、窗口、托盘、剪贴板、输入法。

## Exit

- 两步完成，上文检查全部通过；Tart 虚拟机里用最终二进制重跑 `mac-gui` 与 `mac-resources`。
- codex-reviewer 审查 `ad3f6c3` 起的全部范围，发现逐条核实。
- P16-W15 的 Windows 桌面回归加入：导出（含图片）、配置保存、预览图片、托盘菜单。
