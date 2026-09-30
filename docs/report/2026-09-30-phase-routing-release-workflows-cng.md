# 2026-09-30 阶段入口、发布流程与 CNG 哈希迁移

已依次完成剩余阶段入口统一、打包/SBOM 内部编排和 Windows 哈希后端替换。
收益分别是参数/拒绝行为单点维护、减少 runner 的跨进程往返，以及非空文件哈希命令的实测耗时下降。
独立 PowerShell SBOM 入口存在额外进程成本，不能把内部调用的收益外推到所有入口。

## 背景与契约

用户明确授权按这三个优先级实现、验证和同步文档。开始时 HEAD 为
`fc233ae224a016b17cecf9d9c1e59837646eef5c`，工作树已有上一轮 ZIP 落盘、Phase 12–14、远程观察
与 SBOM 准备组合的未提交改动。本轮读取并保留这些改动；没有提交、推送、触发或轮询远程工作流。

适用 AGENTS、工程宪法、术语、plan 11、first-run feature 与阶段验收投影已核对，结构查询使用已有
CodeGraph。权威仍为 [phase-verification-harness](../plan/11_testing_and_release.md#phase-verification-harness)
和 [release-artifact-authority](../plan/11_testing_and_release.md#release-artifact-authority)。
没有修改 plan、产品代码、Cargo 依赖/锁文件、发布权限、候选身份或人工验收状态。

问题是：14 个旧阶段入口重复拼参数；runner 通过 PowerShell 回调 Cargo/Rust 发布命令；
Windows 共用哈希对每个非空文件启动 certutil 并解析输出。

## 实现与保留边界

1. `phase_entry/` 复用 canonical Phase/Options parsers，覆盖 00–14、11-b 和 all。
   各入口保持原有参数名称、类型、顺序和可用范围；PowerShell 共享参数绑定、CWD/编码恢复及退出码转发。
   治理只检查阶段入口存在，真实路由/拒绝行为由编译后的 CLI 和双宿主入口测试证明，不再搜索脚本关键字。
2. `release/package_workflow` 与 `sbom_workflow` 直接复用 staging、身份、发布、checksum、Syft pin/快照规则。
   runner 保留原 task identity、顺序及 JSON 输出边界，新增薄 CLI `build-package` / `generate-sbom`。
   `package.ps1` / `generate-sbom.ps1` 各调用一次锁定 Cargo；runner 内部无需递归调用这些入口或 Cargo run。
   原 prepare/publish 命令及其输出继续保留。`syft/plan` 提供同一 typed plan 的 JSON 投影。
   下载的三次重试、1/2 秒退避、partial 清理和校验发布改由 Rust 持有，旧下载编排 helper 移除。
3. `integrity/windows` 使用系统 CNG，以 64 KiB 有界缓冲流式读取，处理短读、中断和读取错误。
   每次重新打开并读取输入，不复用摘要；空文件正常计算，不再特判常量返回或解析 certutil 文本。
   RAII 保证 hash 先于 provider 释放；unsafe 限定在 Windows 工具适配模块并附 SAFETY 说明。
   `integrity/portable` 保留原 sha256sum 后端。没有自行实现加密算法或引入 crate。

PowerShell 保留稳定入口、ZIP 压缩/解压、网络传输和 Syft 外部执行。新 helper 为
`package-archive.ps1`、`syft-fetch.ps1`、`syft-execute.ps1`。入口将自身 5.1/7 host 路径作为平台事实传入，
避免压缩时无意切换 .NET 实现；直接 CLI/runner 默认 pwsh。Syft 解压使用仓库 archive-facts 已采用的
`ZipFile.ExtractToDirectory`，保留新目录、拒绝覆盖与危险路径拒绝；UIA/COM/资源读取不在本轮改动范围。

CNG 接口和生命周期依据 Microsoft 的
[CreateHash](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptcreatehash)、
[HashData](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcrypthashdata)、
[FinishHash](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptfinishhash) 与
[OpenAlgorithmProvider](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptopenalgorithmprovider)
文档核对。工具侧平台适配不进入 StickyMD.exe。

## 同输入基线与实际产物

本轮 ignored 证据目录为 `target/cli-continuation-20260930/`，保存开始时 diff、旧脚本、旧工具二进制、
对照脚本、CSV 与验证日志。先以 `cargo build -p stickymd-win --release --locked` 检查当前 EXE，
再用迁移前入口在 `baseline 中文 space/` 生成新包；迁移后在 `after 中文 space/` 和
`final 中文 space/` 独立生成。所有包都是本次 dirty worktree 的 `DIRTY_VALIDATION`，不是正式候选证据。

同一 EXE/版本/来源/锁图下，迁移前后 ZIP 字节一致：

```text
217388f56a727ec2a55c51f62d96cf6e79b3f39ed495cb61a9bfc81bbdeea4da
```

实际缓存的 Syft 1.50.0 生成 SPDX，仅统一 `documentNamespace` 与 `creationInfo.created` 后，
完整对象序列化结果相同。最终 SBOM SHA-256 为
`ba874beabce19143629302dd48205e1ca358bf9d9f6d0164cf46bc37953a2f1f`。
notices 仍有 187 项运行时依赖。没有拿历史 Release 包或旧验收收据证明新工具有效。

## 性能测量与限制

所有数据为同一台 Windows 机器、本地暖缓存、串行测量；不代表 CI、下载、产品启动或其它机器。

### SBOM 调用链

五组交替顺序，首对预热不计入，两条路径使用相同 CNG 后端与已验证 Syft 缓存。
Rust 直接调用与旧 runner 风格（启动 pwsh，旧脚本两次 cargo run）的结果：

| 入口 | 中位数 | 最小 / 最大 | 样本 |
| --- | --- | --- | --- |
| 旧 runner 风格 | 5.838294 s | 5.340396 / 6.466221 s | 5 |
| Rust 内部流程 | 3.965717 s | 3.525617 / 4.072065 s | 5 |

本组中位数减少 32.1%。日志 `sbom-direct-timing.log`，可复用 opt-in
`compare_in_process_and_legacy_dispatch`，必须显式提供本轮新包与迁移前脚本快照；测试拒绝缺失缓存以避免下载。

另测已运行 PowerShell 会话中的独立脚本入口，结果不同：初版新流程为 4.318926 s，旧脚本为
3.591175 s。新流程多启动平台适配进程；改用直接 .NET 解压后，另五对交替测量为新 4.807842 s、
旧 4.504699 s，仍慢约 6.7%。两次测量时系统绝对耗时不同，不能跨批把差值全归于解压改动。
这些数据在 CNG 替换前取得，只用于比较编排和适配开销；不声称独立 PowerShell 入口提速。
对应 CSV 为 `sbom-workflow-initial-timing.csv` / `sbom-workflow-timing.csv`。

### 哈希命令

迁移前单独基线已记录在 `hash-before.csv`。最终使用保存的旧 debug CLI 与新 debug CLI，
同一文件七组交替测量；首轮预热不计入。测量包括进程启动、文件哈希、manifest 原子写入与输出，
不是纯算法吞吐。每次都与 PowerShell Get-FileHash 的摘要比对。

| 输入 | 旧中位数 | 新中位数 | 变化 |
| --- | --- | --- | --- |
| 空文件 | 38.0271 ms | 41.3508 ms | 增加 8.7%，无收益 |
| 4 KiB | 111.4488 ms | 49.6075 ms | 减少 55.5% |
| 16 MiB | 143.0060 ms | 63.8230 ms | 减少 55.4% |

原始记录在 `hash-comparison.csv`；小样本只支持这个工作负载的本地诊断结论。

## 验证与修正

- 阶段迁移前：原有 phase-entry 三个单元案例与双宿主案例通过；扩展的旧入口同输入映射基线在
  5.1/7 都通过。迁移后新增各阶段参数范围、shard、非法组合、false、中文/空格路径和状态恢复覆盖。
  最初测试误把 Release/Package 当作可同时开启，已修正为分别比较并保留原 canonical 拒绝行为。
- 发布迁移后：35 个 release 单元案例通过，两个 opt-in 案例未在该命令执行；后续双宿主回归通过。
  将旧 PowerShell 下载 mock 迁到真实 transport helper 时发现 script-scope 计数不再共享，修正隔离测试
  的计数作用域后通过；实际重试/校验/partial 清理的权威测试已在 Rust。
- 最终 `cargo test -p stickymd-smoke --locked`：297 passed、9 ignored；headless 集成 21 passed。
  覆盖两种 PowerShell 的实际入口、源码/哈希/重复 checksum/危险路径、失败 Syft 输出与旧文件保留。
  危险 ZIP 还须在外部 Syft 执行前拒绝。没有源码关键字补丁伪装 gate 执行。
- CNG：已知空/abc/多块向量、百万 a、63 字节短读、中断、错误读取、错误长度、缺失/目录/锁定输入和
  文件变更重读通过。FFI 前显式检查安全 Read trait 返回的长度，避免信任错误实现导致越界。
- `cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings` 通过。
- 最终新包串行执行 `direct_package_runtime_preserves_output_and_cleanup`：1 passed。
  覆盖 ASCII、空格、中文目录、同目录第二实例退出且持久文件不变、不同目录独立运行、临时清理。
  检查前后 StickyMD 进程数均为 0；没有键盘、剪贴板、托盘或人工 Session 操作。

CLI README、P00-A11、各阶段入口投影、REL-CLI-16/19/20 与 coverage matrix 已同步。
人工状态仍为 NOT TESTED，路径选择、包有效性、候选身份与发布资格保持分离。

## 未验证项与可选路径

未在 Linux 实机执行；未测试真实下载/断网重试、远程 CI 或 GitHub 写操作；未运行完整资源、性能、
桌面及人工 Campaign。没有更新任何旧发布身份、readiness 或人工验收结论。

保留平台 helper 的选择降低了 ZIP/Syft 重写成本。独立 PowerShell 入口若继续优化，应针对其额外
宿主启动建立独立测量；本轮没有引入长期二进制缓存、持久 helper 服务或新的 IPC 协议来掩盖这项成本。

## Resolution：2026-09-30 交付复核

最终 `cargo fmt --all --check`、`git diff --check` 通过；文档更新后执行
`stickymd-smoke phase-entry 00`，governance contracts 通过。按 .gitattributes 对本轮已修改文本的
行尾作了规范化，未更改产品或依赖文件。默认忽略的九项诊断中，本轮另行显式运行了 SBOM 内部计时
与新包 runtime 两项，其余未运行。最终 SPDX 再次与迁移前新生成基线进行语义比较，通过。
所有已有工作树改动保留，仍未提交或推送。
