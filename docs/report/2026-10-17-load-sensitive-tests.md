# 负载下时序敏感测试的加固（2026-10-17）

## 背景与范围

[`2026-10-10` 报告](2026-10-10-manual-reuse-and-domain-split.md) C2b-2 各轮记录了几项只在机器高负载（CPU 96–100%、磁盘紧张）
下失败、单独重跑通过的既有测试。USER 要求在不削弱断言、不改产品行为的前提下让它们对调度延迟稳健，并给出加压前后的失败率。

涉及测试：

| 测试 | 位置 |
| --- | --- |
| `preview::worker::tests::{worker_completion_keeps_generation_and_typed_failure, scrollbar_completion_preserves_requested_offset_even_after_clamping, raster_release_keeps_semantic_tree_for_fast_preview_return, document_projection_release_forces_the_next_visible_request_to_rebuild}` | `apps/stickymd-win/src/preview/worker.rs` |
| `managed_process::tests::stale_smoke_preflight_fails_without_killing_the_existing_process` | `tools/stickymd-smoke/src/managed_process.rs` |
| `qualification::exact_desktop::tests::a_descendant_holding_the_output_is_ended_with_the_helper` | `tools/stickymd-smoke/src/qualification/exact_desktop.rs` |
| `qualification::manual::instance::tests::a_candidate_that_exits_at_once_is_a_startup_failure` | `tools/stickymd-smoke/src/qualification/manual/instance.rs` |

`raster_release_keeps_semantic_tree_for_fast_preview_return` 不在原始失败记录中，但与另三项 Preview worker 测试是同一模式、同一模块，
且在下文的复现中同样失败，一并处理。

契约边界：`docs/acceptance-cases/`、`docs/coverage-matrix.md`、`unit_owners` 与人工覆盖表均未点名这些测试（全仓 `rg` 只在上述报告中
出现），因此不涉及 plan 或验收投影的修改。plan 11 规定 deterministic test 不适用成功率容错；本次没有任何断言重试，只有夹具清理在
边界内重试。产品 runtime（`apps/` 的非测试代码、`crates/`）未改。唯一的非测试改动在 smoke 工具内部：`manual::instance::start`
增加启动超时参数，生产调用方传入原来的 `exact_desktop::TIMEOUT`，行为不变。

## 逐项根因与修改

### Preview worker（4 项）

- **根因（已复现）**：每个 `PreviewWorker` 在工作线程上先构造 `PreviewPipeline`，其中 `FontSystem::new()` 扫描系统字体，之后才处理
  第一个任务；测试用 `recv_timeout(5 s)` 等完成通知，这 5 s 同时覆盖了字体扫描。空载时一项只需等 `NoDocument` 失败的测试耗时约
  0.17 s（几乎全部是字体扫描）。测试进程拿不到足够 CPU 时 5 s 到期返回 `Timeout`——复现中的 panic 位置与原始记录一致
  （`worker.rs:529`、`559`、`691`、`723`）。只加普通优先级忙循环（档位 A）时这些测试始终通过（推测与 Windows 的线程优先级提升有关，
  未核实），须限制测试进程本身的 CPU 份额才能复现。
- **修改**：测试模块内新增 `next_completion`，以 `COMPLETION_DEADLINE = 120 s` 等待完成通知，超时 panic 并说明原因；7 处
  `recv_timeout(5 s).unwrap()` 改为调用它。完成通知本身就是要等的事件，期限只防挂起，不是延迟断言；关于 generation、
  `NoDocument`、viewport、`scroll_y`、metrics 的断言逐字未变。`#[ignore]` 的 Phase 6 release 基线不动（其时限即被测量对象）。

### `stale_smoke_preflight_fails_without_killing_the_existing_process`

- **根因（推断，未复现）**：子进程 `kill` + `wait` 返回后立刻 `remove_dir_all` 夹具。Windows 上进程对象变为 signaled 时，映像节（以及
  对刚复制的 EXE 的扫描）可能尚未释放，删除其 EXE 短暂失败；负载会拉长这个窗口。本次加压前后各 5 次均未复现，原始失败当时伴随
  磁盘紧张。
- **修改**：清理改为 `remove_after_exit`：在子进程已被回收后于 30 s 内指数退避重试删除，超时仍 panic 并给出最后的错误；仅当一次失败
  的尝试之后再遇到 `NotFound` 才视为已删除。"preflight 识别该 PID 且不终止它"的断言未变。

### `a_descendant_holding_the_output_is_ended_with_the_helper`

- **根因（部分复现，其余为推断）**：helper 与其后代都是新启动的测试二进制，原测试有四个随负载变化的时限：helper 等后代启动 10 s、
  `run_bounded_output` 总时限 30 s、"及时返回" `elapsed < 20 s`，以及后代 3 s 后自行写 `survived.txt`——若 helper 退出到 job 关闭之间
  的间隔超过 3 s，正确实现也会被误判为"后代存活"。CPU 限额下复现的是 `elapsed < 20 s`（20.5 s、20.1 s）；其余三处与原始记录中
  54.7 s 的失败属于哪一处均未复现、为推断。
- **修改**：后代不再按自己的计时器写哨兵，而是持有管道直到被探测（或到 `HOLDER_LIFETIME = 240 s`）；调用返回后测试写入探测文件，
  4 s 内哨兵出现即判"后代存活"。时限改为 `HOLDER_START = 150 s`、`HELPER_DEADLINE = 180 s`。
- **意图保持**：三项性质仍逐一被证明——后代确实运行过（`.started`）；调用不等待后代释放管道；后代没有存活（存活者会立即回应探测）。
  其中绝对时限 `elapsed < 20 s` 不是被删除，而是由结构性证明取代：后代持有管道的时间超过调用期限，若调用等待管道关闭就不可能成功
  返回。这比原断言更强——原设计下等待后代自然退出（3 s）的实现仍能在 20 s 内返回。正确实现下后代已被 job 结束，永远不会写哨兵，
  因此"后代存活"判定在任何负载下都不会误报；负载只可能降低对回归的检出灵敏度。
- **代价**：仅在回归时，未被结束的后代会存活至多 `HOLDER_LIFETIME`（4 min），期间占用 `stickymd_smoke-*.exe`，可能使重新链接失败。

### `a_candidate_that_exits_at_once_is_a_startup_failure`

- **根因（已核实）**：`exact_desktop::TIMEOUT` 为 12 s，测试断言 `elapsed < 10 s`，而计时还包含 `fixture::seed`、EXE 的 SHA-256 与挂起
  启动；负载下两种失败都出现过：`hostname.exe` 副本 12 s 内未退出（报"did not write … within 12s"），以及已走退出分支但耗时超过 10 s
  （"waited for the full timeout"）。纯测试侧无法修复前者：只要产品时限是 12 s，负载下进程晚于 12 s 退出就必然走超时分支。
- **修改**：`start` / `wait_for_startup` 增加启动超时参数，`Workshop::launch` 传 `exact_desktop::TIMEOUT`（行为不变）；测试传
  `STARTUP_TIMEOUT = 120 s` 并断言 `elapsed < STARTUP_TIMEOUT`。"在进程退出时立即结束等待、而非等满期限"的判别力保留：
  只在期限处才注意到退出的实现会耗时 ≥ 120 s 而失败。

## 变异检验（证明断言未被削弱）

对修改后的测试注入回归，确认仍失败（检验后已还原，`git diff` 中无残留）：

| 注入的回归 | 测试结果 |
| --- | --- |
| `run_bounded_output` 的 job 改为 `Descendants::BreakAway`（后代不随 helper 结束） | descendant 测试 FAILED：`pipe-holding helper stdout stayed open after it exited`（180.4 s） |
| 同上，且读取端放弃管道立即返回（调用"及时"返回、后代存活） | descendant 测试 FAILED：`the descendant outlived its helper`（0.4 s） |
| `wait_for_startup` 只在期限到达时才检查进程是否退出 | quick-exit 测试 FAILED：`waited for the full timeout`（121.4 s） |

Preview worker 与 preflight 两组只改了等待期限与清理方式，断言未动，未做变异检验。

## 加压测量

方法：在 `8bfdedf`（修改前）与修改后各构建一次测试二进制并复制出来，逐项用 `--exact`（Preview worker 用模块过滤
`preview::worker::tests`）循环运行；判定要求退出码 0 且输出含 `test result: ok.`。负载档位：

- **A**：一个 pwsh 进程运行 40 个普通优先级忙循环线程，覆盖全部 20 个逻辑 CPU；测试普通优先级。
- **B**：测试进程固定在 CPU 0、优先级 BelowNormal，4 个普通优先级忙循环线程同样固定在 CPU 0；测试线程只能靠 Windows 的饥饿提升
  获得 CPU。
- **C**：不加忙循环，把测试进程（及其之后启动的子进程）放进 CPU 速率硬上限的 job（`JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP`），
  上限以整机 20 个逻辑 CPU 的百分比计，类似 CI 的 CPU 配额。

结果（失败的运行 / 运行次数；Preview worker 每次运行含 4 项相关测试，失败的运行中 4 项全部超时）：

| 测试 | 档位 | 修改前 | 修改后 | 备注 |
| --- | --- | --- | --- | --- |
| Preview worker（模块） | A | 0/20（整个 stickymd-win 二进制另 0/20） | 0/10 | 修改前每次 6–15 s；修改后 2–3 s，首次 44 s |
| Preview worker（模块） | C 0.25% | **5/5** | **0/5** | 修改后模块 84–94 s |
| preflight | A | 0/5 | 0/5 | 每次约 210–255 s |
| descendant | A | 0/10 | 0/10 | 每次约 5–7 s |
| descendant | C 0.1% | **2/10**（`elapsed` 20.5 s、20.1 s） | **0/10** | 修改后 11–27 s，其中 5 次超过旧的 20 s |
| quick-exit | A | **10/10**（均为 "waited for the full timeout"，约 42 s） | **0/10** | 修改后约 42 s |

边界（修改后仍失败，说明期限的适用范围而非修复无效）：

| 测试 | 档位 | 修改前 | 修改后 |
| --- | --- | --- | --- |
| Preview worker（模块） | C 0.1% | 2/2（5 s 超时） | 2/2（120 s 超时，4 项中 3 项） |
| Preview worker（模块） | B | 2/2（5 s 超时，模块 635–1116 s） | 3/3（120 s 超时，模块 341–491 s） |

说明：

- 档位 B 与 C 都按比例拉长所有等待，任何固定的墙钟期限都存在一个更低的 CPU 份额使其失效。旧的 5 s 在 0.5% 与 0.25% 之间失效
  （0.5% 时 1 次试跑，模块 4.6 s 通过），新的 120 s 在 0.25% 与 0.1% 之间失效；档位 A（USER 描述的整机忙循环）下修改前后均未触发 Preview
  worker 失败。
- quick-exit 的另一种失败形态"did not write its note and configuration within 12s"在档位 A 的早期 2 次探测中出现过 1 次（另一次为
  "waited for the full timeout"），这正是纯测试侧放宽时限无法修复、需要启动超时参数的原因。
- preflight 的清理竞争在档位 A 下前后各 5 次均未出现，修改依据是原始失败记录与 Windows 语义，未经负载复现。
- 判定以退出码 0 且含 `test result: ok.` 为准。早期一组用 `cmd start /wait` 启动的试跑丢失了退出码，已整体作废，未计入上表。

## 需要 USER 判断的一项

Preview worker 的 `COMPLETION_DEADLINE = 120 s` 是原 5 s 的 24 倍，在档位 A 与 0.25% 限额下都有余量，但 0.25% 时模块已用到 84–94 s。
可选：维持 120 s；或提高到 300–600 s（仍低于 CI 作业的 20/60 min 超时，只会让真正的死锁更晚报告）；或改为无期限 `recv()`
（与负载完全无关，但真正的死锁会挂起到 CI 作业超时；工作线程 panic 时通道断开仍会立即失败）。本次维持 120 s。

## 验证

修改后、空载（同一工作树，基于 `8bfdedf`）：

- `cargo fmt --all -- --check`：通过。
- `cargo clippy -p stickymd-win -p stickymd-smoke --all-targets --locked -- -D warnings`：0 警告。
- `cargo test --workspace --locked --no-fail-fast`：退出 0；stickymd-win 287 passed / 8 ignored，stickymd-smoke 534 passed / 17 ignored
  与 headless 26 passed，core 50 + 5、render 168 及其集成测试全部通过。

## 未处理与后续

- 仓库中另有同类固定短时限、但未见失败记录的测试（`apps/stickymd-win/src/persistence/worker.rs` 的 3 s 接收、
  `platform/windows/file_watch.rs` 的 3 s、`platform/windows/single_instance.rs` 的 1 s）。本次按范围未改。
- 修改的文件进入模块账本指纹，相关模块的既有成功记录将不再复用，下次资格化需重跑。
