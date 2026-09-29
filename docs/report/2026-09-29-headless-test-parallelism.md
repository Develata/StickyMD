# 2026-09-29 Headless 测试并行优化

本报告记录现有验证工具的并行化与本机对照，不建立产品或发布资格化结论。
检查起点为 clean `794a1a23c15dc6f2bae531f72293e78378511e72`；实现遵循
[plan 11](../plan/11_testing_and_release.md#phase-verification-harness) 的资源隔离约束与
[模块化 CI](../plan/11_testing_and_release.md#modular-headless-ci) 的 Rust 聚合规则。

## 问题与取舍

- `release_wrappers.rs` 把 PowerShell 5.1 和 7 放在同一个 test 的循环中，因此两版必定串行。
  两版执行相同的成功/失败、Unicode、打包、SBOM、checksum 与调用者状态检查。
- `ci.yml` 的 Linux smoke Clippy/tests 位于 `plan` job 末尾；所有平台 job 都 `needs: plan`，
  因而即使已算出范围，也必须等 Linux smoke 完成才能启动。
- 同一个桌面的 GUI、clipboard、tray、focus、startup 和 CPU/内存测量仍须独占。
  现有 GitHub-hosted tests/performance 分片已经使用独立 runner；本轮保持其覆盖并集与参数。
- 未采用多个本地 `cargo test` 争用同一个 target 目录，也未增加测试运行器依赖或通用调度器。
  后续若并发多个测试可执行文件，仍需先盘点写目录、命名对象和线程预算，编译由 Cargo 统一持有。

## 实现与资源边界

PowerShell 两版改为两个普通 Rust tests，由现有测试框架调度；显式 `--test-threads=1`
仍可要求串行。每版内部的两个 PowerShell fixture 继续顺序执行，保留全部既有断言。

| 资源 | 所有权与访问 |
| --- | --- |
| 仓库、预编译 smoke CLI | 两版共同读取；`GIT_OPTIONAL_LOCKS=0` 禁止 Git 查询刷新共享索引 |
| 测试输入、ZIP/SBOM/checksum 输出 | 每版独占临时目录；PID、时间戳与 edition 共同命名，时间戳相同也不冲突 |
| 环境、当前目录、控制台编码 | 仅子进程持有；移除跨版本 `PSModulePath`，以 `CREATE_NO_WINDOW` 避免共享父控制台 |
| Cargo | 包装入口的 `cargo run` 转交同一预编译 CLI；许可证生成保留既有 `cargo metadata --locked`，缓存访问由 Cargo 管理，不发起 Cargo build |
| GUI、资格化收据、便签 | 此组 wrapper 测试不使用；正式测量的独占要求保持不变 |

Windows PowerShell 5.1 仍为必需；PowerShell 7 不可启动时沿用原有 `NOT_TESTED` 诊断。
本次机器上两版都实际执行并通过，未走该缺失分支。

Linux smoke 改为 `needs: plan` 的独立 job，与其他隔离平台 job 并发。选择条件由 Rust
`Checks` 单点投影为 `smoke_needed`，只在 full 或 smoke 范围运行。聚合 CLI 新增必填
`--smoke=<status>`，失败、取消、缺少字段、未知状态和意外 skip 均拒绝；其他范围只接受
计划内 skip。新 job 使用独立 Cargo cache lane，不生成候选资格化账本。

## 本机对照

环境：Windows x64，20 个逻辑处理器，`RUST_TEST_THREADS` 未设置；Rust/Cargo 1.97.1，
Windows PowerShell 5.1.26100.9444、PowerShell 7.6.5。三次均使用：

```powershell
cargo test -p stickymd-smoke --locked --test release_wrappers -- --nocapture
```

| 状态 | Cargo 构建阶段 | Rust test 部分 | 整条命令墙钟 |
| --- | ---: | ---: | ---: |
| 修改前：同一个 test 内串行两版 | 1.51 s | 127.52 s | 129.82 s |
| 首次并行，两版均通过 | 10.81 s | 86.03 s | 99.04 s |
| 补齐 Git 索引只读隔离后的最终复测，两版均通过 | 5.06 s | 85.42 s | 91.86 s |

最终 test 部分比串行基线少 42.10 s，约 33.0%。这是一次串行基线与两次实现迭代检查，
不是统计基准；编译开销单列，不能把这个比例推广到完整验收或远程 CI。对照期间没有主动
并发运行其他 Cargo 构建或产品性能测量。原始日志和墙钟记录位于 ignored
`target/parallel-tests/{before,after,final}-release-wrappers.log` 与对应 `*-timing.json`。

CI 的结构收益是移除所有平台 job 前面的 Linux smoke 等待；增加一个隔离 runner 的启动、
checkout 与缓存开销可能影响实际收益。本轮未 push 或触发远程 CI，因此没有远程节省秒数。

## 验证与限制

- 最终两版 wrapper：2 passed；原有两份 PowerShell fixture 内容未改变。
- `cargo test -p stickymd-smoke --locked --bin stickymd-smoke --test cli_exit --test package_path_wrapper`：
  232 单元测试、10 CLI 测试、1 路径 wrapper 通过。新增覆盖包含选择范围、独立 job 接线、
  所有 job 的失败/取消/skip、缺失字段，以及编译后的 CLI 拒绝不完整结果。
- 合计 245 项通过；1 项既有、显式运行的资源规划性能剖析测试保持 ignored。
- `cargo fmt --all --check`、最终 strict Clippy（smoke 全 targets、`-D warnings`）、
  `actionlint .github/workflows/ci.yml .github/workflows/scheduled.yml` 与 `git diff --check` 通过。

本轮未重跑产品 workspace 全量测试、Linux 上的实际编译执行、原生 GUI、startup、资源矩阵或
exact-candidate 验收。没有改变硬阈值、样本数、性能测试串行参数、产品 runtime 或发布权限。
本轮只落实已批准的 headless 并发边界，不需要改变 plan 的骨架或资格化标准。

## Resolution（2026-09-29）

文档落盘后再次运行 `cargo run --quiet -p stickymd-smoke --locked -- phase 00 --json`，
`governance contracts` 与 `acceptance readiness` 两项均通过；输出保留在
`target/parallel-tests/governance.json`。它只证明当前源码的治理检查，不是产品 readiness 结论。
