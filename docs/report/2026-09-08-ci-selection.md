# 2026-09-08 模块化 CI 实施记录

基线：`codex/modular-ci-tooling`，HEAD `c5300ab11ac64370c308aa3464f6e2f11376b2b7`，
本地 `origin/main` 为 `37fa6e0b6da3ae83c16bb29a42b605ad078ac3c2`。
工作树包含此前运行时审计改动；本次没有覆盖这些改动。

## 已核实事实：背景、批准与实现

原日常 CI 每次执行完整集合，缺少工程模块选择；原有 tests/performance 分片仅划分任务类型。
USER 已批准[影响分析](RISK-2026-09-08-modular-ci.md)的推荐方案；当前 authority 已更新为
[plan 11 的模块化 headless CI 合同](../plan/11_testing_and_release.md#modular-headless-ci)。
前置的包路径 Rust 迁移及显式模块入口见[开发工具维护记录](2026-09-08-modular-tooling.md)。

本次实现如下：

| 责任 | 实现与边界 |
| --- | --- |
| 模块定义 | `headless.rs` 持有六个 Cargo 模块及直接依赖；既有 runner 持有完整任务图，模块入口只投影包选择 |
| Git 输入 | `ci/git.rs` 验证完整 SHA、对象与工作树，使用 NUL 路径传输；重命名按删除和新增保留两端归属 |
| 选择 | `ci/selection.rs` 归属变更并计算反向依赖闭包；core 覆盖 core/render/windows，render 覆盖 render/windows |
| 漂移检测 | `ci/registry.rs` 对照 Cargo 的 workspace、两个实验及其直接依赖，不自行解析 Cargo manifest |
| 工作流输入 | `ci/projection.rs` 输出范围、原因、source/base、矩阵与适用检查参数，状态始终为 `NOT_RUN` |
| 聚合 | `ci/results.rs` 按同一检查范围验证成功/跳过；失败、取消、缺项、未知状态和意外跳过返回非零 |
| GitHub 适配 | workflow 提供事件事实、独立 runner、缓存与结果转发；没有第二套路径分类规则 |

未知路径、共享构建/合同/CI 输入、无效或缺失比较基线、Git 比较失败、非 UTF-8 路径、
脏工作树和注册表漂移均请求原完整检查。纯说明文档保留 fmt 和治理检查。
跨 Cargo 图嵌入的 `rendering-stress.md` 作为共享输入处理，避免漏掉 smoke/G5 的使用方。
部分模块执行适用的 tests、既有 Release performance、Clippy、Linux portability、依赖政策及
Windows Release/native-runtime 检查。完整回退沿用原 tests/performance 两个分片。

手动与定时 CI 保留全量，定时工作流复用同一提交中的 CI 定义。Release/promotion 工作流及
既有 artifact gate 未修改。缓存只复用 Cargo 下载和构建产物，不把缓存命中当作测试成功；
部分 CI 不写候选资格化账本，不生成 exact-artifact 或人工验收证据。

## 复现、失败路径与已尝试方案

- 集成执行 Phase 00 时，旧治理校验将同仓库可复用工作流视为未固定 SHA，实际失败。
  同时确认旧校验只识别 `uses:`，漏检 `- uses:`。新 `governance/actions.rs` 覆盖两种形式，
  外部引用继续要求完整 SHA；本地引用必须解析到当前 checkout 的 `.github` 内有效定义，
  拒绝越界、缺失路径及伪造 revision。两个回归测试与真实治理入口均通过。
- 初稿把 `cancelled()` 写在环境表达式中，actionlint 拒绝该位置。改为条件步骤采集取消事实，
  再由 Rust 聚合器判断；修订后的工作流静态检查通过。
- 规划前的 live registry 单测可能先于保守回退中止工作流，因此公共规划门只执行 fmt/治理；
  注册表事实校验在 Rust 选测阶段负责回退。规划器回归随 smoke 测试运行，规划器变更触发全量。
- 模块投影测试通过独立的串行性能命令验证参数保留，不依赖旧运行时审计中尚未提交的参数改动。

## 实际验证数据

- `cargo test -p stickymd-smoke --locked`：144 个单元测试、6 个 CLI 集成测试、1 个
  Windows PowerShell 兼容性测试全部通过。随后只调整模块投影测试的输入隔离，重跑该测试通过。
- 覆盖真实临时 Git 仓库的新增、删除、跨模块重命名、中文路径和脏工作树；覆盖模块反向依赖、
  未知输入全量回退、当前 Cargo 图核验、空矩阵、完整任务并集及失败/取消聚合。
- 已在当前脏工作树实际运行基线规划，输出 `full=true` 和脏工作树原因，没有执行完整测试。
- 实际执行工作流中选测步骤的 PowerShell 脚本体，读取真实 Rust JSON 并写入隔离的本地输出文件；
  十个输出字段、六模块、两个全量分片及 lint/portable 参数往返检查通过。这是本地适配验证。
- fmt、smoke 的 all-targets Clippy（warnings denied）、Phase 00 治理和 diff 空白检查通过。
- actionlint 对 `ci.yml`、`scheduled.yml` 的检查通过；缓存 Action 已核对固定提交。

## 基于证据的推断与可选后续路径

模块任务并集测试证明选测沿用完整入口的命令，六节点闭包计算无需额外图依赖或后台线程。
减少无关模块的执行应能缩短部分改动的等待，但净收益受 runner 排队、缓存与依赖编译影响，
不能从本地任务数量推算实际节约比例。工具不进入产品 executable，本次不直接降低产品内存。

可在后续已授权的正常 push/PR 后比较纯文档、单模块和全量运行的 workflow 耗时与状态；
如需回滚选测，可恢复对完整入口的调用，包路径迁移与模块显式入口不依赖 GitHub 选测。

## 尚未验证与操作边界

未执行远程 CI，未测量远程耗时或缓存收益。未运行完整 Phase Campaign、GUI/IME、
人工视觉验收、产品性能或内存测量。本次没有 push、workflow dispatch、tag、发布或修改
GitHub 分支规则，也没有创建 Source Freeze / Promoted Candidate。
这些结果不归属于已发布的 `v0.1.0` exact source `64690ab8f86f63f3cbfeabbb0961276978c8f26d`。

## 外部行为依据

- GitHub [任务依赖与 always 聚合语义](https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/use-jobs)。
- GitHub [同仓库可复用工作流](https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows)。
- GitHub [工作流语法与条件求值](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax)。
- 官方 [actions/cache 固定提交](https://github.com/actions/cache/tree/9255dc7a253b0ccc959486e2bca901246202afeb)。
