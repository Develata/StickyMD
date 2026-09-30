# 2026-09-30 发布工具落盘、阶段路由与远程观察规则收尾

本轮将完成 ZIP 的最终落盘、Phase 12–14 参数组合与路由、远程 tag/草稿观察校验迁入现有
std-only `stickymd-smoke`，并组合 SBOM 准备调用。主要收益是共享规则和独立失败测试；
本机缓存命中 SBOM 入口的五组交替计时中位数减少约 22.4%，不代表其他路径或 CI 的性能。

## 背景、授权与契约

用户明确同意依序完成 ZIP 落盘、阶段参数路由、发布工作流 tag/草稿状态校验与 SBOM 调用优化。
开始时工作树干净，HEAD 为 `fc233ae224a016b17cecf9d9c1e59837646eef5c`。
最近适用 AGENTS、宪法、术语、plan 11、first-run feature 与对应验收投影已核对；结构查询使用
既有 CodeGraph。工作区的本轮改动均保留，没有自动提交、推送、远程触发或发布。

权威是 [phase-verification-harness](../plan/11_testing_and_release.md#phase-verification-harness) 与
[release-artifact-authority](../plan/11_testing_and_release.md#release-artifact-authority)。
没有修改 plan、产品 runtime、依赖政策、Source Freeze/Promoted Candidate 身份、发布权限或人工边界。

原问题：ZIP 完成后的相同/冲突判断仍在 PowerShell，Phase 12–14 重复维护动作互斥和命令映射，
工作流重复比较 tag SHA/草稿状态且可能把查询失败当作不存在，缓存命中 SBOM 自动选包路径仍需
五次独立 Cargo/CLI 启动。

## 实现及保留边界

| 范围 | Rust 权威实现 | 保留适配 |
| --- | --- | --- |
| ZIP 最终落盘 | `release/package_publish.rs`，复用 `checksums`、`integrity`、`atomic_evidence` | PowerShell 创建 ZIP、固定时间戳、清理本次临时输入、转发既有成功输出 |
| 阶段参数 | `phase_entry/` 组合/路由/defaults，复用 canonical CLI parsers | 原参数名称、类型、顺序与阶段入口保留；共享 `invoke-phase.ps1` 绑定参数、恢复 CWD/编码、转发退出码 |
| tag/草稿观察 | `release/remote_state.rs` 校验完整来源、ref/tag、草稿布尔值和 HTTP/GraphQL 成功状态 | `github-observation.ps1` 只读查询；工作流保留全部 GitHub 写操作和 tag/draft/publish 权限分段 |
| SBOM 准备 | `release/sbom_preparation.rs` 组合 workspace version、已有包选择和 Syft plan/快照 | PowerShell 仍下载、解压及执行 Syft；最终输出继续调用已有 `publish-sbom` |

ZIP 先制作私有完整快照；已有相同 hash 接受，不同 hash 拒绝。原子 no-replace 避免并发覆盖，
并发相同赢家可接受、不同赢家拒绝。manifest 最后独立原子写入，失败保留完整 ZIP 与旧 manifest，
返回非零、不输出成功或资格化收据。调用者输入不删除。它不是多文件事务；后续验包仍检查 manifest。
拒绝文字保留在 Rust stderr；PowerShell 通过既有通用 release-tool 异常传递失败，不承诺旧异常栈逐字相同。

阶段路由仅扩展 CLI 适配层，不把规则放进 runner。`phase-entry-plan` 经同一规范解析后输出
`NOT_RUN`/arguments，不运行资格化。显式 false、数值零、文本 `false`/`0`、参数大小写与默认值
区分处理。Phase 12/13 的遗留 `-Candidate` 原来已到达不支持的命令，本轮保留拒绝，不擅自晋升为 Freeze。

远程观察使用 `gh api --include`：tag 为 REST ref，草稿为 GraphQL pending tag 查询。
选择后者依据 [GitHub CLI 官方 FetchRelease 实现](https://github.com/cli/cli/blob/trunk/pkg/cmd/release/shared/fetch.go)：
它为草稿另走 GraphQL，不能只靠 REST releases/tags 查询。CLI 无网络和写权限，允许缺失必须显式指定；
tag HTTP 404 或成功 GraphQL 的 release null 才可返回 exists=false，repository null、403/5xx、
传输失败、GraphQL errors 和错误字段均拒绝。观察不构成授权，也不提供查询到写入之间的远程事务保证。
旧 Source Freeze 不会自动获得这些新命令；工作流仍必须 checkout 包含对应工具的获准源码。

`prepare-sbom` 复用现有规则，不增加 pin 或缓存旁路。cache hit 同一次请求生成验证快照；
miss 沿用下载、缓存发布与 syft-verify；外部 Syft 保留原 bypass。自动选包 cache hit 从五次调用降为两次，
显式 ZIP 从四次降为两次。每次仍 `cargo run --locked`，没有另建可能过期的工具二进制缓存。
包路径选择、包验证、候选身份和发布资格继续独立。

## 基线与实际数据

ignored 证据在 `target/release-followup-20260930/`，含迁移前脚本副本、同输入新包、SPDX、
命令日志、阶段映射基线、计时脚本和 CSV。本轮先以锁定依赖构建 Release EXE，再用旧入口在隔离
中文/空格目录生成 `CLEAN_PREFLIGHT`；改动后使用 `-AllowDirtyValidation` 生成 `DIRTY_VALIDATION`。
产物文件名随工作树状态变化，包内容相同；未使用历史公开资产或旧正式收据证明新实现。

同一 EXE、来源、版本、许可证和锁图的 ZIP 字节一致：

```text
SHA256 217388f56a727ec2a55c51f62d96cf6e79b3f39ed495cb61a9bfc81bbdeea4da
```

新包静态验证通过，notices 仍含 187 项运行时依赖。
新旧入口使用真实已缓存 Syft 1.50.0 分别生成 SPDX。仅统一 `documentNamespace` 与
`creationInfo.created` 后逐对象序列化结果一致，没有声称原始 SPDX 字节一致。

计时为本机 PowerShell 7.6.5，真实缓存、相同 ZIP 与锁图、每次新私有 context，
保持 Cargo freshness 检查；首对预热不计入，随后五对交替顺序，没有并发构建或 GUI 检查。
该计时覆盖完整 generate-sbom 入口及 Syft，未包含外部 PowerShell 进程启动。

| 指标 | 原入口 | 新入口 |
| --- | --- | --- |
| 每次 CLI 调用 | 5 | 2 |
| 中位耗时 | 4.334816 s | 3.365203 s |
| 最小/最大 | 4.183143 / 4.776237 s | 3.295567 / 4.037969 s |
| 样本数（预热后） | 5 | 5 |

中位数差为 0.969613 秒（22.368%）。这是小样本本地诊断，不外推到冷缓存、下载、CI、ZIP 或产品启动性能。

## 验证、发现与修正

- 迁移前 `release::` 基线 27 passed、2 ignored；ZIP 迁移后为 30 passed、2 ignored。
- ZIP 重复/冲突、并发相同/不同输入、路径别名、缺失输入、Windows manifest 文件锁覆盖通过。
  新用例发现 Windows absolute normalization 会吞掉末尾点号，已将共享名称检查移到规范化之前。
- 原 Phase 12–14 脚本与新路由在同一组参数下分别运行，PowerShell 5.1/7 均通过。测试基线初次
  额外启用 StrictMode 时触发旧脚本 null.Count 问题；改为按旧脚本本来未启用 StrictMode 的条件
  建立基线。新入口仍在 StrictMode 下测试并通过；无源码迁就旧运行报告。
- 阶段实际 wrapper 测试调用编译后的只读 Rust 路由，覆盖中文/空格、显式 false、零、common 参数、
  action 映射、诊断互斥、失败非零与调用者状态恢复。
- 远程纯规则测试 3 passed。双 PowerShell 实际 workflow step 的离线测试覆盖合法已有/新建、
  错误 SHA、已发布 Release、404/null、403、无响应和 GraphQL errors，断言拒绝后零写操作。
  首次测试的 mock script-scope 和 YAML step 提取范围错误已修正；没有实际访问或修改用户 Release。
- 双宿主 release wrapper 回归通过，实际检查 ZIP 成员、README/许可证编码、失败 SBOM 保留与
  失效缓存拒绝；最终 smoke 全套、fmt、Clippy 与串行启动检查在下方 Resolution 追加实际结果。

治理仍检查工作流权限和禁止 rebuild 等架构边界。已迁移行为由编译实现、真实 wrapper/step 和
失败结果验证，没有新增搜索 PowerShell/Rust 关键字来假装规则已执行。

## 影响、可选路径与未验证项

选择合并相关 CLI 调用，避免引入额外 compiled-tool cache 身份政策；ZIP 压缩、Syft 网络/执行、
Windows 资源及 UIA/COM 继续留在平台适配端。没有新增依赖或 runtime 网络客户端。
测试只证明列出的工具行为，人工项、完整桌面/资源/性能 Campaign、readiness 均不变。
未触发或轮询本轮远程 CI，未在线验收新 workflow、真实下载/重试及远程写操作，未在 Linux 执行。

## Resolution：2026-09-30 最终验证与边界复核

`cargo test -p stickymd-smoke --locked`：290 个单元测试通过，8 个 opt-in diagnostics 默认忽略；
集成首次为 20 passed、1 failed。失败是治理 `verify_phase_artifacts` 仍要求每份脚本包含
`stickymd-smoke`/`phase` 关键字，与共享薄适配器冲突。Phase 12–14 改由已有实际路由/拒绝测试
证明，治理仍要求入口可读取；其他未迁移阶段的旧检查未扩大改动。修正后单独复测失败案例通过，
21 个集成案例均已覆盖通过，没有用注释补入关键词来掩盖问题。

末次 review 使 workflow fixture 的 tag 随当前 workspace 版本生成；HTTP 404 还必须配合 gh
正常的错误退出码 1，进程异常退出即使已收到 404 也拒绝。相关 Rust 三个单元案例及双宿主
实际 step 集成重新通过。最终 `cargo fmt --all --check`、
`cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings` 与 `git diff --check` 通过。
新增模块各有 plan_ref，按落盘、路由、远程观察、准备编排分工，未向 runner 堆入发布规则。

Windows PowerShell 5.1 还用当前真实缓存运行最终 SBOM 入口并完成静态验包，通过；
该 SBOM SHA-256 为 `d76a49a05a2fbbeeb360eccb0a6dcecc1ceba6b21ee819df46e2d9fcad255429`。
随后对本轮 `after 中文 space/` 新包串行执行
`release::package::diagnostics::direct_package_runtime_preserves_output_and_cleanup`，1 passed，
覆盖 ASCII、空格、中文路径、同目录第二实例退出、持久文件不变、不同目录独立运行及临时清理。
运行前后均未发现 StickyMD 进程；没有输入、剪贴板、tray 或人工 Session 操作。

本轮只运行了上述 opt-in 启动检查，其余默认忽略的诊断未运行。最终日志包括
`final-smoke-tests.log`、`final-governance-recheck.log`、`final-remote-state.log`、
`final-remote-steps.log`、`final-clippy.log`、`final-static-package-ps51.log` 和 `final-package-runtime.log`。
以上结果不产生发布资格或人工通过状态；所有远程/Linux/完整 Campaign 缺口仍如前述。
