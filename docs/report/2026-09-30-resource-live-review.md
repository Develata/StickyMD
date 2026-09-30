# 诊断复用现场实测与边界修复

日期：2026-09-30。USER 要求实测、review 并修复问题，通过后允许本地提交。
本报告记录实现和测量证据，不建立新契约，不改变产品 runtime 或正式验收协议。

## Review 发现与修复

审查范围为 `1964017`、`ba9c37e`、`78c9035` 引入的跨命令等价复用、整批身份校验、
失败优先及 PowerShell 入口；对照 plan 11 的诊断身份、完整采样、失败处理和收据边界。
发现两个 P2 问题，均先用回归复现，再修复：

1. **长测中失败提示过期会把成功测量误报为失败。** `Selection::complete` 对提示重读使用
   `?`；提示在开始时有效，完成时超过 24 小时，或被损坏，都会把建议性调度状态的错误
   传播给完整新结果。现在重读失败只记录 `NOT_CLEARED` 并保留现有提示；新结果仍须先
   通过完整样本验证。当前有效且版本相同的提示仍原子清除；真正的写入失败继续报错。
   回归模拟提示在测量期间过期/损坏，另用 Windows 共享模式阻止文件替换，验证清除错误
   未被吞掉。历史或不完整观测仍不能清除提示。
2. **薄入口会静默忽略与资格化动作混用的 `-ResourceResume`。** 原先只有 Plan 和
   FailureFirst 的混用检查；`-Resources -ResourceResume -SourceFreeze` 等组合会直接
   分派资格化动作。现在在 Cargo 分派前拒绝该组合，也拒绝缺少 Resources 的 Resume。
   集成回归用 mock Cargo 覆盖 SourceFreeze、Environment、WindowStress、Campaign
   的三种诊断选项混用，以及三种合法诊断参数转发；测试没有运行实际资格化动作。

复现日志：`target/acceptance-profiling/resource-round4-expiry-before.log` 与
`resource-round4-wrapper-before.log`，均按预期失败。修复后对应的 `*-after.log` 通过；
提示回归 4 passed，入口回归 1 passed。采用局部修复，未改变缓存有效期、采样量、
硬门或身份规则；另一条可选路径是放宽提示/缓存规则，但没有必要且会模糊证据边界。

## Zoom 跨命令现场实测

在已有独立 checkout `E:\gitclone\StickyMD-resource-probe-0d89ca5` 上，先切到 clean
`6de6d481a676fa6d5552e42c9459047ca80832a6`。该 checkout 无 `dist/`；其 target junction
指向主仓库 target。两次运行之间未改变 source、EXE、harness 或测试输入，也未编译。
此对照发生在上述修复之前，JSON 均明确标记 `worktree_dirty=false`。

两次命令仅输出文件名不同：

```powershell
E:/gitclone/StickyMD/target/debug/stickymd-smoke.exe phase 14 --resources --resource-module=zoom --resource-resume --resource-failure-first --evidence-file=target/acceptance-profiling/resource-round4-live-20260930/zoom-fresh.json
E:/gitclone/StickyMD/target/debug/stickymd-smoke.exe phase 14 --resources --resource-module=zoom --resource-resume --resource-failure-first --evidence-file=target/acceptance-profiling/resource-round4-live-20260930/zoom-reused.json
```

| 路径 | 完整命令墙钟 | 组执行时间 | 观测 |
| --- | ---: | ---: | --- |
| 首次完整 Zoom | 105.341411 秒 | 101.777754 秒 | 15 次新观测 |
| 同身份诊断续跑 | 10.189901 秒 | 7.128244 秒 | 15 次历史观测 |

两次均 exit 0，桌面探针通过。本次完整命令耗时减少约 90.3%；这是一次配对实测，
不代表所有资源组或负载下的稳定收益。比对后，15 次原始观测去掉 `shared_from` 后
完全相同，硬门也完全相同；续跑所有观测都保留 `diagnostic-cache:…:group-zoom` 来源。
身份采集由首次 6 次变为续跑 4 次。FailureFirst 没有命中范围内提示，保持原顺序，
不能据此声称已经现场验证“真实失败后提前重测”。

Release EXE SHA256：
`ba77d77e3d698d31ed5754e3e5802d050e013f689364b2252944e208a1be4165`。
本次 CLI harness SHA256：
`ce2b33c86051a2af3d96e82bf7d56fdc9814dd085ae66a5fc3b6ae2bf5d55ebd`。
JSON 与同名 `.log` 位于 `target/acceptance-profiling/resource-round4-live-20260930/`。

## 修复后真实场景的等价和批量读取

新增显式 ignored 的 `native_resource_alias_and_batch_reuse`，使用现有桌面探针、
`measure_case`、身份校验、原子缓存写入和证据输出。将本轮 7 个待提交实现/回归文件
同步到上述 probe 后运行；包括 Phase 14 投影、薄入口、priority 实现/回归、resources
测试声明、新 native 测试和 cli_exit 回归。运行期间不再改变这些输入。
证据如实标记 source `6de6d48` 加待提交差异、`worktree_dirty=true`，不能冒充 clean
commit 或正式候选收据。此时尚未写入本报告和 README 补充。

```powershell
$env:STICKYMD_SMOKE_PROBE_REPOSITORY = 'E:\gitclone\StickyMD-resource-probe-0d89ca5'
cargo test -p stickymd-smoke --locked native_resource_alias_and_batch_reuse -- --ignored --nocapture --test-threads=1
```

- `preview-no-math` 和 `preview-1-math` 各完成 5 次真实内存观测，保留每次 30 秒预热。
  两个场景的新测量连同场景间身份校验/保存耗时 **314.717980 秒**；测试总计 333.50 秒。
  这两个已注册场景本来不含 CPU 采样，没有缩短其协议或使用合成样本。
- 重新打开 Store 后，两次独立读取共 **4.673061 秒**、4 次身份采集；一个批次读取
  **2.354167 秒**、2 次身份采集。返回完整结果逐项一致；本次配对减少约 49.6%。
- 再读取 `preview-no-images`，通过严格等价关系复用 `preview-no-math` 的 5 次观测；
  run、原始数值和原始来源保持不变，仅转换请求场景标签。原缓存文件 bytes 未变，
  未新建 alias 缓存文件，因此没有重新计时或续期。

此实验通过真实采样后重开 Store 验证等价和批量路径，没有注入合成缓存；它不是完整
跨组 CLI 长测对照。完整 CLI 的跨命令路径由上面的 Zoom 实验覆盖。

产品 EXE SHA256 与 Zoom 实验相同；测试 harness SHA256 为
`3add9eedb43bcd2094551dd60d7087546dcebd278f4dfd0e8da01aaab11f3707`。
结果：`target/acceptance-profiling/resource-native-reuse-review.json`；
日志：`target/acceptance-profiling/resource-round4-live-20260930/native-alias-batch.log`。

## 检查与证据边界

完整 smoke、fmt、strict clippy 和 Phase 00 的最终结果将在提交前追加 Resolution。
本轮现场覆盖完整 Zoom 及两个真实场景的缓存行为；完整五组正式资格化、Window 全压力、
四对等价场景全部跨组 CLI 对照、真实故障触发的 FailureFirst 现场排序和人工矩阵仍为
NOT TESTED。相关身份漂移、损坏/过期、部分批次回退、优先排序及强制新测量由自动回归
覆盖。没有改变正式资源门槛，也不继承 `v0.1.1` exact-artifact 或发布特例，原技术
readiness 仍为 NOT_READY。

## Resolution — 2026-09-30：回归与 review 完成

- `cargo test -p stickymd-smoke --locked`：271 passed、6 ignored；集成 17 passed。
  新增 native 测试需显式启用，本轮已单独执行并通过，不混入常规测试耗时。
- `cargo fmt --all -- --check`、
  `cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings`、Phase 00 通过。
  日志分别为 `target/acceptance-profiling/resource-round4-full-tests.log`、
  `resource-round4-fmt.log`、`resource-round4-clippy.log`、`resource-round4-governance.log`；
  治理 JSON 为同目录 `resource-round4-governance.json`，如实记录提交前 worktree。
- 最终检查保留严格身份、原始来源、五次完整观测和正式/诊断路径隔离。两个 P2 均已
  修复，当前审查范围未发现其他阻塞问题；未覆盖的现场矩阵仍按上一节保留。
  本轮只作本地提交，不 push。
