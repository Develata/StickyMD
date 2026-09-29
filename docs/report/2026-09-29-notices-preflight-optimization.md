# 2026-09-29 发布声明输出预检优化

本轮起点为 clean `559d94408f2edc3f83fb70319c968aa5f81c968a`，延续
[集成测试合并维护](2026-09-29-headless-integration-consolidation.md)。本报告只记录开发工具
验证与诊断耗时，不构成产品、真实发布包或候选资格化证据。实现遵循
[plan 11](../plan/11_testing_and_release.md#phase-verification-harness)，不修改 plan 骨架。

## 定位与改动

`release::notices::generate` 原先先执行 Cargo metadata、遍历许可证并渲染声明，最后才
尝试创建输出。因此，即使目标已经存在或父目录缺失，也会完成必然白做的依赖读取。

本轮将目标绝对路径解析和只读预检提前：父目录必须存在，目标文件或目录不能已经占用，
无法读取目标属性时返回错误。检查使用 `symlink_metadata`，不把悬空链接误判为空闲路径。
预检不创建目录、文件或占位符；许可证内容和正常生成流程保持原样。

最终仍调用原有 `atomic_evidence::write_new`。它通过同目录临时文件及原子 no-replace
发布保护旧文件，预检后出现的竞争创建仍不能被覆盖。没有增加依赖、持久缓存或并发线程，
也没有改变 PowerShell wrapper 的接口、断言和用例顺序。

新增回归在没有 Cargo manifest 的隔离目录中调用实际 `generate`，分别验证已有文件、
已有目录和缺失父目录：必须先返回对应路径错误，保留原文件字节和目录，不产生临时输出。
已有原子写入并发测试继续验证竞争创建只有一个成功，以及占用临时文件不得被清理。

## 本机分项对照

先构建 smoke CLI，再从两份现有 PowerShell fixture 生成隔离脚本，仅在 `cargo` 转交
预编译 CLI 的位置加入 Stopwatch 计时。原有成功/失败、调用者状态、打包及 SBOM 断言
全部执行。两轮均由一个 Windows PowerShell 5.1 子进程运行相同场景，使用独立临时目录、
独立控制台、清理后的 `PSModulePath` 和 `GIT_OPTIONAL_LOCKS=0`；计时不包含 Cargo 构建。
这不是上一轮两版并发运行的整个 `headless` target，不能直接与其耗时比较。

| 场景 | 修改前 | 修改后 |
| --- | ---: | ---: |
| 目标 notices 已存在 | 4.533 s | 0.030 s |
| notices 父目录不存在 | 4.340 s | 0.028 s |
| 合法 notices 生成，主体未改变 | 5.263 s | 1.498 s |
| 单个 PowerShell 5.1 的完整 fixture | 84.75 s | 51.27 s |

两条无效路径已不再读取依赖，其实际计时约为 0.03 s。整套场景少了 33.48 s，但未修改的
合法生成和其他命令也明显变快，说明两轮存在运行环境或缓存等影响；本轮没有定位这些
波动的来源，不能把完整降幅归因于本次改动，也不据此宣称稳定百分比或全量验收加速。

脚本与原始日志保存在 ignored `target/acceptance-profiling/`：
`profile-release.ps1`、`{before,after}-wrapper-{stdout,stderr}.log` 和
`{before,after}-wrapper-timing.json`。stderr 的 `COMMAND_TIMING` 行对应逐次 CLI 调用；
两轮均退出 0，并包含 `RELEASE_WRAPPERS=PASS` 与 `RELEASE_OUTPUTS=PASS`。

## 验证与后续范围

- `cargo test -p stickymd-smoke --locked --bin stickymd-smoke release::notices::`：4 项通过。
- `cargo test -p stickymd-smoke --locked --bin stickymd-smoke atomic_evidence::`：3 项通过。
- `cargo test -p stickymd-smoke --locked --test headless`：13 项通过，PowerShell 5.1/7
  均实际执行；test 部分为 78.88 s，命令墙钟 81.71 s。该次用于兼容性回归，无同条件全量基线。
- 合计 20 项定向测试通过；原有 fixture 内容与完整测试入口未改变，新增 1 项单元回归。
- `cargo fmt --all --check`、strict smoke Clippy（`--all-targets --locked -- -D warnings`）
  与 `git diff --check` 通过。日志保存在上述目录的 `notices-tests.log`、`atomic-tests.log`、
  `headless.log`、`headless-timing.json` 和 `clippy.log`。

ZIP 事实采集仍有重复 PowerShell 启动，是可进一步分解的热点；合并查询前须保留 Rust
对唯一成员与来源的判定。收据校验继续使用既有新鲜输入验证，本轮没有复用旧指纹或重试
上一轮已撤回的并行方案。合法输出不使用跨调用许可证缓存，以免失去输入变化的检查。

本轮未重跑完整 smoke 单元集合、产品 workspace、Linux、远程 CI、真实 Syft 生成、GUI、
startup 或资源资格化。wrapper 使用既有 fixture EXE 与 fake Syft 验证接口及失败边界，
这些结果不等同于真实发布资产或桌面验收。

## Resolution（2026-09-29）

文档落盘后，`cargo run --quiet -p stickymd-smoke --locked -- phase 00 --json` 的
`governance contracts` 与 `acceptance readiness` 均通过。输出保存在
`target/acceptance-profiling/governance.json`；这是源码治理检查，不是产品 readiness。
