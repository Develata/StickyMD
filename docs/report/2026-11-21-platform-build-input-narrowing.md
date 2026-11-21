# 平台构建输入收窄为打包 crate：实现与验证记录

日期：2026-11-21。分支 `feat/package-crate-split`（从 `origin/main` `521903a` 开出），未 push。依据：
[Phase 18 验证记录](2026-11-20-phase-18-verification.md)末尾的 Resolution（USER 2026-11-21，“按你的建议来”）。

## 结论

产出发布字节的代码已迁入 `tools/stickymd-package`（std-only，库加二进制 `stickymd-package`），`stickymd-smoke` 改为依赖它。
Platform Build Input Fingerprint 升为算法 v2：`tools/stickymd-smoke/` 除清单外不再计入，`tools/stickymd-package/` 整体计入两个平台。
发布 workflow、Windows 打包脚本与 `build-app.sh` 中产出字节的调用点只调用打包工具，governance 检查编译器看不到的边。macOS 本机基线见下文。
M5 的“R 上的源码测试”只写进了合同，核对程序未实现。

## 提交

| 提交 | 内容 |
| --- | --- |
| `06c7c87` | 合同：`11` #platform-artifact-reuse、#macos-release-gate、#phase-16-macos-host；`01`；`AGENTS.md`；验证记录的 Resolution |
| `4a187cc` | 合同审查的修正：成员 `build.rs` 与分类器计入，边界写成对生产链全部输入的约束，R 与 S 各自证明什么，M5 两行与时点 |
| `fc6fc09` | 实现：迁移、改调用点、指纹 v2 与测试、边界检查、投影文档 |

## 迁移的范围

- Windows：包目录与 ZIP（`package_*`、`package_readme.txt`）、第三方声明（`notices/`）、SBOM 与 Syft 缓存（`sbom*`、`syft/`）、checksum、
  PowerShell 适配（`windows.rs`）、包路径与命名（`package_path.rs`）。
- macOS：包清单与 README（`macos/content.rs`）、打包与包校验（`macos/package.rs`、`listing.rs`）、SBOM（`macos/sbom.rs`）、bundle 身份
  （`macos/bundle.rs`，原 `mac_common/fixture.rs` 的一部分）。
- 产品输入指纹：原 `stickymd-smoke mac-ledger inputs`（`build-app.sh` 把它写进 Info.plist），连同构建身份、环境检查、版本规范化与 tracked
  文件列表，改为 `stickymd-package app-inputs`。`mac-ledger inputs` 已移除，调用时提示新命令。打包时核对产品输入的子进程也改调打包工具
  （原先在 `release/macos/package.rs` 里运行 `cargo run -p stickymd-smoke -- mac-ledger inputs`）。
- 公共代码：JSON、`integrity`（SHA-256）、原子写入、临时目录、`repository`、路径身份、JSON 转义。
- 留在 `stickymd-smoke`：包校验（`release/package.rs`、`package_rules.rs`、`package_runtime.rs`）、晋升与 resolver、附件集合、远端状态、
  workflow 观察、指纹分类表与沿用判定、全部资格化代码。

## 输出守卫

原先 `stickymd-smoke release` 对写出命令检查“不得写入资格化存储”，用到 `qualification` 中各协调者的路径。打包 crate 不知道这些路径，
改为拒绝整个 `dist/evidence/`、资源诊断缓存与克隆级账本存储（仍按路径身份比较）。这比原先严格：原先允许写到 `dist/evidence/` 下
不属于协调者的诊断路径。发布 workflow 与本地 runner 都写到 `dist/`，不受影响（推断，未在 Windows 上运行）。

## 已验证（本轮实际运行，macOS 27.2 arm64）

- **基线（`fc6fc09` 的工作树）：** `cargo fmt --all --check` 通过；`cargo clippy --workspace --exclude stickymd-win --all-targets --locked -- -D warnings` 通过；
  `cargo clippy -p stickymd-package -p stickymd-smoke --all-targets --locked --target x86_64-pc-windows-msvc -- -D warnings` 通过；
  `cargo test --workspace --exclude stickymd-win --locked`：1842 项通过，0 失败（打包 crate 74 项，含其 CLI 退出码测试）；`stickymd-smoke phase 00` PASS。
  `stickymd-win` 只在 Windows 编译，未在本机构建。
- **`phase 18` 按名运行：** 判为未就绪，原因是 P18-P04 等 Automated 行仍为 `NOT TESTED`（需要真实 dispatch），与拆分前相同。
- **字节等价（同一 tree，旧二进制取自 `521903a` 构建的 `stickymd-smoke`）：** 两个平台的第三方声明逐字节相同（Windows 189 个运行时包，macOS 196 个）；
  在仓库之外的干净克隆上，旧 `mac-ledger inputs` 与新 `app-inputs` 给出同一个产品输入指纹 `6e886b3e…`。
- **指纹测试：** 只改 `tools/stickymd-smoke/` 的源码与测试时两个平台的指纹相同、可沿用；改打包 crate、smoke 的清单、新增 smoke 的 `build.rs`
  或改分类器时两个平台都不能沿用（`platform_inputs.rs` 的临时仓库测试）。
- **边界检查：** governance 在真实仓库上通过，各条规则都有被拒的坏样本（`governance/package_boundary.rs`）。

## 推断（有依据但未执行）

- Windows 的打包字节不变：打包代码只改了模块路径与可见性，命令参数与默认值照旧；`package.ps1`、`generate-sbom.ps1`、
  `generate-third-party-notices.ps1` 只换了调用的 Cargo 包。两平台第三方声明在同一 tree 上逐字节相同（已验证）；Windows ZIP 与 SBOM 没有在
  Windows 上生成比对。
- Windows 上的 PowerShell 包装测试（`release_wrappers`）：假 `cargo` 按 `-p` 分派到两个二进制，打包二进制由测试进程用独立 target
  目录构建一次。两个 Windows 测试只在 Windows 上编译与运行，本机只做了 Windows 目标的 clippy。

## 未验证

- Windows CI（含 Windows PowerShell 5.1 的包装测试与 headless 计划中新增的 `package` 模块）。
- `release`、`release-macos` 的真实 dispatch。
- M5 核对程序与“R 上的源码测试”收据：只有合同。

## 影响

- 改 `11` 与 Phase 18 矩阵使 Windows 全部资格化模块的收据失效（#module-success-ledger）；下一版本本来就要整体重新资格化。
- 根 `Cargo.toml` 的成员表与 `Cargo.lock` 改变，macOS 产品输入指纹随之改变，账本中的 PASS 不再复用；这些行本来就要在候选 bundle 上重跑。
- 还没有任何候选收据，没有 v1 指纹记录需要迁移。

## 留给后续

- `tests/` 与 `tools/smoke/` 仍保守计入两个平台；构建与打包是否读它们没有逐项证明，可另行决定是否排除。

## 需要 Develata 决定：构建 job 中的验证代码（合同审查 BLOCKER 1）

**问题。** 两个发布 workflow 在产出字节的同一个 job 里运行不计入指纹的 `stickymd-smoke`，而且有的在产出步骤之间：

| workflow | 步骤（顺序） | 与字节的关系 |
| --- | --- | --- |
| `release` | `release workspace-version` → `all.ps1 -Ci`（smoke 全部测试）→ 构建 → `qualification native-runtime` → 打包 → SBOM → `verify-package.ps1` → 上传 | native-runtime 在构建与打包之间，读 `target/release/stickymd-win.exe`，打包随后取同一文件 |
| `release-macos` | `release workspace-version` → `cargo test … -p stickymd-smoke` → `build-app.sh` → 构建记录（`release platform-inputs`）→ 打包 → SBOM → `macos-verify-package` → 上传 | platform-inputs 在 bundle 与打包之间；校验在上传前 |

这些步骤能写后续步骤读取的被忽略文件（例如 `target/`），打包时工作树仍是干净的。所以“S 与 R 的指纹相同即可沿用”这个承诺，
对 S 上实际产出的字节并不成立：字节可能取决于被排除代码的运行副作用。审查认为这超出了“编译器保证验证代码影响不到打包代码”这一放宽前提。
合同当前把它写成由代码审查承担的信任假设（`11` #platform-artifact-reuse“构建 workflow 中的验证只是自检”）。

**方案。**

- **A（建议）：按 runner 隔离，分三个 job。** `self-check`（smoke：版本、fmt、Clippy、全部测试）→ `build-package`（`needs: self-check`；只运行计入指纹的代码：
  产品构建、PE import 门、打包、SBOM、macOS 包检查、上传）→ `verify-artifact`（下载刚上传的 artifact；smoke：Windows `verify-package`、构建记录中的平台指纹）。
  run 成功要求三个 job 都成功，晋升与 TAG/DRAFT/PUBLISH 照旧只认成功的 run。为了不改 `11` #portable-windows-runtime“在打包前执行该 gate”
  与 #macos-release-artifact“校验……然后上传”，PE import 门（`pe_dependencies.rs`，484 行，std-only）迁入打包 crate，打包二进制加
  `native-runtime` 与 `macos-verify-package` 两个命令（macOS 包检查的代码已经在打包 crate 中）；它们因此计入两个平台。
  代价：两个 workflow 重排、`release_workflow` 测试与 governance 的 workflow 检查随之改；多一个 runner。“构建前自检”的字面意思保留。
- **B：同一 job，把所有 smoke 步骤挪到上传之后。** 不加 runner；PE 门与 macOS 包检查同样要迁入打包 crate 才能留在上传前。构建前的测试变成上传后，
  与决定中“构建前自检”的字面不符；被排除代码仍与已上传字节同处一个文件系统，只是时间上在后。
- **C：保持现状，信任假设写进合同（当前文本）。** 不改 workflow；审查判为 BLOCKER。

A 与 B 都要再改 `11` #platform-artifact-reuse 的“构建 workflow 中的验证只是自检”一条；A 不改其他已批准的门。

## Resolution（2026-11-21）：构建 job 中的验证代码

USER 的回复（原话，经主会话转达）：

> A：拆三个 job

按方案 A 实施：每个发布 workflow 拆成 `self-check` → `build-package` → `verify-artifact` 三个 job；`build-package` 只运行计入指纹的代码；
PE import 门（`pe_dependencies.rs`）迁入打包 crate，打包二进制增加 `native-runtime` 与 `macos-verify-package` 命令；`verify-artifact` 下载刚上传的
artifact 并在它上面校验。`11` #platform-artifact-reuse、`release_workflow` 测试与 governance 的 workflow 检查随之修改。

## 方案 A 的实现与两份实现审查的处理（2026-11-21）

提交 `09145d5`（实现）与本节所在提交（验收行号与本节）。

- **workflow：** `release` 与 `release-macos` 各分为 `self-check` → `build-package` → `verify-artifact`。`build-package` 只运行 Git 的祖先核对、
  产品构建、`stickymd-package native-runtime`（PE import 门，`pe_dependencies.rs` 已迁入打包 crate）、打包、SBOM、macOS 的
  `stickymd-package macos-verify-package` 与构建记录，然后上传；`verify-artifact` 以 `actions/download-artifact`（v7.0.0，按 SHA 钉住）取回刚上传的
  artifact，用 `stickymd-smoke` 校验（Windows `verify-package.ps1`，macOS `release macos-verify-package`），macOS 另在摘要中写平台指纹。
  `stickymd-smoke qualification native-runtime` 与 `release macos-verify-package` 调用同一份库代码。
- **打包 crate 审查（`fc6fc09`）：** 已修——证据目录内的链接（含 Windows junction）指向别处时从真实路径写入也被拒；默认输出目录也受检；
  含反斜杠的 tracked 路径被拒；macOS 精确打包用归档内 bundle 的产品输入再比对一次，ZIP 以硬链接发布、同名已存在即失败；Info.plist 中注释或嵌套
  字典里的键被拒；README 的 `workspace-version` 一段。未改：守卫仍拒绝整个 `dist/evidence/`，比拆分前严格（拆分前允许写到其中非协调者的诊断路径），
  因为打包 crate 不知道协调者的具体文件，发布 workflow 与 runner 都写 `dist/`；库函数可被进程内调用而绕过 CLI 守卫，与拆分前相同（runner 的本地
  预检以默认路径调用）。“读取 smoke 的清单”按合同属计入的输入，已在合同中写明。
- **smoke 侧审查（`fc6fc09`）：** 已修——计入的例外按大小写不敏感匹配（`Build.rs`、`CARGO.TOML`）；边界检查改用区分注释与字符串的词法，
  逐层检查被 `include!`/`#[path]` 引入的 Rust 文件，跟进脚本中单双引号与 `&` 调用的引用，读不出的 dot-source 被拒，workflow 按 job 解析
  且注释不截断 job，依赖与构建脚本改由 `cargo metadata` 判定；Phase 18 发布工具任务与 P00-A07 的命令包含 `stickymd-package`；打包 crate 的改动
  也选中 macOS 的 CI lane；`.gitattributes` 的 LF 规则随模板路径更新；主帮助与迁走命令的提示（测试断言 stderr）；Windows 包装测试从 Cargo 的
  `compiler-artifact` 消息取打包可执行文件；分类器的测试移到 `platform_inputs/tests.rs`（不计入）；验收行 P18-P01、P02、P03、P05、P10 的行号按
  测试函数重新核对，P18-P05 的“排除表逐项”收窄为实际覆盖的范围。
- **已验证（macOS 27.2 arm64，`09145d5` 的工作树）：** fmt、Clippy（macOS 与两个工具 crate 的 Windows 目标）通过；workspace 测试 1848 项通过，
  0 失败；`phase 00` PASS。第三方声明在 `fc6fc09` 的 tree 上新旧逐字节相同：Windows `360d0993…`、macOS `8c7875b2…`（SHA-256 前 16 位）。
- **未验证：** 三个 job 的真实 dispatch（包括 `download-artifact` 取回的路径布局）、Windows 上的 PowerShell 包装测试与 CNG、Windows 打包字节在
  同一 Windows 环境下新旧比对。

## 窄范围复审（`09145d5`、`b6f8f1d`）的处理（2026-11-21）

- **已修：** 各成员的构建脚本必须自成一体（不声明模块、不 `include!` Rust 代码），workspace 中二进制名唯一（Cargo metadata）；打包 crate 的
  `include!`/`#[path]` 留在 crate 内，数据 include 按 Git 索引中的原样拼写核对（同一文件的大小写变体不算计入），`cfg_attr` 携带 `path` 被拒；
  词法器识别 `br`/`cr` 原始字符串与 `b'"'`，include 只在去掉字面量内容的 token 中查找，跨行参数可读；脚本引用按 `$PSScriptRoot` 或仓库路径解析到
  `tools/release`，同名异目录与解析不了的引用被拒，注释行不收集；workflow 的 `needs` 只读 job 直属字段，下载只认真实步骤，job 头可带注释；
  Rust 源码收集不跟随目录链接；输出守卫对每条命令只取一次链接快照，读取失败即拒绝，并核对命令实际发布的每个文件（含由默认目录派生的
  ZIP、checksum 与 SBOM）；Info.plist 改由 `plutil` 读取顶层键（注释与嵌套字典中的键不算）；macOS ZIP 的硬链接即提交点，之后的清理失败只警告，
  输出卷须支持硬链接；macOS `verify-artifact` 改为校验下载的完整三件套（新命令 `release macos-verify-artifact`，复用晋升的集合检查）；Windows
  `verify-artifact` 在摘要中写平台指纹；workflow 与合同写明重试从 `build-package` 起。
- **按合同修订而非改代码：** 合同原写“拒绝成员清单中的 `build` 键”，实际需要的不变量是“构建脚本只能是清单旁的 `build.rs`”，已改为以 Cargo
  metadata 判定（`build = false` 或显式 `build = "build.rs"` 都满足它）。
- **算法版本不再升：** 本分支内的分类器改动（例外改为大小写不敏感、`build.rs` 与分类器计入）都发生在 v2 合入之前；本仓库与本机的 ignored
  证据中未发现记录了 v2 值的候选收据或沿用记录（其他机器上的 ignored 目录无法从 Git 证明），所以 v2 指合入时的分类器。合入之后再改分类器或序列化，按合同升版本。
- **未改：** Windows `build-package` 仍以 `cargo build --workspace` 构建，与已资格化版本的 feature 合并方式相同；被排除成员在构建时能运行的代码，
  由 Cargo metadata 与边界检查限于各成员计入且自成一体的 `build.rs`（workspace 中没有过程宏成员，二进制名唯一）。只构建 `stickymd-win` 是否会实际改变
  产品的 feature 图没有比对，按 Cargo 规则可能改变。改为只构建 `stickymd-win` 会改变 feature 合并，属于 Windows 行为变化，不在本次范围。

## 第三轮窄范围复审（`69f0f41`）的处理（2026-11-21）

用户要求“再审一轮”。复审判为不通过，两项 BLOCKER，均已修：

- **打包 crate 内非 `.rs` 片段失去递归检查（`69f0f41` 引入的回归）：** 边界检查改为按队列逐个检查 `include!` 与 `#[path]` 引入的文件，任何扩展名，
  去重；引入的片段不得声明外部模块。
- **构建脚本的外部模块漏检（`mod r#type;`）：** 宏、属性与 `mod` 声明改为在去掉字面量内容的 token 上按标识符识别（含 raw identifier），内联模块允许，
  合同措辞随之改为“不声明外部模块文件”。

SHOULD_FIX 已修：Git 索引改用 `git ls-files --cached`（不含未跟踪文件）；`include_str !(…)`、`# [ path = … ]` 这类带空白的写法与 `std::` 前缀都能识别，
自定义宏 `my_include_str!` 不算；`cfg_attr` 按配对括号解析，跳过谓词，检查后续属性与嵌套的 `cfg_attr`；打包 crate 启动的脚本取自词法器记录的字符串
字面量，带目录的脚本名被拒；PowerShell 引用只在绑定到 `Join-Path $PSScriptRoot '<名>'`、`$PSScriptRoot/<名>` 或仓库路径时成立，`build-app.sh` 返回的
引用也被跟进；下载步骤只认步骤自身的 `uses`（块标量中的文字不算）；守卫遇到解析不了的链接（悬空、环）即拒绝，保留根本身是链接时计入其目标，
不写文件的命令不再遍历证据树；Windows 打包只解析一次包计划，守卫与 writer 用同一份，受保护目录的错误先于计划错误；Info.plist 读一次到私有副本，
以 `plutil -n` 取值并逐字节比对（首尾空白不再被接受）；macOS 集合检查只接受普通文件（新增缺件、多件与链接的负例）；合同中的重试规则限定为
macOS（Windows artifact 名不带 attempt）；workspace 中出现过程宏成员即拒绝；报告中“没有任何 v2 值”“只剩构建脚本”两句收窄到可证明的范围。

保持不变：顶层同名键在原始 XML 中出现两次即拒绝（注释或嵌套字典中的重名也会被拒，偏保守；`build-app.sh` 的模板没有这种写法）；未结束的 raw string
现在报错（不再退化为普通字符串）。

