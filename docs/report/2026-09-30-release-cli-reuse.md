# 2026-09-30 发布工具 Rust 复用维护

本轮完成两项有明确收益的工具维护：Rust 内部验包直接调用现有验证器；CI 版本读取和远程
workflow 身份判断复用 Rust 规则。既有 PowerShell 参数入口保留，无新增依赖、产品 runtime
改动或发布权限变化。本报告记录本地工具验证，不是 Source Freeze、Promoted Candidate 或发布批准。

## 背景与契约

- 开始时为 `main`、`c5c7df245c60aa9a31626714feeeeefcc75a8a5d`，工作树干净。
- 已重新读取适用 AGENTS、工程宪法、术语表、plan 11 和 Phase 14 投影；有索引的结构查询使用 CodeGraph。
- 对应 [phase-verification-harness](../plan/11_testing_and_release.md#phase-verification-harness)、
  [release-artifact-authority](../plan/11_testing_and_release.md#release-artifact-authority) 和
  [portable-windows-runtime](../plan/11_testing_and_release.md#portable-windows-runtime)。
- 包选择、来源/hash/checksum、包结构/runtime、notices、SBOM 规则已经在 Rust 中；本轮没有再次迁移或复制这些规则。
- 问题在调用关系与规则复用：runner 和下载产物检查仍经过 Rust → PowerShell → Cargo → Rust；
  release workflow 单独用正则读版本，promotion workflow 用三次 `gh api` 分别判断同一 run 的三个字段。

## 实现与边界

| 已实施项目 | 单一实现与调用方 | 可核实收益 |
| --- | --- | --- |
| 验包内部调用 | `release/package.rs` 暴露 typed verifier；runner、qualification/remote 和 release CLI 共用 | 消除外层 PowerShell、Cargo 和第二个 smoke 进程；规则与 runtime 检查不变 |
| 输出流管理 | package/notices 接受 writer；`runner/package.rs` 处理诊断输出 | human CLI 保留原 `KEY=value`；JSON phase 的诊断走 stderr，stdout 保留原收据格式 |
| CI workspace 版本 | workflow 调用已有 `release workspace-version` | 与本地包输入共用作用域、空白和缺失/歧义拒绝规则 |
| workflow 观察校验 | `release/workflow.rs`；qualification 远端记录与新 `release verify-workflow` 共用谓词 | 完整匹配 SHA、success conclusion、release 名称只定义一次；publish 步骤只查询一次 |

`verify-workflow --source-sha <full-sha> --workflow-json <UTF-8 file or ->` 仅消费观察值；`-` 读 stdin。
缺失/重复/类型错误 JSON 字段、非完整/不匹配 SHA、失败或取消的 run、错误 workflow 均拒绝。
成功只输出 `WORKFLOW_IDENTITY=PASS`，不访问网络、不写收据、不代表 artifact 验证或完整资格化。
资格化仍先检查 Source Freeze、PUSH authority、upstream，再查询 run/artifact；下载包通过全部检查后
才进入原有 promotion/收据路径。tag、draft、publish 的独立授权及其他校验保持不变。

保留所有阶段入口和 release PowerShell 参数接口。ZIP 压缩解压、原生资源读取、Syft 获取/调用、
UIA/COM 继续作为适配层；包 staging/README 仍在 `package.ps1`。本轮不引入 ZIP crate，std-only 保持。
原依赖 workflow URL 字面量的治理断言被移除；实际 workflow step 的离线行为测试验证查询、参数和失败传播。
CLI 说明、REL-CLI-11/12 与覆盖映射同步更新；人工项状态不变。

## 同输入基线与集成验证

修改前构建锁定的 smoke CLI 和 `stickymd-win --release`，用当时干净工作树生成本轮独立
`CLEAN_PREFLIGHT` ZIP，并用缓存的 Syft 1.50.0 实际生成 SBOM。没有复用旧发布包或资格化收据。

本地日志/输入根目录：`target/release-cli-20260930-705db580/`，包目录为 `package 中文 space/`。
这些是 ignored 本地诊断材料，不是仓库长期证据通道。

| 文件 | SHA-256 |
| --- | --- |
| `StickyMD-0.1.1-local-rc-c5c7df245c60-windows-x64-portable.zip` | `63d53b0ddaaaf71d86526571235472688861ae6d61646beac0ea16f845781aef` |
| `SBOM.spdx.json` | `fce977bf4c35c50533335f0a8f4a5911323c8d1b58993ea8e8624292601df6db` |
| `SHA256SUMS.txt` | `ade915655c2fca13e982ce85af399ec504a40f329595d084fc827dd7a2d0b8c7` |

迁移前/后 CLI 对同一输入均通过，187 个 runtime dependencies。除私有临时 notices 路径外，
输出逐行相同，manifest 摘要不变。计时诊断还逐轮比较 direct 与原 wrapper 调用路径的稳定输出。
该 ZIP 只验证本轮工具，不能作为含本轮工具修改的新 source artifact。

串行执行 `direct_package_runtime_preserves_output_and_cleanup`：运行前未发现 StickyMD 进程；
ASCII、空格、中文目录启动，同目录第二实例退出且 durable files 不变，不同目录实例独立存活，
临时快照清理均通过。没有发送键盘、鼠标、托盘或剪贴板操作；结束后未发现 StickyMD 遗留进程。
未运行完整资源、性能或人工验收 Campaign。

## 本地耗时

Windows、Rust 1.97.1；使用同一 package、同一修改后 Rust 验证实现，比较进程内 API 与现有
PowerShell/Cargo 入口。每种路径先预热一次，随后各 5 次，逐轮交换顺序；关闭 runtime 检查。
测量时本任务没有并行构建或其他测试；机器并未全局独占，后台负载仍可能影响耗时。

| 路径 | 5 次原始耗时（ms） | 中位数（ms） |
| --- | --- | --- |
| Rust 进程内 | 3345.9178, 3406.3576, 3691.8959, 3738.9150, 3328.2965 | 3406.3576 |
| PowerShell/Cargo 入口 | 4373.0672, 4618.3164, 4949.4346, 5382.8489, 4634.5545 | 4634.5545 |

两组中位数相差 1228.1969 ms，即本样本静态验包约减少 26.5%。ZIP/资源适配、文件哈希、
锁定 Cargo metadata 仍包含在测量中；结果不能推算产品启动、完整 qualification 或远程 CI 提速。
workflow 查询由三次变一次是已验证调用次数，不是网络耗时测量。

复现诊断：先按上述步骤新建隔离包，设置 `STICKYMD_PACKAGE_DIAGNOSTIC_DIRECTORY`，
构建 `cargo test -p stickymd-smoke --locked --bin stickymd-smoke --no-run`，直接运行输出的 test binary：

```text
--ignored --exact release::package::diagnostics::compare_package_dispatch_paths --nocapture --test-threads=1
--ignored --exact release::package::diagnostics::direct_package_runtime_preserves_output_and_cleanup --nocapture --test-threads=1
```

两项必须分别运行；runtime 项需要交互式 Windows 桌面，不能与其他 GUI 检查并发。

## 检查记录

| 检查 | 本报告创建时结果 |
| --- | --- |
| 迁移前 release unit tests | 18 passed |
| 迁移后 targeted release unit tests | 20 passed；2 个 opt-in diagnostics 单独执行通过 |
| `headless release_workflow::` | 3 passed；真实 workflow step 使用离线 GitHub 观察和编译后 CLI |
| `cargo test -p stickymd-smoke --locked` | 274 unit passed、8 ignored；19 integration passed，1 个治理 JSON 案例因本报告尚未落盘而断链失败，落盘后需重跑 |
| PowerShell 5.1/7 薄入口 | 两版通过；Unicode/空格路径、非零失败、CWD、编码与环境恢复；包括新增 workflow 命令 |
| 真实 workflow step 离线执行 | 两版 PowerShell 下分别覆盖成功、身份失败、`gh` 返回 23；一次查询，查询失败不调用 Rust，非零正确传播 |
| 隔离本地验包/runtime | 通过；同输入输出比较及清理检查见上文 |
| `cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings` | 通过 |
| `cargo fmt --all --check` | 通过 |
| `actionlint .github/workflows/release.yml .github/workflows/promote-release.yml` | 通过 |
| `git diff --check` | 通过 |

错误 SHA/来源、hash mismatch、重复 checksum、危险路径、缺失许可证及 build/dev/normal 依赖分类
继续由原有 release/integrity/notices 与双宿主行为测试覆盖。本轮新增 run 身份、无效观察文件、
stdin、取消/失败/错误 workflow、CLI 参数拒绝和实际步骤失败传播测试。

## 可选后续方向与尚未验证项

| 方向 | 判断 |
| --- | --- |
| 包内容清单与 README/staging 计划 | 后续有一致性收益：收拢生成端和验证端共享数据；本轮保留现状 |
| Syft 版本、校验与缓存选择规则 | 可进一步测试化；下载/调用仍适合保留 PowerShell 适配；本轮未改 |
| ZIP 编解码、UIA/COM 全量重写 | 本轮没有收益依据，继续保留现有平台适配 |

未触发远程 workflow，未执行真实远端下载/promote/tag/draft/publish，未测网络或 CI 总时长，未在
Linux 主机执行本轮工具测试。没有自动提交或推送；本轮不改变既有 readiness 与人工验收缺口。
promotion workflow checkout 的新 Source Freeze 必须包含 `verify-workflow`；旧发布 source
不自动获得新命令，也不得用新主线工具伪装重建旧 artifact。无需修改 plan、依赖政策或发布权限。

## Resolution — 2026-09-30

报告落盘后重跑唯一失败的
`cli_exit::successful_json_request_returns_zero_and_writes_one_json_document`，1 passed。
治理链接、默认 Phase 00 JSON 输出和零退出码均通过；没有重跑已通过的其他案例。
因此本轮累计验证 274 个 unit cases 与 20 个 integration cases 全部通过。
默认忽略的 8 项中，本轮单独执行 2 项 package diagnostics；其余 6 项资源/桌面诊断没有执行。
