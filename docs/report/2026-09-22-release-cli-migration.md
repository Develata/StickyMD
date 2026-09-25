# 2026-09-22 release CLI migration

## 背景与边界

在现有 plan 11 下将发布规则迁入 std-only `stickymd-smoke`。保留 PowerShell 参数入口、ZIP/Syft/native resource/UIA 平台适配，不修改产品 runtime 或发布授权合同。

工作树开始时存在 runtime 审查改动、四份验收投影及 coverage-matrix 改动；全部保留。runner 的已有串行性能测试改动保留。CodeGraph 两次查询均返回 disk I/O error，因此用当前源码核实。

## 实现与复核

来源、版本、完整 SHA、checksum 及 notices/package 规则按职责放入 Rust；资格化 receipt 复用 `integrity`。包路径选择、模块化检查及 CI 选测直接复用。PowerShell 源码中的 Source commit/notice 关键字断言移除，相应行为由 compiled Rust 和双版本 wrapper 集成测试验证。

本次还修复 Linux 基线编译错误：startup 收据引用 Windows-only runtime 常量。两个常量移到共享 tooling 模块，值和产品行为均未改变。

## 验证记录

验证正在收尾，完整命令和结果将在本次交付前追加。基线与本次临时产物均位于本次新建的隔离临时目录，未用旧发布包或旧收据证明新工具有效。

## 当前状态与限制

commit: pending。未提交、推送、触发远端 workflow、建立 Source Freeze 或发布。
人工验收、Clean Windows VM、Resources/Performance/G3/G4/G5 全 Campaign 未运行；历史矩阵状态保留。

## Resolution — 2026-09-22 完成交付验证

Base HEAD: `268e525c7b9d45cc98f07f6dbf3ee6a20d95cfe7`；本次修改仍为 `commit: pending`，未建立新的发布源身份。

### 职责与兼容性

- `integrity.rs`：沿用既有 Windows certutil/Linux sha256sum 适配，统一完整 SHA/SHA-256 格式及 checksum manifest 校验；promotion、package、candidate receipt 共用。
- `release/identity.rs`、`promoted.rs`：当前 HEAD、完整来源 SHA、tag/version、用户预期 ZIP/SBOM hash、README 唯一来源；只验证输入，不 Promote。
- `release/package_inputs.rs` + 既有 `package_path.rs`：local RC/dirty/tag/exact 命名和 dirty 策略、参数覆盖语义；只输出 `NOT_RUN` 计划。`generate-sbom.ps1` 复用仓库版本读取。
- `release/package*.rs`：六成员白名单、危险/重复路径、30 MiB 边界、manifest 与实际 ZipPath 绑定、私有 ZIP 快照、notice 对比、版本/icon 事实断言。PE 头、GUI subsystem/manifest 复用 `pe_dependencies` 的 parser。启动与多实例检查复用 `ChildGuard`，使用原始 bytes + mtime 比较 bootstrap 文件。
- `release/notices/`：锁定、Windows-filtered Cargo metadata；normal-edge 闭包、build/dev-only 排除、经过本地依赖继续遍历、循环终止、Ordinal 排序、完整 reviewed fallback 清单、严格解码及 UTF-8/LF 产出。crate 仍无 dependencies。
- 既有三个 PowerShell 参数入口保留为转发层；`package.ps1` 保留 staging/README/ZIP 组装，`archive-facts.ps1` 与 `resource-facts.ps1` 只采集事实或执行原生操作。Syft 获取/运行和 UIA/COM 适配继续保留。
- 新 notice 文件先写完整临时文件再原子 no-replace publish；Windows 使用现有 MoveFileExW 适配且不设置 replace 标志，Linux 使用 hard link。并发创建仅允许一个成功；失败不覆盖现有目标，也不删除未取得所有权的临时文件。

有意修正的宽松检查：旧 promotion/receipt 只检查 checksum 行存在，现拒绝重复、额外成员和危险名称；README 多个来源声明拒绝；显式 ZipPath 不能用另一个同名文件的 checksum 冒充校验。合法输入、输出字段、路径及脚本失败退出语义保留，资格化 receipt schema 与发布权限不变。

### 基线与同输入对比

迁移前保存原脚本、初始 diff/status，并实际运行旧 promotion 和 notices 入口。以本次 locked Release build 生成新的 DIRTY_VALIDATION 包和 Syft SBOM；旧 package verifier 实际通过，再用相同输入验证新实现。

- Notices：187 个 registry packages，1,829,345 bytes；旧 PowerShell 与 Rust 全文件 `cmp` 相同，SHA-256 `eae30f48408b0cd66de03370fc75831cf0b113ba86177001c4325f81add666cc`。
- Portable ZIP：迁移前、迁移后重新打包逐字节相同；SHA-256 `69bb8ee9ec443e783375bf360451d42633d47c295dab34b8e02f89860fcdd70c`。重新生成的 SBOM 独立校验，不要求带时间等信息的 SBOM 跨次相同。
- 临时基线及最终验证产物：`E:\stickymd-cli-baseline-poeg7uvw` 下独立目录；最终包位于 `package-final 中文 with spaces`。这些都是本次工具验证输入，未进入 `dist/exact-candidate/`，不能用作 Release Exact Artifact 或人工验收证据。

### 实际故障与修正

首次真实启动验证在空 bootstrap note 上失败。独立对 0-byte 文件运行 `certutil -hashfile <empty.md> SHA256`，稳定返回 `0x800703ee / ERROR_FILE_INVALID`；非空文件和混合斜杠路径成功，因此不是 ZIP、中文路径或产品保存失败。Rust 文件不变性断言改用原始 bytes + mtime，并加入空文件、内容变化、文件缺失回归。失败时 RAII 已清理测试进程，原有用户便签仍存活；修正后新隔离目录重跑通过。

### 执行命令与结果

| 检查 | 命令 / 范围 | 结果 |
| --- | --- | --- |
| Linux headless | `cargo test -p stickymd-smoke --locked --target-dir /home/deve/.cache/stickymd-cli-target` | 127 unit + 7 CLI integration，0 failed |
| Windows headless + wrappers | `cargo test -p stickymd-smoke --locked -- --test-threads=1` | 159 unit + 7 CLI + 1 package-path wrapper + 1 双宿主 release wrapper，0 failed |
| Rust 格式 | `cargo fmt --all -- --check` | 通过 |
| Windows Clippy | `cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings` | 通过，无 warning |
| Linux Clippy | `cargo clippy -p stickymd-smoke --all-targets --locked --target-dir /home/deve/.cache/stickymd-cli-target` | exit 0；保留既有 Windows-only 路径在 Linux 下的 dead_code warnings，不作无警告声明 |
| 当前产品构建 | `cargo build -p stickymd-win --release --locked` | 首次本次 fresh build 成功，最终同输入检查成功 |
| Native imports | `stickymd-smoke qualification native-runtime --exe=target/release/stickymd-win.exe` | inbox imports 通过；无 developer runtime imports |
| 新包实际流水线 | `package.ps1 -OutputDirectory <isolated> -AllowDirtyValidation` → `generate-sbom.ps1` → `verify-package.ps1 -Runtime` | 新 ZIP/SBOM 校验通过；ASCII/空格/中文、同目录零 durable write、不同目录实例通过 |
| PowerShell 兼容 | Windows PowerShell 5.1.26100.9444 与 PowerShell 7.6.6；wrapper fixture + 新完整包复核 | 缺失/错误来源、错误 hash、重复/危险/缺失 checksum、缺失/重复 README、危险/非白名单 ZIP、中文/空格路径、输出已存在、父目录缺失均正确处理；CWD/encoding 恢复 |
| 数据与工作树 | 初始 diff 逐文件比对、Cargo manifest/lock 字节比较、`git diff --check` | 原有 14 份其他 dirty diff 及 runner diff 保留；coverage 仅追加本次映射；manifest/lock 无变化；空白检查通过 |

上述本地检查不写资格化成功账本。最终真实 GUI 检查串行执行，结束后保留了全部 1 个既有 StickyMD 进程，未留下测试 StickyMD 进程。

### 未运行与限制

没有执行远端 workflow、完整 workspace 产品回归、Resources/Performance/G3/G4/G5 Campaign、人工/物理显示器矩阵或 Clean Windows VM。与本次工具改动相称的 smoke crate 全测试、当前产品 locked build 和针对性真实包/启动检查已执行。人工状态和旧发布身份均保留；没有性能对比测量，因此不声称性能收益。Linux 不执行 Windows ZIP/resource/GUI 适配；其他文件系统/Windows 版本未额外验收。

### 最终复核补充

最后复核将 smoke evidence 的 EXE 哈希与 checksum 读取也接入 `integrity`，避免另一份解析规则；未改变 evidence schema。包与 promotion 在成功路径显式清理私有快照目录，清理失败返回非零；错误路径继续由 Drop 做 best-effort 清理。包白名单与大小测试使用独立的 plan 投影数据，避免随实现常量一起漂移；Windows license 枚举保留原入口对 hidden files 的处理。

最终源文件再次通过 Windows 159 unit + 9 integration、Linux 127 unit + 7 integration，Windows Clippy `-D warnings` 与 workspace fmt 检查。最终文档通过 `phase 00 --json`，Windows 稳定 `tools/smoke/phase-00.ps1` 入口也通过。成功清理路径调整后，针对相同本次新包再次执行 package verifier 通过；未因纯工具清理调整重跑 GUI Campaign。

## Resolution — 2026-09-22 Review and fix

本节复核的是当前未提交迁移实现，不继承上文完成声明。CodeGraph 对 WSL 路径返回 disk I/O error，重试 Windows 路径仍无法读取索引，故按源码与独立复现检查。原有产品 dirty work 保留；base HEAD 不变，`commit: pending`。

### 按严重程度排列的发现与修正

1. **P1：哈希适配器可把路径当作摘要。** 既有 certutil parser 搜索 stdout 的任意 64 位十六进制 token；路径 `中文 <64 个 a> with spaces/abc.bin` 的诊断标题先于真正摘要，结果错误返回 64 个 a。独立编译直接调用当前 `integrity::sha256`，实际 `abc` 的 Get-FileHash 值为 `ba7816bf...15ad`，修复前返回 `aaaa...`。迁移扩大了该共享适配器的调用面。现 Windows 只接受唯一独立摘要行；Linux 只读取首字段并处理 GNU escaped filename 前缀。路径先 absolute 化，不能被当成命令选项或标准输入标记。
2. **P2：空文件哈希兼容性缺口。** 原 PowerShell Get-FileHash 正常返回 SHA-256(empty)，certutil 在本机返回 ERROR_FILE_INVALID。共享适配器现先通过实际打开并读取确认 EOF，再返回标准空串摘要；不把 metadata length 当作观察证据，也不吞掉打开/读取错误。不引入新的密码算法、依赖或进程协议。
3. **P2：版本提取存在格式回退和作用域错误。** `version="0.1.0"` 由原 PowerShell regex 正常读取，新调用复用的旧 Rust reader 却报 missing；另一个表在前时会读出该表的 `9.9.9`。现读取仓库 `[workspace.package]` 的有引号 scalar，兼容赋值空白和尾注，缺失/重复/不支持的 scalar fail closed，不选择依赖或 metadata 中的 version。完整 TOML 有效性仍由锁定 Cargo 构建负责。
4. **P2：打包测试遗留进程不在测量前置检测范围。** `release::temporary` 下的 runtime 位于 `stickymd-verify-*`，而旧 detector 未识别该前缀。先加入真实目录形状断言，Windows 定向测试稳定失败，再补入该已知工具目录；现同一生命周期回归同时测试 smoke 与 package 目录中的活跃测试进程，只报告/阻断，绝不终止既有进程。

上述修正均收敛于工具事实读取与已有隔离合同，不改变产品运行时、候选身份、发布权限、人工边界或依赖政策。与保留错误的宽松 token/首行提取相比，选择准确解析现有适配器输出，不新增哈希库或第二份规则。

### 可重复证据与验证范围

隔离复现目录为 `E:\stickymd-cli-review-l5816hy8`。`probe.rs` 直接引用本次源码模块，`reproduce.ps1` 对比原 Get-FileHash/regex 与 Rust；修复前后运行相同输入，摘要和版本均恢复正确。日志为 `/tmp/stickymd-cli-review-reproduce.log`、`/tmp/stickymd-cli-review-fixed-probe.log`。目录分类修复前失败记录在 `/tmp/stickymd-cli-review-isolation-before.log`。

新增/扩展回归涵盖已知摘要向量、空文件、中文/空格/十六进制路径、Linux 换行与反斜杠路径、版本表作用域和错误输入、遗留测试进程的阻断与存活。PowerShell wrapper fixture 也使用真实的十六进制路径词，防止只在纯 parser 测试中修正。完整复核结果在下节追加。

### Review 修复后的验证结果

- `cargo test -p stickymd-smoke --locked -- --test-threads=1`（Windows）：163 unit + 7 CLI + 1 package-path wrapper + 1 双宿主 release wrapper，172 tests 全部通过。PowerShell 5.1 与 7 均实际可用并执行；没有把缺失宿主计为通过。
- `cargo test -p stickymd-smoke --locked --target-dir /home/deve/.cache/stickymd-cli-target`（Linux）：132 unit + 7 CLI，139 tests 全部通过。
- `cargo fmt --all -- --check`、Windows `cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings` 通过。Linux 对应 Clippy 退出 0；仍有原有 Windows-only 路径的 dead_code 警告，逐项与上一轮日志核对一致，无新增警告。
- 全新集成输入：`cargo build -p stickymd-win --release --locked` 增量确认当前产品构建；在 `E:\stickymd-cli-review-l5816hy8\package 中文 <64 个 a> with spaces` 重新执行 `package.ps1 -AllowDirtyValidation`、`generate-sbom.ps1`、Windows PowerShell 5.1 `verify-package.ps1 -Runtime`，随后串行用 PowerShell 7 再验包，全部通过。生成 187 个运行时依赖 notices，ZIP SHA-256 `69bb8ee9ec443e783375bf360451d42633d47c295dab34b8e02f89860fcdd70c`，本轮新 SBOM SHA-256 `0d716c4a79bb56f29c19da52b3773945b0e533b964ab4772e92b854eb3e35fbf`，Syft 1.50.0。
- 实际 bootstrap 覆盖 ASCII、空格、中文目录、同目录第二实例退出/文件不变、不同目录主实例存活。结束后无本轮遗留测试进程，预先存在的用户便签 PID 29116 仍存活。未发送键盘、剪贴板、托盘或鼠标输入。
- 以迁移前保存的 `initial.diff` 比较 15 份既有文件差异，逐字节一致；既有 coverage 新增段完整保留，Cargo.toml/Cargo.lock 与原始副本一致。

主要日志：`/tmp/stickymd-cli-review-{windows-full,linux-full,clippy-windows,clippy-linux,fmt,integration}.log`。CLI integration 内的 phase 00 治理检查通过；本轮文档同步维护 README、REL-CLI-02/06/07 及 coverage 映射，人工矩阵状态未改。

未运行：全量产品 workspace 测试、Resources/Performance/G3/G4/G5 Campaign、人工验收、Clean VM、多 Windows 版本或远端 workflow；本轮是工具修复的定向验证。没有性能收益测量或声明。上述本地 DIRTY_VALIDATION 产物不是发布候选，未创建新的 Source Freeze、candidate 或人工收据。未提交、推送、触发远端工作流或发布。

### Review 追加发现：checksum 角色重名

**P2**：再次审查严格 manifest 的集合判断时，发现调用者若用 `--zip .../SBOM.spdx.json`，预期 ZIP 与 SBOM 名称相同；只比较 manifest 条目数再逐项查找，会把另一个多余成员漏检。用本轮新 ZIP 的 bytes 复制为 `artifact alias fixture/SBOM.spdx.json`，manifest 写该摘要和一个不存在的 `unused.bin` 条目，修复前真实 Windows verifier 输出 `PACKAGE_VERIFY=PASS`（`/tmp/stickymd-cli-review-alias-before.log`）。这是包验证的错误通过，不代表 candidate/remote Promote 已被绕过。

修正位于共享 `integrity::verify_manifest_text`：预期名称经相同大小写规范化后必须互异，之后条目数与逐项匹配才构成严格集合相等。加入纯规则测试及 compiled CLI 测试，后者断言非零、无成功 stdout，并在 ZIP/native 操作前以 `distinct names` 拒绝；验收投影 REL-CLI-02 同步补充。没有新增第二份包名规则或修改候选身份合同。

追加修复后的最终验证：同一 alias 输入退出 1，报 `Expected checksum artifacts must have distinct names`，且没有 `PACKAGE_VERIFY=PASS`；正常的本轮新包再次验证通过（`/tmp/stickymd-cli-review-alias-after.log`、`/tmp/stickymd-cli-review-package-final.log`）。该追加修复只改变 manifest 规则，没有重复执行 GUI 检查。

最终全套 smoke 为 Windows **164 unit + 8 CLI + 2 wrapper = 174**，Linux **133 unit + 8 CLI = 141**，均零失败；Windows Clippy `-D warnings`、Linux Clippy（既有 dead_code 警告）和 workspace fmt 再次通过。日志为 `/tmp/stickymd-cli-review-{windows-final,linux-final,clippy-linux-final,fmt-final}.log`。稳定 Windows `phase-00.ps1` 入口及最终 CLI integration 中的 phase 00 均通过。未验证范围、权限边界、原有工作树所有权与 `commit: pending` 状态沿用本节前述说明。

## Resolution — 2026-09-25 当前工作树续验与跨 PowerShell 宿主修复

### 已核实事实与本轮范围

当前分支为 `codex/modular-ci-tooling`，HEAD 为 `268e525c7b9d45cc98f07f6dbf3ee6a20d95cfe7`，
本地 `origin/main` 为 `37fa6e0b6da3ae83c16bb29a42b605ad078ac3c2`，未 fetch。进入本轮时已有未提交的
产品维护、三项 release CLI 迁移、文档和测试改动。本轮读取当前实现并重新验证，没有将前述历史通过记录
当作本轮证据，也没有重做已实现的迁移。CodeGraph 当前可用。

已核对：来源、完整 SHA、预期 hash 与 manifest 规则由 `identity/promoted/integrity` 持有；
包白名单、危险路径、大小、来源与资源断言由 `package/package_rules` 持有；启动和实例检查复用
`ChildGuard`；依赖通知由 `notices` 的 normal-edge 图遍历、排序及许可证选择实现。
候选 receipt 和普通包验证复用 `integrity`，PowerShell 原有 gate 源码关键词断言已改由 compiled
Rust 与 wrapper 行为测试承担。包路径、模块化检查和 CI 选测的既有 Rust 实现继续复用。

### 故障、复现与修正

初始 `cargo test -p stickymd-smoke --locked -- --test-threads=1` 中，164 个 unit、8 个 CLI 和
package-path wrapper 通过，release wrapper 在 Windows PowerShell 5.1 的 `Get-FileHash` 处失败。
独立通过 Python 原生子进程复现：继承 PowerShell 7 的 `PSModulePath` 时退出 1，移除该子进程变量后
由 5.1 重建默认模块路径，退出 0。直接从 PowerShell 启动与经 Cargo/Rust 启动的环境处理不同。

随后增加确定性回归：在独立临时目录提供冲突的 `Microsoft.PowerShell.Utility`，由 wrapper 将其路径
传给 compiled Rust。修复前真实 `archive-facts.ps1` 在 `Add-Type` 处报模块加载失败；修复后相同输入
通过，并确认父进程的 `PSModulePath`、CWD 和控制台编码未被改动。

修复只在 `release/windows.rs` 创建子进程时使用 `env_remove("PSModulePath")`；两个 wrapper 测试宿主
同样从自身默认模块环境启动。发布适配器只需内置模块，因此无需保留其他 PowerShell edition 的模块目录。
没有修改全局环境、用户配置、产品运行时、依赖策略、候选身份或权限合同。未引入新依赖。

### 本轮实际验证

环境：Windows PowerShell `5.1.26100.9444`、PowerShell `7.6.5`。证据根目录：
`C:\Users\QQ\AppData\Local\Temp\stickymd-cli-20260925-fdb40c2992e5491ca293b65bbc7070d8`。
保存了初始 Git diff/status、文件 SHA-256 清单，以及 baseline/adapter-before/adapter-after/final-tests、
Clippy、构建与 integration 日志。旧脚本取自当前 HEAD，仅在隔离副本中将 repoRoot 指向实际仓库以便
执行原规则；未覆盖仓库脚本。旧 package verifier 调用当前 notices 入口，其输出已另行与旧 notices
实现逐字节比较。

| 检查 | 当前结果 |
| --- | --- |
| `cargo test -p stickymd-smoke --locked -- --test-threads=1` | 164 unit + 8 CLI + 2 wrapper = 174，通过；两个 PowerShell 宿主均实际执行 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings` | 通过，无警告 |
| `cargo build -p stickymd-win --release --locked` | 增量构建检查通过 |
| `qualification native-runtime --exe=target/release/stickymd-win.exe` | Windows inbox imports 通过，developer runtime imports 为 none |
| 旧新 notices 同输入比较 | 187 packages、1,829,345 bytes，逐字节一致；SHA-256 `eae30f48408b0cd66de03370fc75831cf0b113ba86177001c4325f81add666cc` |
| 新建本地 ZIP + Syft 1.50.0 SBOM | `DIRTY_VALIDATION`，独立中文/空格目录；未使用旧发布包 |
| 同包旧新验证 | 旧 package verifier 与新 5.1 `verify-package.ps1 -Runtime` 通过；随后 7 单独验包通过 |
| 真实启动 | ASCII、空格、中文目录 bootstrap，同目录第二实例退出且 durable files 不变，不同目录实例存活；测试串行执行 |
| promotion 输入 fixture | 本轮新 ZIP/SBOM 的独立副本；旧新 `verify-promoted-artifact.ps1` 的成功输出完全一致，未创建 candidate |
| 进程与用户数据 | 开始和结束均仅有原用户 StickyMD PID 29116；无遗留测试 StickyMD 进程 |

本轮新 ZIP SHA-256：`69bb8ee9ec443e783375bf360451d42633d47c295dab34b8e02f89860fcdd70c`；
新 SBOM SHA-256：`719e40bbc0379131e15564b384a749646a9cd7f213440b3ea664603c173513d0`。
即使 ZIP 摘要与历史本地构建碰巧一致，也只按本轮新生成、实际读取的输入报告，不据此宣称可复现构建或发布资格。

### 边界、推断与未验证事项

基于当前调用关系，发布规则集中与行为测试改善了维护可检查性；没有性能收益测量。
ZIP 压缩/解压、原生资源读取、Syft 获取/调用和 UIA/COM 继续保留为 PowerShell 平台适配，
`package.ps1` 继续负责 staging/README 组装，阶段入口保持不变。

本轮没有重新运行 Linux 测试、完整 workspace 产品回归、资源/性能/G3/G4/G5 Campaign、人工视觉、
物理多屏、Clean VM 或远端 workflow。历史人工状态保持原样。没有 Source Freeze、Promoted Candidate、
新资格化收据或账本更新，没有提交、推送、tag 或发布；全部改动仍为 `commit: pending`。

最终文档治理通过稳定 `tools/smoke/phase-00.ps1` 无参数入口。按初始文件 SHA-256 清单复核，
本轮只改变 `release/windows.rs`、两个 `release_wrappers` 测试文件、CLI README、Phase 14 投影、
coverage 与本报告；其他既有文件内容保持不变。`git diff --check` 通过。
