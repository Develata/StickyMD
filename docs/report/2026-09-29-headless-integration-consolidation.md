# 2026-09-29 Headless 集成测试合并

本报告记录开发验证工具的本机试验，不构成产品或候选资格化证据。检查起点为 clean
`32c7d4cba18507a07318517c83accdc7c6c313e6`；本轮遵循
[plan 11 的验证并发边界](../plan/11_testing_and_release.md#phase-verification-harness)，
延续[此前的 PowerShell / CI 并行维护](2026-09-29-headless-test-parallelism.md)。

## 问题与保留方案

此前 PowerShell 5.1 与 7 已能并发，但 CLI、包路径 wrapper、发布 wrapper 仍是三个独立
Cargo 集成测试可执行文件，依次运行。本轮把它们注册为同一个 `headless` target 的模块，
让现有 Rust test harness 调度这些独立用例。Cargo 继续统一编译，普通 module、phase、
workspace 命令的覆盖保持不变；未引入测试运行器或依赖。

| 资源 | 隔离方式 |
| --- | --- |
| 测试输入和输出 | 各用例使用原有独立临时目录；两版发布 wrapper 的目录额外含 edition |
| 环境、当前目录、控制台编码 | 只改变子进程；Windows PowerShell 共用 `CREATE_NO_WINDOW` 和移除 `PSModulePath` 的启动 helper |
| 仓库与预编译 CLI | 共享读取；所有 PowerShell 子进程和读取 Git status 的 CLI 规划用例设置 `GIT_OPTIONAL_LOCKS=0` |
| Cargo | wrapper 转交预编译 CLI；许可证 metadata/cache 访问仍由 Cargo 持有，不启动并发 Cargo build |
| 同版 PowerShell fixture | 内部断言有状态依赖，继续按原有顺序运行 |

三个原有 Rust 文件及两份 PowerShell fixture 保留；既有断言、平台条件与 PowerShell 7
缺失时的 `NOT_TESTED` 诊断不变。Windows PowerShell 5.1 仍必需，本机两版均实际执行。
显式 `--test-threads=1` 仍能串行化集成用例，GUI、startup、CPU/内存与资源资格化保持独占。

Cargo 的独立 integration target 自动发现已关闭，以免模块重复执行。以后新增模块须在
`tests/headless.rs` 注册；README 已说明新的定向命令，例如：

```powershell
cargo test -p stickymd-smoke --locked --test headless release_wrappers::
```

## 本机对照

环境：Windows x64，20 个逻辑处理器，Rust/Cargo 1.97.1，`RUST_TEST_THREADS` 未设置。
对照期间未主动并发其他 Cargo 构建或产品性能测量。

修改前：

```powershell
cargo test -p stickymd-smoke --locked --test cli_exit --test package_path_wrapper --test release_wrappers -- --nocapture
```

合并后：

```powershell
cargo test -p stickymd-smoke --locked --test headless -- --nocapture
```

| 状态 | Cargo 构建阶段 | Rust test 部分 | 整条命令墙钟 |
| --- | ---: | ---: | ---: |
| 三个独立 target，合计 13 项通过 | 0.31 s | 5.50 + 5.55 + 89.02 = 100.07 s | 100.74 s |
| 合并 target，13 项通过 | 1.58 s | 94.48 s | 96.71 s |

整条命令少 4.04 s，约 4.0%；test 部分少 5.59 s，约 5.6%。收益较小，发布 wrapper
仍占主要等待时间。这是一次本机前后对照，不是统计基准，也不能推导完整验收或远程 CI
同比提速。原始日志为 ignored `target/headless-concurrency/{before,after}-integration.log`
及对应 `*-timing.json`。完整 smoke 回归中的该 target 为 94.64 s，不作为第二组独立基准。

## 未保留的收据并行试验

另一个试验把 automated readiness 的七组独立收据验证放到最多四个线程中，并保持诊断
顺序与失败语义。以同一个收据完整性测试作对照：

```powershell
cargo test -p stickymd-smoke --locked --bin stickymd-smoke source_only_headless_can_differ_from_final_exe_but_artifact_receipts_cannot -- --nocapture
```

| 状态 | Rust test 部分 | 整条命令墙钟 |
| --- | ---: | ---: |
| 原有串行收据检查，1 项通过 | 22.21 s | 24.16 s |
| 最多四组并行，1 项通过 | 30.23 s | 32.66 s |

此次试验没有显示加速，因此撤回该实现与新增的并发测试，保留原有串行验证及全部负向
用例。进程启动或资源竞争可能影响结果，但本轮未确定回退原因。试验日志保存在
`target/headless-concurrency/{before,after}-readiness.log`；被撤回的实现仅保留为 ignored
`readiness-parallel-rejected.patch`，不进入提交。后续若继续处理该热点，应先定位重复工作，
本轮不增加线程数或通用调度器。

## 验证与限制

- `cargo test -p stickymd-smoke --locked`：232 项单元测试和 13 项集成测试通过，
  1 项原有显式性能剖析测试保持 ignored；本次整条命令为 180.79 s，无全量前后基准。
- 完整回归之后，补齐 CLI 规划用例的 Git 索引只读隔离；该用例以完整名称和 `--exact`
  定向复测通过。最后的 `cargo fmt --all --check` 与 strict smoke Clippy
  （`--all-targets --locked -- -D warnings`）通过。
- `cargo test -p stickymd-smoke --locked --tests -- --list` 对照去除新增的三个模块名前缀后，
  修改前后均为 246 项（含 ignored），缺失、增加和重复均为 0。最终结果保存在
  `target/headless-concurrency/final-inventory-comparison.json`；早期试验的
  `inventory-comparison.json` 包含已撤回的新测试，不代表最终清单。
- 完整回归、最后的只读隔离定向复测及清单分别保存在同目录的 `final-smoke.log`、
  `final-git-isolation.log`、`final-inventory.log`，墙钟记录为 `final-smoke-timing.json`。

本轮没有运行产品 workspace 全量、Linux 实机执行、远程 CI、原生 GUI、startup 或资源
资格化。非 Windows 模块条件保持原样，Linux 专属 unsupported-GUI 用例仍在源码中，
其实际执行结果未在本机验证。本轮不改变 plan 骨架、发布权限、验收阈值或样本数。

## Resolution（2026-09-29）

文档落盘后，`cargo run --quiet -p stickymd-smoke --locked -- phase 00 --json` 的
`governance contracts` 与 `acceptance readiness` 均通过；结果保存在
`target/headless-concurrency/governance.json`，`git diff --check` 通过。
这项源码治理检查不代表产品 readiness 或候选资格化。
