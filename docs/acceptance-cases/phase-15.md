# Phase 15 Acceptance Matrix

本矩阵验证共享实现迁移（[`11` Phase 15 节](../plan/11_testing_and_release.md#phase-15-shared-core-migration)）。
它不是资格化 suite：Phase 14 的 exact-candidate 与发布门不变，`stickymd-app` 的测试经 `cargo test --workspace` 进入既有
收据。基线提交为 `b846e83`（代码移动前）。未实现的里程碑与未运行的项保持 `NOT TESTED`。

## Milestone A

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P15-A01 | `stickymd-app` 不含 unsafe，不依赖 winit、softbuffer 或平台 crate，源码无平台类型与 `cfg(windows)`/`target_os` | Automated | `governance.rs` `verify_shared_application_boundary` 与其回归 | AUTOMATED PASS |
| P15-A02 | 迁移的语义测试在 `stickymd-app` 下实际执行，Linux portable 选择包含该包 | Automated | `cargo test -p stickymd-app --locked`（Windows；WSL Debian 115 passed、2 个 Release 基线 ignored）；`ci` portable 选择回归 | AUTOMATED PASS |
| P15-A03 | 按名字过滤的 `cargo test` 任务选中其测试：Windows 默认构建的实际测试清单里，任务按其模式（`--ignored`、`--lib`）至少运行所选包的一个测试，所选包之外的产品包没有它该运行的测试；各平台的近似源码核对（同样区分模式）覆盖其他平台才编译的测试；Release 或其他 profile 才编译的测试不在核对范围 | Automated | `unit_owners::verify` 的 `verify_filters`（`unit-test owner verification`，Windows 全量）；`runner::test_filters`（各平台）；`runner::filtered_tasks` 拒绝未建模的参数；G4 定向运行以通过数为 0 判失败 | AUTOMATED PASS |
| P15-A04 | 新增路径有资格化指纹规则，旧规则无命中或产品根之外的源码即失败 | Automated | `qualification::module_ledger::fingerprint` 分类与回归 | AUTOMATED PASS |
| P15-A05 | `config.toml` 字节在代码移动前后一致，load 后 save 不改变字节 | Automated | `config::storage::tests::phase15_config_bytes_are_fixed_and_survive_a_load_and_save`（基线提交与迁移后各通过一次；这不等于回退重开同一副本，后者见 P15-B13） | AUTOMATED PASS |
| P15-A06 | 无操作启动与保存不改变 note、config、图片字节 | Automated | `startup::bootstrap::tests::phase15_a_no_op_start_and_save_leave_the_data_byte_identical`（基线提交与迁移后各通过一次） | AUTOMATED PASS |
| P15-A07 | 每键全文快照次数与每次编辑提交给 worker 的后台构建次数不超过基线；提交次数以同一 fixture 在 `b846e83`（加同样的视口与计数补丁）和迁移后分别实测，见任务文件；完成回执触发的任务与 IME、Search、交错 tick 见 P15-D02、P15-D03 | Automated | `shell::hot_path_tests::phase15_a_typing_burst_stays_at_the_baseline_snapshot_and_build_counts`（经真实 Preview worker 计数） | AUTOMATED PASS |
| P15-A08 | 每帧复制字节未被 A 触及 | Automated | `surface.rs` 与 `app/presentation.rs` 均无改动（`git diff`） | AUTOMATED PASS |
| P15-A09 | Release 基线（Source 流水线、Source 查找、窗口 reducer）不低于基线提交的硬门 | Automated | `phase 15 --performance`；基线与迁移后数字见任务文件 | AUTOMATED PASS |
| P15-A10 | Windows 真实桌面回归（托盘、贴边、输入法、剪贴板）在 A 后无变化 | Manual | receipt: [2026-11-08 桌面回归](../report/2026-11-08-phase-15-desktop-regression.md)（本地桌面预检 P15-D09 与 USER 人工检查，单屏）；exact-candidate 的 G3/G4 未做 | MANUAL PASS |

## Milestone B entry

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P15-B01 | 身份失配的回执经真实处理入口不产生任何副作用：其他恢复操作的清理与保留回执、已被取代的外部观察 | Automated | `shell::receipt_tests`（去掉身份校验后这些测试失败） | AUTOMATED PASS |
| P15-B02 | 有效的旧 generation 保存回执仍被接受：记录实际落盘的指纹，较新编辑保持 dirty 并补一次保存 | Automated | `shell::receipt_tests::a_valid_older_save_receipt_is_accepted_and_the_newer_edit_stays_dirty` | AUTOMATED PASS |
| P15-B03 | 较旧但内容不同的 temp 在重启时成为恢复候选，两份文件都保留 | Automated | `startup::bootstrap::tests::an_older_different_temporary_survives_a_restart_as_a_recovery_choice` | AUTOMATED PASS |
| P15-B04 | 笔记回执的每个分支（保存、外部观察、清理、保留、F6/F7）的最终诊断、保存次数、恢复与冲突状态、隐藏与退出屏障都已钉住，迁移的每一批都不变；请求经真实 worker 与磁盘核对 | Automated | `shell::phase15_receipt_tests`（`phase 15` 运行；未达 generation 的回执一行在 `2026-11-07` 决定后改为按失败结算） | AUTOMATED PASS |
| P15-B05 | 邮箱按固定优先级取出 8 类任务；每个 `NoteStorageError` 的消息文本不变 | Automated | `execution::io_worker::tests::the_mailbox_drains_every_job_kind_in_its_priority_order`、`persistence::storage::tests::phase15_every_note_storage_error_keeps_its_message` | AUTOMATED PASS |
| P15-B06 | 笔记存储经 `NoteStorage` 端口执行，worker 不再持有路径；错误分类穷举且消息逐字不变；守护保存的判定是共享纯函数 | Automated | `persistence::adapter::tests::phase15_every_storage_error_keeps_its_message_and_gets_its_class`、`flow::note_guard::tests::every_observed_state_against_every_expectation` | AUTOMATED PASS |
| P15-B07 | 保存、外部协调、恢复的决策在 `stickymd-app` 的 `NoteIo`，返回有序副作用；身份失配的回执没有副作用；Windows 只执行副作用，P15-B04 的表格逐行不变 | Automated | `flow::note_io::tests`（Linux 也运行）、`shell::phase15_receipt_tests` | AUTOMATED PASS |
| P15-B08 | Preview worker、任务协议与结果准入在 `stickymd-app`；宿主只注入本地图片读取与唤醒回调 | Automated | `execution::preview_worker::tests`、`flow::preview_io::tests`（Linux 也运行）、`app::preview_runtime::runtime_tests` | AUTOMATED PASS |
| P15-B09 | 资产、导出、配置回执与 worker 停止的每个分支都已钉住，迁移的每一批都不变 | Automated | `shell::phase15_worker_receipt_tests`（`phase 15` 运行） | AUTOMATED PASS |
| P15-B10 | I/O worker 与邮箱在 `stickymd-app`，经 `IoPorts` 注入的资产、导出、配置端口执行；Windows 只实现端口与注入唤醒回调；资产决策在 `AssetIo` | Automated | `execution::io_worker::tests`（Linux 也运行）、`persistence::wiring_tests`（真实 adapter） | AUTOMATED PASS |
| P15-B14 | 存储错误按 plan 05 分类，包装错误继承内层分类；未达 generation 的保存回执按失败结算：结束屏障、恢复操作与保留本地，不解除持久化义务 | Automated | `persistence::adapter::tests`、`flow::note_io::tests` 的 `a_receipt_for_an_unreached_generation_*` | AUTOMATED PASS |
| P15-B11 | 隐藏分请求、回执、提交：回执来自窗口的实际状态，成功才进入 `HiddenToTray` 并释放缓存；被拒绝时保持显示、恢复输入与边缘感应置顶并上报；过期回执无副作用；窗口效果的顺序不变；隐藏中、已隐藏与等待隐藏保存的窗口不会自动收起，隐藏失败或退出取消回到屏幕后重新开始自动收起计时，且不被隐藏时已关闭的弹层阻挡 | Automated | `flow::window::reducer` 的 `a_hide_commits_only_on_its_success_receipt`、`a_rejected_hide_stays_shown_with_its_input_and_caches`、`a_stale_hide_receipt_changes_nothing`、`a_hide_receipt_after_a_show_changes_nothing`、`a_hiding_or_hidden_docked_window_never_collapses`、`a_window_back_from_a_failed_hide_arms_its_auto_collapse`；`shell::phase15_receipt_tests::{closing_and_showing_reach_the_window_in_order,a_rejected_hide_keeps_the_window_shown_through_the_shell,a_quit_cancelled_with_a_failed_hide_restarts_the_auto_collapse,a_rejected_hide_does_not_stay_blocked_by_the_popup_it_closed}`；真实窗口的回执属 P15-A10 | AUTOMATED PASS |
| P15-B13 | 上一检查点的二进制重新打开迁移后写出的数据副本（plan 11 每批出口 3） | Automated local | D 的 Release 编辑并保存后的副本，由 C5、C4、基线 `ae09b67` 的二进制分别打开：退出码 0，27 行里程碑，`note.md` 与 `config.toml` 字节不变（任务文件 D 记录） | AUTOMATED PASS |

## Milestone C

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P15-C01 | `stickymd-desktop` 不含 unsafe，源码无原生类型、`cfg(windows)`/`target_os`、`crate::platform`、`winit::platform::`，清单不依赖 windows、arboard、tray-icon、notify；可用其余 winit 与 softbuffer；源码树为空即失败 | Automated | `governance.rs` `verify_shared_desktop_boundary` 与其回归 `the_shared_desktop_boundary_refuses_native_code` | AUTOMATED PASS |
| P15-C02 | 迁移的几何、标题、帧计划、转换与计数测试在 `stickymd-desktop` 下实际执行；留在宿主的只有两个读宿主源码的守卫 | Automated | `phase 15` 的 `cargo test -p stickymd-desktop --locked`；`shell::title::tests` | AUTOMATED PASS |
| P15-C03 | 只有 `age == 1` 只写 damage 区；`age` 为 0、2、3 全量填充，裁剪后为空也全量；长度不匹配在每个 age 都报错 | Automated | `frame::convert::tests::{age_one_writes_only_the_damage,every_other_age_fills_the_whole_buffer,the_age_decides_before_the_damage_is_clipped,a_length_mismatch_is_an_error_at_every_age}` | AUTOMATED PASS |
| P15-C04 | 工具栏高度只有一份，`geometry()` 的 `f32` 取整与 `ControlLayout` 的 `f64` 路径都不变 | Automated | `geometry::tests::the_toolbar_height_keeps_its_rounding_and_its_exact_widening`；`controls::tests` 现有布局测试 | AUTOMATED PASS |
| P15-C05 | 模块、依赖图、CI 选择与 Linux portable 选择、指纹归属、测试所有者与名字过滤都认识新 crate，迁走的旧规则已删除 | Automated | `ci::selection::tests`、`ci::projection::tests::the_portable_lane_builds_the_shared_desktop_when_it_changes`、`ci::registry`、`every_session_shared_desktop_code_invalidates_every_module`、`every_name_filter_selects_all_of_its_tests`、`unit-test owner verification` | AUTOMATED PASS |
| P15-C06 | `stickymd-desktop` 在 WSL Debian 上构建并通过测试，依赖不含 gtk | Automated local | WSL Debian 的 clippy 与 `cargo test -p stickymd-desktop --locked`；`cargo tree --target x86_64-unknown-linux-gnu` | AUTOMATED PASS |
| P15-C07 | `stickymd-desktop` 在远程 Windows 与 Linux CI 上真实构建（plan 11 C 出口） | Manual | 远程 CI 未运行 | NOT TESTED |
| P15-C08 | 每帧复制字节不超过基线（plan 10，编译条件已变，不能静态豁免） | Automated local | 真实窗口的帧计数，基线 `ae09b67` 与各批数字见任务文件（C4 在 DPI 1.0 上与基线逐项相同）；每批重测，D 后以最终候选为准 | AUTOMATED PASS |
| P15-C09 | Windows 真实桌面回归（plan 11 C 出口；在 D 之后执行一次），单屏部分 | Manual | receipt: [2026-11-08 桌面回归](../report/2026-11-08-phase-15-desktop-regression.md)（本地桌面预检 P15-D09 与 USER 人工检查）。多屏与混合 DPI 见 P15-D10 | MANUAL PASS |
| P15-C10 | 搬迁前钉住现状并在 C3 各批不变：DPI 1.0、1.25、1.5、2.0，三种视图，默认、最小与 294 DIP 阈值两侧尺寸下的控件矩形、工具栏、滚动条、Search 叠层（无字体的标记路径）像素哈希、悬停光标，以及按下经真实 `handle_mouse_button` 的最终路由（整数格、像素中心、矩形边缘的分数点；弹层与 Search 打开时各一份） | Automated | `shell::shell_pins_tests`（`phase 15` 运行；期望行在 `shell/shell_pins/`；修改只由任务文件列出的批次发生） | AUTOMATED PASS |
| P15-C12 | 控件布局、透明度状态、工具栏绘制、滚动条、Search 布局绘制、Esc 判定、按键表与窗格几何命中在 `stickymd-desktop` 下实际执行，包括 Linux；宿主只留无状态转发，以及依赖机器字体的标签宽度检查 | Automated | `phase 15` 的 `cargo test -p stickymd-desktop --locked`（`controls::`、`scroll::`、`search::`、`input::`、`preview::`）；WSL Debian 的同一命令 | AUTOMATED PASS |
| P15-C13 | `Keymap` 类型与默认表在 `stickymd-desktop`（USER 决定 D3），宿主选择 `Keymap::ctrl_standard()`；五个函数的签名与按键表逐字不变 | Automated | `input::keymap::tests`；`host.rs` 的 `KEYMAP`；`shell::phase15_host_tests::phase15_a_key_means_what_the_hosts_table_says` | AUTOMATED PASS |
| P15-C14 | `ControlLayout` 是壳几何的唯一来源：`hit_test` 按弹层、缩放带、Split 同步、按钮、工具栏空白的顺序裁决，悬停与按下共用；绘制、悬停、按下读同一份布局，随窗口尺寸、DPI 与视图更新；旧的模式命中已删除，整数坐标路由、悬停与像素不变（USER 决定 D4） | Automated | `controls::tests::a_hit_is_the_first_of_popup_border_split_control_button_and_toolbar`；`shell::shell_pins_tests`（`phase15_the_layout_follows_the_size_the_dpi_and_the_view` 与未改动的 `press-integer`、`hover`、`toolbar`、`scrollbars`、`search`、`rectangles`） | AUTOMATED PASS |
| P15-C11 | 弹层空白不穿透；关闭弹层的按下只动作一次；原生拖动或缩放失败也算消费，不落到视图切换或正文；壳输入关闭时窗口内任何按下都不动作 | Automated | `shell::shell_pins_tests::{phase15_a_press_on_the_popup_blank_does_not_fall_through,phase15_a_press_that_closes_the_popup_acts_once,phase15_a_failed_drag_or_resize_is_consumed,phase15_no_press_acts_while_shell_input_is_off}` | AUTOMATED PASS |
| P15-C15 | `AppSession` 搬迁前钉住现状并在 C4 各批不变：dirty 的笔记失焦才保存；恢复等待时不启动监视；IME 提交与键盘的编辑门控对恢复、Preview 焦点、退出等待、资产对账、壳输入的全部组合，键盘门不读 Preview 焦点；事件通过的笔记、Preview、窗口 tick 与缩放去抖；放置夹取 220、120、16,384 | Automated | `shell::phase15_session_tests`（`phase 15` 运行） | AUTOMATED PASS |
| P15-C16 | 交互纯规则在 `stickymd-app` 的 `interaction/rules.rs`，在 Linux 也运行：Search 的打开、视图切换与替换，转换范围，主题轮转，选区与放置夹取（220、120、16,384），编辑门控（`preview_focused` 与 `shell_input_enabled` 都是参数），窗口守卫投影 | Automated | `interaction::rules::tests`（`phase 15` 的 `cargo test -p stickymd-app --locked`；WSL Debian 同一命令） | AUTOMATED PASS |
| P15-C18 | 共享应用状态归 `stickymd-app` 的 `AppSession`：壳（C6 起名 `DesktopShell`）只留视图与原生状态并持有 `app: AppSession`；笔记、资产、文档扇出、回执、退出屏障、失焦保存、事件通过的裁决在 `session/`，经 `ShellEffects` 要求壳动作；源码无平台词 | Automated | `session::tests`（Linux 也运行）；`governance.rs` `verify_shared_application_boundary`；回执表 `shell::phase15_receipt_tests`、`shell::phase15_worker_receipt_tests`、`shell::receipt_tests` 除 P15-C17 的一行外不变 | AUTOMATED PASS |
| P15-C19 | 效果顺序与现状一致：文档扇出按 persistence、Preview、编辑会话与 Search、投影、标题、资产；效果引发的步骤先于外层剩余效果完成；键入不增长输出缓冲 | Automated | `session::tests::{a_document_change_asks_in_the_order_of_the_fan_out,a_nested_frame_runs_before_the_rest_of_its_parent,typing_does_not_grow_the_buffer}`；`shell::phase15_receipt_tests::closing_and_showing_reach_the_window_in_order`；`shell::hot_path_tests` | AUTOMATED PASS |
| P15-C20 | 剪贴板是 `Box<dyn ClipboardPort>`，转发保留宿主的 `read_paste` | Automated | `flow::clipboard::tests::a_boxed_clipboard_keeps_the_hosts_own_paste` | AUTOMATED PASS |
| P15-C17 | 退出在同一个 `ApplicationHandler` 回调末尾执行，托盘先销毁；任何屏障未结算时不退出（W7；无事件循环的回退已删除） | Automated | `shell::phase15_session_tests::{exit_comes_after_the_tray_is_destroyed,exit_does_not_appear_while_a_barrier_is_unsettled}`、`session::tests::exit_appears_only_after_the_last_barrier`；前者读 `FakeHost` 的调用记录，证明代码顺序，不证明原生销毁 | AUTOMATED PASS |
| P15-C21 | 壳经窄端口使用全部原生能力，壳（`DesktopShell<H: Host>`，C6 前名 `StickyApp`）对宿主泛型；`Host` 是窄 trait 的捆绑；窗口的可移植调用仍由壳直接调用；壳的非测试源码无 `crate::platform`、`winit::platform::` | Automated | `FakeHost` 与 `WinHost` 都实现 `Host` 并编译（`cargo clippy --workspace --all-targets`）；`shell::phase15_host_tests` | AUTOMATED PASS |
| P15-C22 | 端口的顺序与答复：隐藏、显示、聚焦后重设身份，身份失败恢复任务栏并写诊断；只有确认的隐藏告诉托盘；置顶、透明度、监视、导出、链接、光标叠加的失败写出宿主原文 | Automated | `shell::phase15_host_tests`（`phase 15` 运行）；回执表 `shell::phase15_receipt_tests`、`shell::phase15_worker_receipt_tests`、`shell::receipt_tests`、`shell::shell_pins_tests`、`shell::hot_path_tests` 的期望行不变 | AUTOMATED PASS |
| P15-C23 | `EventSink`、`DesktopEvent`、`HostSignal`、`FakeHost` 在 `stickymd-desktop`，源码无原生词，在 Linux 也运行；`HeadlessApp` 不需要事件循环 | Automated | `host::tests`、`testing::tests`（`cargo test -p stickymd-desktop --locked`；WSL Debian 同一命令）；`verify_shared_desktop_boundary` | AUTOMATED PASS |
| P15-C24 | worker 先于宿主的任何部分停止，所以先于单实例 guard；托盘在事件循环结束之前销毁 | Automated | `shell::phase15_host_tests::phase15_the_worker_stops_before_the_host_drops`、`shell::phase15_session_tests::exit_comes_after_the_tray_is_destroyed` | AUTOMATED PASS |
| P15-C25 | 窗口标记按 W3 分属壳与宿主；`enumerate_active_displays()` 恰好一次的检查与禁用词跟随 `host/monitor_source.rs`；十六进制显示器 id 格式不变 | Automated | `governance.rs`（`phase-entry 00`）；`host::monitor_source::phase8_monitor_projection_tests` | AUTOMATED PASS |
| P15-C26 | `WinHost` 的原生实现（真实窗口的隐藏回执、托盘、唤醒、单实例、显示器切换）与 C5 之前一致 | Manual | 托盘、唤醒、单实例见 P15-D09；显示器切换未测，见 P15-D10 | NOT TESTED |
| P15-C27 | 壳在 `stickymd-desktop` 的 `shell/`（`DesktopShell<H>`），`ApplicationHandler` 在 `shell/handler.rs`；Windows crate 只留装配、`WinHost`、原生 adapter 与存储；`verify_shared_desktop_boundary` 覆盖 `shell/` | Automated | `governance.rs`（`phase-entry 00`）；`cargo clippy --workspace --all-targets --locked -- -D warnings` | AUTOMATED PASS |
| P15-C28 | 壳的测试在 Linux 也运行：WSL Debian 上 `cargo test -p stickymd-desktop --locked` 与 Windows 同为 213 通过、2 个 ignored，壳的 141 个测试都在其中；没有 `cfg` 绕开 | Automated local | WSL Debian `linux-desktop.sh`（clippy、build、test 退出码 0）；远程 Linux CI 属 P15-C07 | AUTOMATED PASS |
| P15-C29 | fixture 不需要 Windows：`AppSession` 由 `SessionParts` 直接构造，剪贴板在内存，磁盘是 `PortableDiskPorts`（`std::fs`，不声称原子性） | Automated | `testing::portable_disk::tests`；`shell::headless_tests` 不引用 Windows 类型 | AUTOMATED PASS |
| P15-C30 | 读写真实文件的 5 个回执行是存储一致性行，期望不变，经真实壳分别跑在 `PortableDiskPorts` 与 `win_io_ports` 上（W1） | Automated | `shell::phase15_receipt_tests::{restoring_publishes_the_temporary_and_then_removes_the_evidence,restoring_over_an_invalid_canonical_moves_it_aside_first,using_the_canonical_removes_the_temporary_without_a_save,using_a_missing_canonical_publishes_it_before_removing_the_temporary,keeping_local_forces_the_save_and_refuses_a_second_choice}` 与 `stickymd-win` 的 `persistence::phase15_storage_conformance::` 同名 5 行 | AUTOMATED PASS |
| P15-C31 | 视图切换、偏好写入时机、启动显示器与初次放置、Search 打开的裁决在 `AppSession`，壳只按 `ShellEffect` 做视图与窗口的部分；这些裁决在 Linux 也有测试 | Automated | `session::decision_tests`（12 个）；`interaction::rules::window_display` 经 `startup_display` 覆盖 | AUTOMATED PASS |
| P15-C32 | 受保护的表在 C6 各提交逐字不变：`phase15_receipt_tests`、`phase15_worker_receipt_tests`、`receipt_tests`、`shell_pins_tests`、`hot_path_tests` 的测试体与 `shell/shell_pins/` 的 8 个文件 | Automated | 各测试在 `stickymd-desktop` 通过；与 `8d3661d` 的逐行比较（任务文件 C6 记录） | AUTOMATED PASS |

## Milestone D

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P15-D01 | 仅为迁移保留的转发已删除：宿主直接导入 `stickymd_app::config`；`stickymd-app` 不提及 winit、softbuffer 或任何平台（`plan_ref` 的契约路径除外），共享层的用户可见文本不带平台名 | Automated | `verify_shared_application_boundary`、`verify_shared_desktop_boundary`（`phase-entry 00`）；在 `crates/stickymd-app` 里搜索 winit 与 softbuffer 均无结果 | AUTOMATED PASS |
| P15-D02 | 完成回执触发的 Preview 任务不超过基线：Preview 下 200 键，每键后交给真实完成处理函数旧文本、当前文本换视口、当前文本同视口三个结果，共 600 个任务，与 `b846e83` 同一 fixture 相同 | Automated | `shell::hot_path_tests::phase15_d_receipt_driven_preview_jobs_stay_at_the_baseline`；基线一侧见任务文件 D 记录 | AUTOMATED PASS |
| P15-D03 | IME 组字提交、Search 输入与每键后截止时间都到期的 Split 输入，快照、保存与 Preview 任务数与 `b846e83` 相同 | Automated | `shell::hot_path_tests::phase15_d_ime_search_and_ticked_typing_stay_at_the_baseline`；基线一侧见任务文件 D 记录 | AUTOMATED PASS |
| P15-D04 | Source CaretOnly 的软件局部呈现只拷贝与转换光标矩形，次数与字节与 `ae09b67` 相同（DPI 1.0 经单进程兼容层、原生 DPI 1.5） | Automated local | 两端临时补丁的 Release 真实窗口，每端 3 次（任务文件 D 记录）；原生 100% 显示器未测 | AUTOMATED PASS |
| P15-D05 | 启动时的破坏性资产整理由共享层决定：恢复等待时不做，否则对载入的引用做一次安全边界整理，缺失与失败进入状态行；Windows 只提供存储 | Automated | `flow::asset_io::startup_tests`（3 个）；`persistence::adapter` 的同步路径不变 | AUTOMATED PASS |
| P15-D06 | Search 替换、缩放写入时机、托盘显示/隐藏、视图切换两步与 Preview 调度的裁决在 `AppSession`，在 Linux 也有测试；视图切换只能按 `begin_view_change` 的结果提交 | Automated | `session::decision_tests` | AUTOMATED PASS |
| P15-D07 | `AppSession` 除 `editor`、`search` 外的字段在产品构建中私有：壳的非测试代码只能用只读视图与会话方法，直接读字段编译失败；测试与受保护表的字段表达式不变 | Automated | `cargo check --locked -p stickymd-desktop` 与 `-p stickymd-win`（feature `testing` 关闭）；任务文件 D 记录的 `E0616` 反例；受保护表与 `8d3661d` 逐字相同 | AUTOMATED PASS |
| P15-D08 | 包装的资产存储错误保留内层分类与完整外层消息（`Directory`、`ChangedDuringProof` 为暂时不可用，`TooLarge` 为失败） | Automated | `persistence::adapter::tests::phase15_a_wrapped_storage_error_keeps_its_class` | AUTOMATED PASS |
| P15-D09 | D 的 Release 在真实窗口上通过本地桌面预检：托盘、托盘失败恢复、贴边、Search 与剪贴板快捷键、公式转换、单实例身份、双副本、微软拼音与微信输入法、崩溃恢复、图片粘贴、用户与受管资产安全、G5 截图四组 | Automated local | `STICKYMD_PREFLIGHT_EXE` 的 17 个 ignored 用例逐个通过（任务文件 D 记录）；这不是 exact-candidate 证据。G3-02 没有通过：见任务文件，基线同样失败，导出的发布在 G3-04 通过，资产改写由 `export` 单元测试覆盖 | AUTOMATED PASS |
| P15-D10 | 多屏与混合 DPI：跨屏移动、贴边、拔显示器、托盘唤醒后的放置与候选窗位置 | Manual | USER 2026-11-08 未测；需要两块显示器 | NOT TESTED |

## Milestone R

回执批（[plan 04](../plan/04_runtime_state_model.md#window-action-receipts)）的真实桌面观察项；状态机与壳的自动化测试见下表的 Automated 行。

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P15-R01 | 托盘唤醒，从隐藏到托盘：窗口可见、未最小化，实际矩形正确，收到焦点事件，可操作，托盘文字与输入状态同步 | Manual | USER 2026-11-09 确认托盘路径的窗口位置与托盘文字（receipt: [2026-11-09 人工检查](../report/2026-11-09-phase-15-window-receipt-manual-check.md)）；本地预检 G4-01 的 `tray-show-restores-window`、`tray-show-takes-focus`、`tray-show-retains-source` 断言可见、取得前台与键盘焦点、内容保留（`35d3c25` 的 Release） | MANUAL PASS |
| P15-R02 | 同目录第二实例唤醒，从隐藏到托盘：同 P15-R01 | Automated local | 本地预检 G4-05（`state=hidden`）的 `second-launch-restores-focused-window`、`second-launch-restores-rect`、`second-launch-tray-text`、`second-launch-immediate-input` 断言可见、未最小化、已激活、矩形不变、托盘文字、可立即输入（`35d3c25` 的 Release）。这不是 exact-candidate 证据 | AUTOMATED PASS |
| P15-R03 | 托盘或第二实例唤醒，从最小化：窗口解除最小化、可见，托盘文字与输入状态同步 | Manual | 托盘路径：USER 2026-11-09 确认窗口位置与托盘文字（receipt: [2026-11-09 人工检查](../report/2026-11-09-phase-15-window-receipt-manual-check.md)）。第二实例路径：本地预检 G4-05 的同两项（`state=minimized`）（`9c19544`） | MANUAL PASS |
| P15-R04 | 托盘或第二实例唤醒，从贴边收起：窗口展开为贴边展开、可操作，托盘文字与输入状态同步 | Manual | 托盘路径：USER 2026-11-09 确认窗口位置与托盘文字（receipt: [2026-11-09 人工检查](../report/2026-11-09-phase-15-window-receipt-manual-check.md)）。第二实例路径：本地预检 G4-05 的 `collapsed-wake-activates-primary` 观察到窗口取得前台与焦点（`9c19544`） | MANUAL PASS |
| P15-R05 | 显示被拒绝或回执过期（需要能让显示失败的环境）：不提交相反可见性；显示失败不恢复输入与 Preview，托盘文字不变，错误可见 | Manual | 未运行 | NOT TESTED |
| P15-R06 | 显示失败后重试：等待能结束，同目标能再次申请，不形成永久动画或等待 | Manual | 未运行 | NOT TESTED |
| P15-R07 | 显示回执读回可见且未最小化：已知不符为失败，无法读回为 `Unsupported`，每种组合都有结果 | Automated | `ports::window::tests::a_show_succeeds_only_when_both_read_backs_are_right` | AUTOMATED PASS |
| P15-R08 | 显示只在成功回执后提交可见性、输入、IME、Preview 恢复与托盘文字；拒绝、`Unsupported` 或过期回执保持原可见性且不启用任何依赖效果并上报；可重试；新请求取代旧请求；等待中的隐藏、退出、守卫、到期计时、拖动结束、拓扑变化都有裁决；显示、隐藏、焦点的回执不来时到期结束，退出取消按当前可见性投影输入与 IME | Automated | `flow::window::show_receipt_tests`（Linux 也运行）；`shell::phase15_host_tests::{phase15_a_rejected_show_changes_nothing_but_the_status_line,phase15_a_show_can_be_retried_after_a_rejection,phase15_a_focus_request_on_a_visible_window_reports_a_bad_read_back}`；受保护表的显示一行见任务文件 R 记录 | AUTOMATED PASS |
| P15-R11 | 贴边动画反向与中断：最终矩形与目标一致，旧动作的迟到观察不覆盖新目标，感应条可达，终态只提交一次 | Manual | USER 2026-11-09 确认反向后最终位置正确、感应条可用（receipt: [2026-11-09 人工检查](../report/2026-11-09-phase-15-window-receipt-manual-check.md)）；迟到观察与只提交一次人眼看不到，由 P15-R16 的自动化覆盖 | MANUAL PASS |
| P15-R12 | 拖动：拖动中不把位置变化当确认；松手后的最终矩形与目标一致，放置只提交一次 | Manual | USER 2026-11-09 确认松手后位置正确、重启后保留（receipt: [2026-11-09 人工检查](../report/2026-11-09-phase-15-window-receipt-manual-check.md)）；拖动中不确认与只提交一次人眼看不到，由 P15-R16 的自动化覆盖 | MANUAL PASS |
| P15-R13 | 拔显示器：窗口回到可用显示器，矩形确认后才写入放置，失败可见，无永久等待 | Manual | 未运行；需要第二块显示器 | NOT TESTED |
| P15-R14 | 混合 DPI：观察矩形与目标不符时结束于诊断且不提交，不挂起；容差实测（P15-D10） | Manual | 未运行；需要第二块显示器 | NOT TESTED |
| P15-R15 | 定位失败后重试：等待能结束，下一次定位能再次申请，不形成永久动画或等待 | Manual | 未运行 | NOT TESTED |
| P15-R16 | 定位分请求、观察与裁决：观察与目标逐边比较（当前容差 0），一次裁决（确认、执行失败、确认超时）；观察不依赖移动事件，到期再观察一次；只有最后一帧与非动画定位被观察，等待中才读窗口；拖动放下与拓扑恢复的 `CommitPlacement` 只在确认后发出；新定位取代旧定位、迟到观察不提交；原生拖动、隐藏、显示、退出、失败后重试都有裁决，无永久等待或动画；隐藏时的拓扑恢复作废等待并只写显示器归属；显示变化待恢复期间不确认任何定位；id 耗尽时不发帧并结束旧等待 | Automated | `flow::window::placement::tests`（Linux 也运行）；`session::decision_tests::a_drag_is_written_only_after_its_frame_was_observed`；`shell::phase15_host_tests::{phase15_an_animation_observes_the_window_only_at_its_last_frame,phase15_a_drag_is_saved_once_the_window_is_observed_there,phase15_a_timed_out_placement_is_reported_and_writes_nothing,phase15_a_topology_change_during_a_drag_reports_nothing,phase15_moved_and_resized_events_can_confirm_after_a_mismatch,phase15_a_scale_change_is_recovered_only_on_the_next_event_turn,phase15_no_observation_settles_a_placement_while_recovery_is_due,phase15_a_recovery_with_no_placement_observes_the_waiting_one_after,phase15_a_collapse_due_at_the_placement_deadline_reads_nothing,phase15_a_window_event_with_no_pending_placement_reads_nothing,phase15_a_native_drag_reads_nothing_and_a_failed_placement_can_be_retried}` | AUTOMATED PASS |
| P15-R09 | 用户置顶与临时感应置顶在真实窗口上：原生置顶位、与普通窗口的层级、不抢焦点；配置与临时状态分离 | Manual | 用户置顶：USER 2026-11-09 确认压在普通窗口之上且不抢焦点（receipt: [2026-11-09 人工检查](../report/2026-11-09-phase-15-window-receipt-manual-check.md)），本地预检 G4-01、G4-02 读回原生位。感应置顶：G4-02 的 `covered-sensor-real-hover` 断言感应条在普通遮挡窗口之上、原生位已置、真实悬停能展开；`dock-leave-delay` 断言悬停展开后移开鼠标 500 ms 收起，说明悬停没有取得焦点；G4-01 的 `tray-topmost-sensor-keeps-config` 断言不改配置（`35d3c25` 的 Release） | MANUAL PASS |
| P15-R10 | 置顶读回与请求一致才算成功，读回不符为失败、无法读回为 `Unsupported`；用户值只在成功回执后进入配置、写盘与托盘勾选，失败不写并上报，可重试；感应值分离且不进配置，隐藏保存与恢复读确认值；原生位按单一顺序结算，新请求取代旧请求，过期回执不提交，失败后按确认值再请求一次，原生位未知时同值的显式重试也到达宿主，回执不来时到期结束；启动路径照旧 | Automated | `ports::window::tests::a_topmost_receipt_is_the_read_back_of_the_wanted_bit`；`flow::window::topmost::tests`（Linux 也运行）；`session::decision_tests::{a_topmost_is_written_only_after_the_window_confirmed_it,a_topmost_before_the_window_flow_goes_straight_to_the_configuration,a_sensor_receipt_writes_no_configuration}`；`shell::phase15_host_tests::{phase15_a_rejected_topmost_writes_nothing_and_can_be_retried,phase15_a_sensor_topmost_stays_out_of_the_configuration,phase15_the_startup_topmost_ignores_the_read_back}`；真实窗口的读回属 P15-R09 | AUTOMATED PASS |
| P15-R17 | R 的 Release 在真实窗口上通过全部本地桌面预检（P15-D09 的用例加 G3-02 导出）；导出对话框由便签窗口拥有，保存或取消关闭后前台回到便签窗口 | Automated local | `STICKYMD_PREFLIGHT_EXE` 的 18 个 ignored 用例逐个通过（任务文件“最终桌面回归”）：17 个在 `9c19544`，修复导出后的 G3-02 在 `b6e2be1`，含取消与两次关闭后的前台断言。这不是 exact-candidate 证据；置顶时对话框在便签之上由 USER 人工确认（receipt: [2026-11-09 人工检查](../report/2026-11-09-phase-15-window-receipt-manual-check.md)） | AUTOMATED PASS |

## Milestone E

不依赖文件语义的裁决移入 `stickymd-app`（[`11` Phase 15 节](../plan/11_testing_and_release.md#phase-15-shared-core-migration)）。设计与审查结论见[报告](../report/2026-11-09-phase-15-e-shared-startup-logic.md)。

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P15-E01 | `config.toml` 的解析、版本裁决、序列化、保留文件命名与警告文字在共享层，宿主只读文件、不覆盖地改名和发布；读取失败、解析失败、版本更新、保留失败各有结果 | Automated | `config::document::tests`（Linux 也运行）；`config::storage::tests` 断言不变，含 `config.toml` 字节 | AUTOMATED PASS |
| P15-E02 | 启动恢复由共享纯函数裁决，宿主观察并执行：每种观察组合的步骤，超限临时文件先隔离，无正文恢复用宿主的换行符（CRLF、LF），无正文首次创建仍用 `Guarded { expected: None }` | Automated | `flow::note_startup::tests`（Linux 也运行）；`startup::bootstrap::tests` 断言不变 | AUTOMATED PASS |
| P15-E03 | 粘贴批次在共享层：各输入分支、批次上限、失败时返回已发布台账、错误文字与语义分类不变，文件解码失败仍归 `MixedOrUnsupportedFiles` | Automated | `flow::asset_paste::tests`（Linux 也运行）；`assets::transaction::tests` 与 `persistence::adapter` 的分类表断言不变 | AUTOMATED PASS |
| P15-E04 | 导出规划只返回命名与复用决定：源记住自己的名字（键由宿主给出，不折叠大小写），同内容沿用第一个名字，名字前缀冲突时加长，询问名字不改变状态。实际复制次数、重复副本删除失败与移动失败时的记录顺序留在宿主，这里没有单独的故障用例 | Automated | `flow::export_plan::tests`（Linux 也运行）；`export::tests` 除移走的命名用例外断言不变 | AUTOMATED PASS |
| P15-E05 | 百分号解码在共享层，路径语法校验留在宿主 | Automated | `preview::parser::tests`；`assets::path::tests` 断言不变 | AUTOMATED PASS |
| P15-E06 | 链接 scheme 分类只有一处，宿主打开前用它复核；笔记文件过滤在共享层 | Automated | `preview::parser::tests`；`platform::windows::shell::tests::forged_allowed_kind_cannot_bypass_adapter_reclassification`；`flow::reconciliation::tests::only_the_working_note_and_its_temporary_need_a_reconciliation` | AUTOMATED PASS |
| P15-E07 | 共享 crate 能编译到 `aarch64-apple-darwin`，依赖策略覆盖该目标 | Automated local | `cargo check --locked --target aarch64-apple-darwin -p stickymd-core -p stickymd-render -p stickymd-app -p stickymd-desktop`；`cargo deny check`。这只是编译，macOS 上没有运行过 | AUTOMATED PASS |
| P15-E08 | Release 在真实窗口上通过全部本地桌面预检 | Automated local | `STICKYMD_PREFLIGHT_EXE` 的 18 个 ignored 用例（`35d3c25` 的 Release，单屏）：托盘 20、托盘失败恢复 1、单实例 27、贴边 54、Search 15、公式转换 4、双副本 1+1、微软拼音与微信输入法 72、图片粘贴 13、崩溃恢复 17、用户资产 11、受管资产 4、导出 4、G5 四组。崩溃恢复修掉脚本竞态后连跑 6 次通过 5 次，另 1 次命中保存前的空临时文件窗口，见任务文件 Open Items。这不是 exact-candidate 证据 | AUTOMATED PASS |

## 窗口启动失败

[`09` Failure Paths](../plan/09_windows_shell.md#windows-shell-failure-paths) 的验证项；USER 批准的方案见[分析报告](../report/2026-11-09-startup-failure-paths.md)。

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P15-S01 | 窗口无法使用或无法安全放置时结束启动：`abort_startup` 依次移除托盘、提示原因、结束事件循环，并记录失败；无显示器与工具窗口身份（显示前后）在此之前停止后续步骤 | Automated | `shell::startup_tests::{a_stopped_startup_removes_the_tray_then_explains_then_exits_and_is_recorded,without_a_display_the_startup_stops_before_it_creates_anything,an_identity_refused_before_the_show_stops_the_startup_before_the_window_is_placed,an_identity_lost_after_the_show_stops_the_startup_before_the_window_is_styled}`；`flow::startup::tests::every_stop_says_why_with_the_platform_text`（Linux 也运行）。窗口与绘制表面失败在 `resumed` 里调用 `abort_startup` 需要真实事件循环，未被测试 | AUTOMATED PASS |
| P15-S02 | 透明度或置顶无法应用时启动继续：状态行说明，不弹窗（恢复提示占用状态行时见 P15-S03），其后的步骤照常；置顶的原生位记为未知：用户来源的同值重试恰好到达宿主一次，配置与托盘不变，成功后同值不再请求；用户与感应来源在 reducer 上都只发出一个目标正确的请求，成功后不再重复。Preview 视图没有状态行，只有工具栏的诊断标记，切到 Source 或 Split 后可见，与所有诊断相同（读代码，未测试） | Automated | `shell::startup_tests::{a_refused_opacity_is_reported_and_the_startup_goes_on,a_refused_topmost_is_reported_and_can_be_retried_once}`；`flow::window::topmost::tests::a_bit_the_startup_could_not_set_lets_a_repeat_reach_the_host_from_either_source` | AUTOMATED PASS |
| P15-S03 | 托盘创建失败仍然弹窗一次并继续，任务栏入口在显示前恢复；启动阶段幸存的失败与已有诊断都写在一行状态行上；恢复提示占用状态行时，失败原因改用对话框，恢复提示保持原样 | Automated | `shell::startup_tests::{a_missing_tray_interrupts_once_and_the_startup_goes_on_with_a_taskbar_entry,every_survived_failure_of_a_startup_has_its_place_on_the_one_status_line,a_survived_failure_joins_what_the_status_line_already_says,a_survived_failure_goes_to_a_dialog_while_the_recovery_prompt_holds_the_status_line,a_missing_tray_is_told_once_while_the_recovery_prompt_holds_the_status_line}`；`flow::startup::tests::only_a_missing_tray_interrupts_a_startup_that_goes_on` | AUTOMATED PASS |
| P15-S04 | 启动的完整编排：里程碑、原生调用与显示、聚焦两步的顺序不变，不提示、不记录失败；诊断探针的退出先移除托盘再结束事件循环 | Automated | `shell::startup_tests::{a_healthy_startup_keeps_its_order_and_reports_nothing,a_measuring_hosts_exit_removes_the_tray_first}`；`resumed` 把 `set_visible`、`focus_window` 传入这两步由读代码确认；真实启动的里程碑顺序见任务文件“窗口启动失败”（`ce77223`） | AUTOMATED PASS |
| P15-S05 | `window_event`、`about_to_wait`、`user_event` 的第一条语句是启动失败时返回 | Automated | `shell::startup_tests::every_callback_starts_by_returning_after_a_failed_startup` 固定源码；守卫的运行属 P15-S06 | AUTOMATED PASS |
| P15-S06 | 真实进程：终止路径的提示可见、托盘消失、退出码非 0；`exit` 之后缓冲的事件不再处理，提示期间的输入不进入笔记；事件循环运行失败时先结束壳再提示，窗口与托盘的原生清理完成；`MessageBoxW` 失败时的表现 | Manual | 未运行；这些失败在真实桌面上难以制造，`main.rs` 的退出码分支没有测试 | NOT TESTED |
