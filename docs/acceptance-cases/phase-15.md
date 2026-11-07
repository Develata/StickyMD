# Phase 15 Acceptance Matrix

本矩阵验证共享实现迁移（[`11` Phase 15 节](../plan/11_testing_and_release.md#phase-15-shared-core-migration)）。
它不是资格化 suite：Phase 14 的 exact-candidate 与发布门不变，`stickymd-app` 的测试经 `cargo test --workspace` 进入既有
收据。基线提交为 `b846e83`（代码移动前）。未实现的里程碑与未运行的项保持 `NOT TESTED`。

## Milestone A

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P15-A01 | `stickymd-app` 不含 unsafe，不依赖 winit、softbuffer 或平台 crate，源码无平台类型与 `cfg(windows)`/`target_os` | Automated | `governance.rs` `verify_shared_application_boundary` 与其回归 | AUTOMATED PASS |
| P15-A02 | 迁移的语义测试在 `stickymd-app` 下实际执行，Linux portable 选择包含该包 | Automated | `cargo test -p stickymd-app --locked`（Windows；WSL Debian 115 passed、2 个 Release 基线 ignored）；`ci` portable 选择回归 | AUTOMATED PASS |
| P15-A03 | 按名字过滤的 `cargo test` 任务选中其测试：Windows 默认构建的实际测试清单里，任务按其模式（`--ignored`、`--lib`）至少运行所选包的一个测试，所选包之外的产品包没有它该运行的测试；各平台的源码核对覆盖其他平台才编译的测试；Release 或其他 profile 才编译的测试不在核对范围 | Automated | `unit_owners::verify` 的 `verify_filters`（`unit-test owner verification`，Windows 全量）；`runner::test_filters`（各平台）；`runner::filtered_tasks` 拒绝未建模的参数；G4 定向运行以通过数为 0 判失败 | AUTOMATED PASS |
| P15-A04 | 新增路径有资格化指纹规则，旧规则无命中或产品根之外的源码即失败 | Automated | `qualification::module_ledger::fingerprint` 分类与回归 | AUTOMATED PASS |
| P15-A05 | `config.toml` 字节在代码移动前后一致，load 后 save 不改变字节 | Automated | `config::storage::tests::phase15_config_bytes_are_fixed_and_survive_a_load_and_save`（基线提交与迁移后各通过一次；这不等于回退重开同一副本，后者见 P15-B13） | AUTOMATED PASS |
| P15-A06 | 无操作启动与保存不改变 note、config、图片字节 | Automated | `startup::bootstrap::tests::phase15_a_no_op_start_and_save_leave_the_data_byte_identical`（基线提交与迁移后各通过一次） | AUTOMATED PASS |
| P15-A07 | 每键全文快照次数与每次编辑提交给 worker 的后台构建次数不超过基线（`b846e83` 上实测：Source 0、Split 1、Preview 每键 1） | Automated | `app::hot_path_tests::phase15_a_typing_burst_stays_at_the_baseline_snapshot_and_build_counts`（经真实 Preview worker 计数） | AUTOMATED PASS |
| P15-A08 | 每帧复制字节未被 A 触及 | Automated | `surface.rs` 与 `app/presentation.rs` 均无改动（`git diff`） | AUTOMATED PASS |
| P15-A09 | Release 基线（Source 流水线、Source 查找、窗口 reducer）不低于基线提交的硬门 | Automated | `phase 15 --performance`；基线与迁移后数字见任务文件 | AUTOMATED PASS |
| P15-A10 | Windows 真实桌面回归（托盘、贴边、输入法、剪贴板）在 A 后无变化 | Manual | G3/G4 exact 需独占桌面 | NOT TESTED |

## Milestone B entry

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P15-B01 | 身份失配的回执经真实处理入口不产生任何副作用：其他恢复操作的清理与保留回执、已被取代的外部观察 | Automated | `app::receipt_tests`（去掉身份校验后这些测试失败） | AUTOMATED PASS |
| P15-B02 | 有效的旧 generation 保存回执仍被接受：记录实际落盘的指纹，较新编辑保持 dirty 并补一次保存 | Automated | `app::receipt_tests::a_valid_older_save_receipt_is_accepted_and_the_newer_edit_stays_dirty` | AUTOMATED PASS |
| P15-B03 | 较旧但内容不同的 temp 在重启时成为恢复候选，两份文件都保留 | Automated | `startup::bootstrap::tests::an_older_different_temporary_survives_a_restart_as_a_recovery_choice` | AUTOMATED PASS |
| P15-B04 | 笔记回执的每个分支（保存、外部观察、清理、保留、F6/F7）的最终诊断、保存次数、恢复与冲突状态、隐藏与退出屏障都已钉住，迁移的每一批都不变；请求经真实 worker 与磁盘核对 | Automated | `app::phase15_receipt_tests`（`phase 15` 运行） | AUTOMATED PASS |
| P15-B05 | 邮箱按固定优先级取出 8 类任务；每个 `NoteStorageError` 的消息文本不变 | Automated | `execution::io_worker::tests::the_mailbox_drains_every_job_kind_in_its_priority_order`、`persistence::storage::tests::phase15_every_note_storage_error_keeps_its_message` | AUTOMATED PASS |
| P15-B06 | 笔记存储经 `NoteStorage` 端口执行，worker 不再持有路径；错误分类穷举且消息逐字不变；守护保存的判定是共享纯函数 | Automated | `persistence::adapter::tests::phase15_every_storage_error_keeps_its_message_and_gets_its_class`、`flow::note_guard::tests::every_observed_state_against_every_expectation` | AUTOMATED PASS |
| P15-B07 | 保存、外部协调、恢复的决策在 `stickymd-app` 的 `NoteIo`，返回有序副作用；身份失配的回执没有副作用；Windows 只执行副作用，P15-B04 的表格逐行不变 | Automated | `flow::note_io::tests`（Linux 也运行）、`app::phase15_receipt_tests` | AUTOMATED PASS |
| P15-B08 | Preview worker、任务协议与结果准入在 `stickymd-app`；宿主只注入本地图片读取与唤醒回调 | Automated | `execution::preview_worker::tests`、`flow::preview_io::tests`（Linux 也运行）、`app::preview_runtime::runtime_tests` | AUTOMATED PASS |
| P15-B09 | 资产、导出、配置回执与 worker 停止的每个分支都已钉住，迁移的每一批都不变 | Automated | `app::phase15_worker_receipt_tests`（`phase 15` 运行） | AUTOMATED PASS |
| P15-B10 | I/O worker 与邮箱在 `stickymd-app`，经 `IoPorts` 注入的资产、导出、配置端口执行；Windows 只实现端口与注入唤醒回调；资产决策在 `AssetIo` | Automated | `execution::io_worker::tests`（Linux 也运行）、`persistence::wiring_tests`（真实 adapter） | AUTOMATED PASS |
| P15-B11 | 隐藏分请求、回执、提交：回执来自窗口的实际状态，成功才进入 `HiddenToTray` 并释放缓存；被拒绝时保持显示、恢复输入与边缘感应置顶并上报；过期回执无副作用；窗口效果的顺序不变；隐藏中、已隐藏与等待隐藏保存的窗口不会自动收起，隐藏失败或退出取消回到屏幕后重新开始自动收起计时，且不被隐藏时已关闭的弹层阻挡 | Automated | `flow::window::reducer` 的 `a_hide_commits_only_on_its_success_receipt`、`a_rejected_hide_stays_shown_with_its_input_and_caches`、`a_stale_hide_receipt_changes_nothing`、`a_hide_receipt_after_a_show_changes_nothing`、`a_hiding_or_hidden_docked_window_never_collapses`、`a_window_back_from_a_failed_hide_arms_its_auto_collapse`；`app::phase15_receipt_tests::{closing_and_showing_reach_the_window_in_order,a_rejected_hide_keeps_the_window_shown_through_the_shell,a_quit_cancelled_with_a_failed_hide_restarts_the_auto_collapse,a_rejected_hide_does_not_stay_blocked_by_the_popup_it_closed}`；真实窗口的回执属 P15-A10 | AUTOMATED PASS |
| P15-B13 | 上一检查点的二进制重新打开迁移后写出的数据副本（plan 11 每批出口 3） | Manual | 需独占桌面；字节 golden 与真实磁盘恢复测试已覆盖无窗口的部分 | NOT TESTED |
