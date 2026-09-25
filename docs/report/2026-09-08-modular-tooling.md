# 2026-09-08 开发工具模块化维护记录

基线：`c5300ab11ac64370c308aa3464f6e2f11376b2b7`。
分支：`codex/modular-ci-tooling`。工作树保留此前运行时审计的未提交改动。

## 已核实事实

本次完成现行 plan 11 允许的两个独立切片；不改变产品运行时、发布资产或候选资格化账本。

1. 包路径选择从 `tools/release/package-path.ps1` 迁入 std-only Rust CLI。
   单包沿用旧选择行为，多包按当前版本、完整 SHA 与 clean/dirty 身份选择；找不到匹配时失败。
   共享 Git/版本读取从 qualification receipt 提取到 `repository.rs`，原调用方保留相同接口。
   PowerShell 仍持有 Windows 调用适配，ZIP/SBOM/UIA 的成熟适配没有整体迁移。
2. 增加 `modules list` 和 `modules run <模块列表|all> [--mode=tests|performance|all] [--plan]`。
   模块为 core、render、windows、smoke、两个 Phase 01 实验。
   默认只执行所选模块的 headless tests；performance 沿用既有 Release 命令与阈值。
   多选共享命令只运行一次；Cargo 自行构建依赖。显式入口不自动选入反向依赖。
3. 模块计划从现有完整入口投影，只缩小 Cargo 包选择；所有模块并集与完整任务集合一致。
   Cargo 提供真实 workspace membership，未知成员或任务目标会阻止模块运行，避免静默漏测。
   计划 JSON 明确标为 `NOT_RUN`；实际运行失败返回非零并停止，成功只证明请求范围。
   不写候选 receipt、不写 last-success ledger，也不把旧 EXE 的散列绑定给当前模块计划。

## 兼容性问题、复现与修复

- 迁移初稿在 Windows PowerShell 5.1、代码页 936、中文目录下无法往返 Rust 的 UTF-8 路径。
  用真实子进程复现失败后，包装器在调用期间设置 UTF-8，结束时恢复原编码。
- 迁移初稿用 .NET `GetFullPath` 解析 `.`，在 PowerShell 已切换目录时指向错误位置。
  回归测试先失败，再改为 PowerShell session path resolver，保持旧函数的相对路径语义。
- 回归覆盖中文、空格、方括号、相对路径，以及成功/失败后的目录和编码恢复。

## 实际验证

- 先运行包路径 5 项定向测试：通过。
- 执行 `modules run smoke`：127 个单元测试、2 个 CLI 集成测试及 1 个 PowerShell 兼容性测试通过；
  同时实际执行了新入口的治理检查与目标模块调度。
- 随后补充进程级模块计划/未知模块测试，重新执行 CLI 集成测试：4 项通过。
- 补充相对路径用例后，PowerShell 兼容性测试先失败，修复后通过。
  最终覆盖为 127 个单元测试 + 4 个 CLI 集成测试 + 1 个 PowerShell 兼容性测试。
- 单元测试验证完整计划及 tests/performance 分片不变、模块覆盖无重复无遗漏、Release
  过滤器/串行参数保留、共享命令去重、无效参数和未知 workspace 成员拒绝。
- fmt、`cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings`、Phase 00 治理通过。
- 用 PowerShell 的 JSON 解析器读取全部六模块计划，验证合法 JSON 与 `NOT_RUN` 状态。
- 已实际比较旧 PowerShell 助手与 Rust 包选择的单包、多包返回值；包含带空格目录，结果一致。

## 推断与未验证事项

按模块运行可以减少不相关的测试命令；实际 GitHub 耗时节约仍需远程 workflow 数据。
工具不进入产品 executable，因此此项迁移不直接降低 StickyMD 的运行时内存。

没有修改 GitHub workflow 或现行全量 CI 要求。自动根据变更选择模块及反向依赖，仍等待
[CI 合同影响分析](RISK-2026-09-08-modular-ci.md)中提出的具体规则确认。
没有运行完整 Phase Campaign、GUI/IME/物理多屏验收、产品性能/内存测量或远程 CI；
没有建立新 Source Freeze / Promoted Candidate，也没有 push、tag 或发布。

## 对规格的影响及后续路径

已完成部分属于既有合同内的开发工具维护，现有阶段入口及完整 `all --ci` 保持兼容。
若批准日常 CI 选测规则，再更新 plan 11 与对应投影，实施变更归属、反向依赖闭包和
GitHub 调度；未知输入/基线缺失回退全量，手动/定时完整检查和发布证据门继续保留。

## Resolution — 2026-09-08

USER 已批准模块化 CI 推荐方案，plan 11 与相应投影已更新，变更选择、反向依赖、
全量回退和 GitHub 调度适配已在本地实现并验证。后续事实见
[模块化 CI 实施记录](2026-09-08-ci-selection.md)。上文等待批准与尚未修改 CI 的描述
保留为前一实施切片的历史状态，不代表当前实现。远程运行及耗时收益仍未验证。
