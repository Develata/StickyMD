# 宿主共享执行：去重与三宿主形态

日期：2026-11-10。基线：`feat/macos-host` `a0a3a11`。状态：**等待 USER 批准**；批准前不改 Windows 宿主、不建新 crate。

## 结论

1. **重复真实存在，但比行数小。** macOS 宿主 7,093 行里约 1,800 行与 Windows 宿主文本近同。按生产代码（去掉测试）算，真正可共享的纯流程约 600 行：导出流程、配置发布编排、本地图片读取、粘贴适配前缀、托盘菜单翻译。其余“近同”多是测试，或是安全保证不同、不能合并的原生实现。
2. **不建通用文件系统接口。** `03` 规则 3、5 禁止逐文件操作的虚接口，也禁止共享层先查路径、再由平台凭字符串删除。共享的单位是**完整的能力事务**（暂存、发布、放弃），由各宿主实现。
3. **建议分两步。** 第一步（现在）：把纯流程与共享契约测试移入 `stickymd-app`，不建新 crate。这要改 Windows 宿主的结构，但不改行为。第二步（开始 Debian 时）：把 macOS 宿主里与 POSIX 通用的原生部分抽成 `crates/stickymd-posix`，由 macOS 与 Debian 共用。第二步是骨架级，届时另行批准。
4. **现在就把 POSIX 通用部分和 macOS 专有部分在文件内分开**，不移动、不建 crate，让第二步变成搬家而不是重写。

## 数据

文本相似度：按行匹配，`2×匹配行 / 两文件总行数`，含注释与测试（Codex 第一轮测量，已用 `diff` 抽查）。

| 文件 | macOS / Windows 行数 | 相似度 | 可共享的生产代码 |
| --- | --- | --- | --- |
| `export/mod.rs` | 752 / 736 | 95% | 368 / 352 行，93% |
| `assets/transaction.rs` | 336 / 335 | 98% | 粘贴适配前缀 64 行，100% |
| `config/storage.rs` | 318 / 302 | 96% | 92 / 76 行，87% |
| `preview/image_source.rs` | 123 / 123 | 98% | 65 行，99%（B1 修复后 macOS 已改为跟随目录） |
| `platform/*/tray.rs` | 251 / 251 | 98% | 菜单标签、命令翻译、图标 |
| `persistence/storage.rs` | 753 / 869 | 11% | 不共享：两边安全保证不同 |
| `assets/storage.rs` | 535 / 669 | 28% | 不共享：同上 |
| `atomic_file.rs` | 163 / 646 | 12% | 不共享：同上 |

同一个修复要维护两份（Debian 加入后三份）。第一轮审查的 B2 正说明了风险：两份看起来相同的流程，掩盖了不同的安全保证（Windows 按句柄删除，macOS 只能先比对再按名删除）。

## 方案

| 方案 | 做法 | 代价 | 对 Debian |
| --- | --- | --- | --- |
| A 维持现状 | 每个宿主各自一份 | 修复和测试做两遍、三遍；容易漂移 | 第三份拷贝 |
| **B 纯流程进 `stickymd-app`**（建议的第一步） | 导出的解析、筛选、命名、改写与发布顺序，配置的文本与发布编排，图片读取，粘贴前缀，托盘翻译，移到 `stickymd-app` 的 `flow`/`execution`。宿主只实现完整事务：导出的暂存、发布、放弃，配置的原子发布，图片的有界读取。共享契约测试对每个宿主各跑一遍 | 改 Windows 宿主结构，需要 Windows 桌面回归；约 1–2 周 | Debian 只写事务实现 |
| C 新 `stickymd-storage` crate | B 的内容放进新 crate | 新的依赖边界，没有 B 之外的收益 | 同 B |
| **D 新 `stickymd-posix` crate**（建议的第二步） | macOS 与 Debian 共用的原生实现：目录句柄、`*at` 调用、临时文件、单实例锁、socket 唤醒 | 新 crate 里有 `unsafe`，需要批准它作为平台 adapter 目录 | 直接复用 |

B 不建立新的 Capability Port 或公共事务接口：事务以宿主内的类型实现现有 Port，共享层只调用纯函数和测试辅助。如果实施中发现必须新增公共 Port，停下另报。

## Debian 适配（第二步的依据）

macOS 与 Linux 的差异集中在少数调用，适合同一个 POSIX 实现加条件编译：

| 能力 | macOS | Linux |
| --- | --- | --- |
| 不覆盖地改名 | `renameatx_np(RENAME_EXCL)` | `renameat2(RENAME_NOREPLACE)` |
| 持久化 | `F_FULLFSYNC` | `fsync` / `fdatasync` |
| 目录当前位置 | `fcntl(F_GETPATH)` | `readlink("/proc/self/fd/N")` |
| 拒绝路径中任何链接 | `O_NOFOLLOW_ANY` | `openat2(RESOLVE_NO_SYMLINKS)` |
| 目录监视 | FSEvents（`notify`） | inotify（`notify`） |
| 单实例与唤醒 | 目录 fd `flock` + Unix socket | 相同 |
| 用户临时目录 | `confstr(_CS_DARWIN_USER_TEMP_DIR)` | `XDG_RUNTIME_DIR` |

窗口、菜单栏或托盘、剪贴板、输入法不属于这里，各宿主各自实现。Debian 的托盘与贴边行为仍要先有 Debian 平台决定报告。

## 对 Windows 的影响

第一步只移动代码，不改 Windows 的行为：
- 共享契约测试必须在 Windows 上跑过。
- 移动前后 Windows 宿主现有的测试断言不变。
- Windows 桌面回归加入 P16-W15：导出（含图片）、配置保存、预览图片、托盘菜单。

## 待批准

1. 第一步：纯流程与契约测试移入 `stickymd-app`。
2. 现在就在 macOS 宿主内把 POSIX 通用部分与 macOS 专有部分分开（不移动文件）。
3. 第二步（`stickymd-posix`）原则同意，开始 Debian 时再单独批准。

## Resolution：批准、执行与验收（2026-11-11）

USER 2026-11-11 批准两步一次完成，不再按上文第 3 项等到开始 Debian 时单独批准；`stickymd-posix` 只放 macOS 实现，Linux 分支留到 Debian。执行设计、提交与出入记录在 [宿主共享任务](../tasks/phase-16-host-sharing.md)。

codex-reviewer 第 8 轮审查 `ad3f6c3..8f1a514`：

| 发现 | 核实 | 处理 |
| --- | --- | --- |
| BLOCKER：托盘一步把 Windows 的 `phase8_show_hide_label_describes_the_action`、`phase8_tray_icon_is_fully_owned_rgba` 移进共享层并去掉前缀，Phase 8 按 `phase8_` 过滤，两项不再运行 | 属实，违反“测试名不变” | `10127b2`：两宿主按原名恢复，函数体调用共享场景；Windows 135 个测试名与 `ad3f6c3` 逐一相同 |
| NEEDS_VERIFICATION：最终二进制的桌面回归未做 | 属实 | macOS：虚拟机用 `10127b2` 的 release 复跑 `mac-resources` 与 `mac-gui`，全部可运行场景通过，见 [macOS 虚拟机验收](2026-11-10-macos-vm-acceptance.md) 末节。Windows：P16-W15 仍 `NOT TESTED` |

第 9 轮审查 `10127b2`：PASS，无阻断。复核了测试身份、Phase 8 的选择、断言、`testing` 特性不进入产品构建。它要求的执行证据：CI 运行 `42970125576`（`10127b2`）全部通过，其中 “Windows x64 headless (all, tests)” 日志显示 `platform::windows::tray::tests::phase8_show_hide_label_describes_the_action ... ok` 与 `phase8_tray_icon_is_fully_owned_rgba ... ok`；Windows 格式与 lint、macOS 测试与 lint 同在该运行中通过。

未实证：Windows 桌面上的托盘、导出、配置保存与预览图片行为，待 P16-W15 在 Windows 机器上运行。
