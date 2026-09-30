# 2026-09-30 包内容与 Syft 缓存规则 Rust 化

本轮已将包内容清单、README 生成、staging 以及 Syft 缓存身份与校验迁入既有 std-only
`stickymd-smoke`。主要收益是生成/验证规则一致、错误路径可独立测试。本轮没有测量性能收益。
PowerShell 继续保留 ZIP、下载、Syft 调用及 Windows 适配；没有改动产品、依赖政策、发布权限或人工状态。

## 背景、授权与契约

用户明确要求在上一轮基础上按包内容/README、Syft 缓存的顺序推进。开始时 HEAD 为
`c5c7df245c60aa9a31626714feeeeefcc75a8a5d`，工作树包含上一轮未提交的发布 CLI 复用改动；
已核对并保留全部既有改动。适用 AGENTS、工程宪法、术语表、plan 11 与 Phase 14 投影已重读，
结构查询优先使用已有 CodeGraph 索引。

权威仍是 [phase-verification-harness](../plan/11_testing_and_release.md#phase-verification-harness)、
[release-artifact-authority](../plan/11_testing_and_release.md#release-artifact-authority) 和
[portable-windows-runtime](../plan/11_testing_and_release.md#portable-windows-runtime)。
产品投影仍是解压即用、不预建用户 note、单目录身份；无需改 plan 或产品 feature。

原问题：PowerShell 生成端、Rust ZIP allowlist 与 SBOM 覆盖各自列举成员，README 模板还在脚本中；
Syft 的版本、两个摘要、缓存判断和上游 manifest 绑定也由 PowerShell 持有，难以独立验证错误路径。

## 实现与规则归属

| 范围 | 权威实现 | PowerShell 保留职责 |
| --- | --- | --- |
| 六个包成员、来源、ZIP 顺序、四个 SBOM 必需成员 | `release/package_content.rs`；验证器与生成器消费同一清单 | 按返回列表压缩，保留固定时间戳与现有 ZIP 发布行为 |
| README 与许可证文本 | `package_content.rs` + `package_readme.txt`；已验证 source state 决定标题；许可证复用 notices 解码 | 转发既有输出 |
| staging | `package_staging.rs`，复用 package input/source 规则和原子写入 | 分配私有临时父目录及最终清理 |
| Syft pin/文件名/URL/缓存命中 | `release/syft/mod.rs`、`cache.rs`，复用 `integrity` | 网络请求、重试等待 |
| 下载校验、缓存替换和解压输入 | Rust 校验私有快照，原子替换缓存；校验两份 pin 与唯一 manifest 条目后返回独立快照 | 解压已验证快照、调用 Syft、恢复环境 |

新增内部可复用 CLI：`prepare-package`、`syft-plan`、`syft-publish`、`syft-verify`；
既有 CLI 和 PowerShell 参数接口保留。`package-inputs` 仍输出原 `NOT_RUN` JSON，prepare 输出中的
该字段仍表示包验证/资格化未运行。没有候选晋升或资格化收据写入。

README 保留三种标题、完整 source SHA、版本、unsigned 警告及原文；输出 UTF-8 无 BOM、CRLF。
许可证文本输出 LF，复用已有严格 BOM/UTF-8/UTF-16/UTF-32 解码器，缺失或非法编码拒绝。
staging 只接受不存在的目标目录，父目录必须存在；失败清理本次创建的部分内容，不清理既有目录。
ZIP 与 SBOM 的原有验证错误顺序也保留。

Syft 继续使用 1.50.0 和原有两个 SHA-256 pin，没有更新工具或允许调用者覆盖 pin。
缓存目录名不证明有效性，文件 hash 必须匹配；损坏或缺失只产生待获取计划。
下载失败或校验失败不替换已有缓存。上游 manifest 的文本分隔符规范化后复用现有严格名称、
digest 和重复拒绝规则。解压使用重新验证的私有副本，后续共享缓存变化不能改变该输入。
下载仍最多三次、间隔 1/2 秒；离线测试替换等待，不实际联网。

`-SyftPath` 保持调用者提供工具的既有 bypass：检查文件存在，不额外认证它的版本/hash。
`syft-plan` 明确返回 `external: true`；历史 `SYFT_VERSION=1.50.0` 输出仍表示配置 pin，
不能将它当成外部工具认证结果。

移除治理中依赖 `generate-third-party-notices.ps1` 源码字面量的断言，改由实际打包、成员字节、
notices 校验及失败行为证明；没有以新的 PowerShell 函数名或关键字断言替代旧断言。

## 同输入基线与数据

本轮先用锁定依赖构建 `stickymd-win --release`，用迁移前工具在隔离目录生成新包；
由于保留上一轮未提交改动，明确使用 `-AllowDirtyValidation`，结果为 `DIRTY_VALIDATION`。
未复用旧发布 artifact 或正式收据。

本地 ignored 材料位于 `target/release-staging-dca6cd84/`，包含迁移前脚本/CLI 副本、初始状态、
`before 中文 space/`、`after 中文 space/` 和运行日志。它们仅为本轮诊断材料。

同一 EXE、版本、source SHA、许可证和锁图在迁移前后生成的 ZIP 完全相同：

```text
StickyMD-0.1.1-local-validation-c5c7df245c60-dirty-windows-x64-portable.zip
SHA256 63d53b0ddaaaf71d86526571235472688861ae6d61646beac0ea16f845781aef
```

真实缓存 Syft 在迁移前后分别生成 SBOM，两次都通过校验。两份 SPDX JSON 只统一
`documentNamespace` 与 `creationInfo.created` 后逐对象序列化比较，结果一致；没有声称 SBOM 字节相同。
迁移后本地 SBOM SHA-256 为 `02e8a2522aacd3b85d581119ee17eb22f60de6e1db40ec8eeaac33bf0a5e892c`。
实际新包静态验包通过，notices 仍含 187 个 runtime dependencies。

## 定向验证与故障修正

- 包迁移后 release unit cases：23 passed；随后增加许可证和 Syft 测试后为 27 passed，两个 opt-in diagnostics 默认忽略。
- 包阶段 PowerShell 5.1/7：两版通过。实际 ZIP 成员和顺序、固定时间戳、README 身份/换行、许可证编码、
  同输入重复生成、不同既有包拒绝覆盖、CWD/编码恢复均已检查。
- 首次扩展包装测试误把 PowerShell 自身 throw 等同于原生命令 `$LASTEXITCODE`，导致两个案例失败；
  修正测试为检查原有拒绝异常和文件保留，重跑两版通过，没有改变既有脚本错误接口。
- Syft unit cases 覆盖 cache miss/hit/corruption、两个 hash 的角色绑定、上游缺失/重复/危险名称、
  错误 hash、私有快照隔离及失败清理、既有目录保护、外部工具覆盖。
- Syft PowerShell 5.1/7：两版通过。实际 Rust pin 校验拒绝 mock 下载的坏字节；下载中断/坏摘要均限制三次，
  保留旧 cache、删除 partial 文件、恢复目录与编码；缺失外部工具和不可验证快照返回非零。
- 实际 Syft cache hit、生成 SBOM 和完整静态包检查通过；没有网络下载。
- 严格 Clippy 已通过；最后的全 crate tests、fmt 与串行 runtime 检查结果追加于 Resolution。

## 影响、可选路径与限制

选择复用现有 Rust CLI 和哈希/原子写入能力，保留 ZIP、Syft 传输/执行及 UIA/COM 平台适配。
没有新增依赖，也没有引入 ZIP 解码器、Rust 网络客户端或新的依赖信任政策。
无需改变 Source Freeze、Promoted Candidate、release readiness 或人工门。

本轮未测性能，不能沿用上一轮内部验包调用的 26.5% 数字作为这些改动的收益。
未验证真实网络重试、远程 CI、远程晋升或 Linux；未运行完整资源/性能/人工 Campaign。
实际下载成功路径由缓存发布 unit tests 与已缓存的真实 Syft 验证共同覆盖，不能称为在线下载验收。
没有提交、推送、触发远程工作流或发布。

## Resolution：最终检查

`cargo test -p stickymd-smoke --locked` 的单元测试为 281 passed、0 failed、8 ignored；
集成测试首次为 19 passed、1 failed。失败来自治理代码仍搜索 PowerShell 中的 Syft 版本和
两个 hash 字面量，并非包或缓存行为失败。移除该源码关键字断言，规则由 `release::syft`
单元测试、双 PowerShell host 的实际坏下载/缓存保护测试及本轮真实缓存验证覆盖。
没有改成搜索 Rust 源码中的相同字面量。

修正后运行
`cargo test -p stickymd-smoke --locked --test headless cli_exit::successful_json_request_returns_zero_and_writes_one_json_document -- --exact --nocapture`，
结果为 1 passed；结合首次运行，其余 19 个集成案例均已通过，包括 PowerShell 5.1/7。
最终 `cargo fmt --all --check`、
`cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings` 和 `git diff --check` 均通过。

对本轮新生成的隔离包串行执行 opt-in
`release::package::diagnostics::direct_package_runtime_preserves_output_and_cleanup`，
结果为 1 passed，覆盖 ASCII、空格、中文路径，同目录第二实例退出、不同目录独立运行及清理。
执行前后没有遗留 StickyMD 进程。默认忽略的其余诊断没有运行，不把该启动检查视为人工验收。
日志保留在上述 ignored 目录，包括 `final-smoke-tests.log` 和 `staged-package-runtime.log`。

包生成、包有效性、候选身份和发布资格仍然独立；此次结果只证明本轮工具迁移的已列自动化行为。
产品 readiness 与人工未验收项未改变，性能、在线下载及远程流程仍保持前述未验证状态。
