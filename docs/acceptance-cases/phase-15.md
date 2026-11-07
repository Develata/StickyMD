# Phase 15 Acceptance Matrix

本矩阵验证共享实现迁移（[`11` Phase 15 节](../plan/11_testing_and_release.md#phase-15-shared-core-migration)）。
它不是资格化 suite：Phase 14 的 exact-candidate 与发布门不变，`stickymd-app` 的测试经 `cargo test --workspace` 进入既有
收据。基线提交为 `b846e83`（代码移动前）。未实现的里程碑与未运行的项保持 `NOT TESTED`。

## Milestone A

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P15-A01 | `stickymd-app` 不含 unsafe，不依赖 winit、softbuffer 或平台 crate，源码无平台类型与 `cfg(windows)`/`target_os` | Automated | `governance.rs` `verify_shared_application_boundary` 与其回归 | AUTOMATED PASS |
| P15-A02 | 迁移的语义测试在 `stickymd-app` 下实际执行，Linux portable 选择包含该包 | Automated | `cargo test -p stickymd-app --locked`（Windows；WSL Debian 115 passed、2 个 Release 基线 ignored）；`ci` portable 选择回归 | AUTOMATED PASS |
| P15-A03 | 按名字过滤的 `cargo test` 任务选中其全部测试：过滤器至少命中一个所选包，且所选包之外的产品包没有匹配测试 | Automated | `runner::test_filters`；G4 定向运行以通过数为 0 判失败 | AUTOMATED PASS |
| P15-A04 | 新增路径有资格化指纹规则，旧规则无命中或产品根之外的源码即失败 | Automated | `qualification::module_ledger::fingerprint` 分类与回归 | AUTOMATED PASS |
| P15-A05 | `config.toml` 字节在代码移动前后一致，load 后 save 不改变字节 | Automated | `config::storage::tests::phase15_config_bytes_are_fixed_and_survive_a_load_and_save`（基线提交与迁移后各通过一次，即回退后可重开同一副本） | AUTOMATED PASS |
| P15-A06 | 无操作启动与保存不改变 note、config、图片字节 | Automated | `startup::bootstrap::tests::phase15_a_no_op_start_and_save_leave_the_data_byte_identical`（基线提交与迁移后各通过一次） | AUTOMATED PASS |
| P15-A07 | 每键全文快照次数与每次编辑的后台构建次数不超过基线 | Automated | `app::hot_path_tests::phase15_a_typing_burst_stays_at_the_baseline_snapshot_and_build_counts` | AUTOMATED PASS |
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
| P15-B05 | 邮箱按固定优先级取出 8 类任务；每个 `NoteStorageError` 的消息文本不变 | Automated | `persistence::worker::tests::phase15_the_mailbox_drains_every_job_kind_in_its_priority_order`、`persistence::storage::tests::phase15_every_note_storage_error_keeps_its_message` | AUTOMATED PASS |
| P15-B06 | 笔记存储经 `NoteStorage` 端口执行，worker 不再持有路径；错误分类穷举且消息逐字不变；守护保存的判定是共享纯函数 | Automated | `persistence::adapter::tests::phase15_every_storage_error_keeps_its_message_and_gets_its_class`、`flow::note_guard::tests::every_observed_state_against_every_expectation` | AUTOMATED PASS |
