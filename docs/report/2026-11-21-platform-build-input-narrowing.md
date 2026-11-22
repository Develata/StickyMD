# 平台构建输入收窄为打包 crate：实现与验证记录

日期：2026-11-21。分支 `feat/package-crate-split`（从 `origin/main` `521903a` 开出），未 push。依据：
[Phase 18 验证记录](2026-11-20-phase-18-verification.md)末尾的 Resolution（USER 2026-11-21，“按你的建议来”）。

## 结论

产出发布字节的代码已迁入 `tools/stickymd-package`（std-only，库加二进制 `stickymd-package`），`stickymd-smoke` 改为依赖它。
Platform Build Input Fingerprint 升为算法 v2：`tools/stickymd-smoke/` 除清单外不再计入，`tools/stickymd-package/` 整体计入两个平台。
发布 workflow、Windows 打包脚本与 `build-app.sh` 中产出字节的调用点只调用打包工具。编译器看不到的边，按 Develata 2026-11-22 的决定
（“改成闭合世界”）以闭合世界的清单检查：rustc 的 dep-info、固定清单、整份 workflow 的固定副本、原样的名字搜索与 Git 索引，不解析任何语法；
详见文末“Resolution（2026-11-22）：闭合世界”及其后各节，当前状态见最后一节。M5 的“R 上的源码测试”只写进了合同，核对程序未实现。

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
  由 Cargo metadata 与边界检查限于各成员计入且自成一体的 `build.rs`（workspace 中没有过程宏成员，二进制名唯一）。改为只构建 `stickymd-win` 是否会实际改变
  产品的 feature 合并没有比对；按 Cargo 规则可能改变，即可能是 Windows 行为变化，所以不在本次范围。

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


已验证（`5da88fe` 的工作树，macOS 27.2 arm64）：fmt、Clippy（workspace 的 macOS 目标与两个工具 crate 的 Windows 目标）通过；workspace 测试（不含在 macOS 上不编译的 Windows 宿主 `stickymd-win`）1850 项通过，
0 失败；`phase 00` PASS。`5da88fe` 本身尚未经过跨模型复审。

## 第四轮窄范围复审（`5da88fe`）的处理（2026-11-21）

用户要求“再审一轮”。复审判为不通过。这一轮不再逐个补漏洞，而是把边界规则收少、收严，使每一条都能完整检查：

- **打包 crate 不再以路径引入 Rust：** 任何写法的 `include!`、`#[path]` 与带 `path` 的 `cfg_attr` 一律拒绝（上一轮的递归检查随之删除），打包 crate 的
  Rust 就是它自己的 `.rs` 文件。数据 include 识别 raw identifier 与路径前缀（`r#include_str!`、`core::include_str!`），已跟踪但两平台都排除的目标被拒。
- **构建脚本：** 只能是成员目录下的 `build.rs`，该目录须计入 Windows 指纹（smoke、mac 目录中的构建脚本被拒）；字符串字面量中不得有 `..` 路径分量；
  引入的数据须被计入该脚本的每个平台计入。两个平台都计入每个成员的 `build.rs`，所以 Windows 宿主的构建脚本实际上不能引入自己的数据（目前也没有）。
- **名字：** 验证工具的包名与 crate 名在去掉注释后的代码中任何位置出现都被拒，不再区分字面量内外。
- **脚本：** 引用按调用方平台检查是否计入，macOS 不计入 `tools/release`，所以 macOS 的构建不能引用任何 PowerShell 脚本；点源须以绑定形式开头；块注释
  结束后的同行代码照常检查；扩展名须为小写 `.ps1`，文件名按 Git 的拼写判定；扫描按字符而非字节，多字节空白不会出错。
- **workflow：** 按 job 与步骤结构解析：job 直属的 `needs`、步骤自身的 `uses`（`- uses:` 列表项与步骤字段都认，环境变量块中的文字不算），缺
  `dependency-policy`、job 重复定义都被拒。合同与 workflow 头部注释写明依赖策略 job 在三个发布 job 之前。

打包 crate：Info.plist 的重名键检查改为在字节上进行（非 UTF-8 的 plist 交给 plutil 判定）；新增两个测试：保留目录的错误先于可执行文件与包计划
（与拆分前的顺序相同：先目录、再可执行文件、再计划），证据树中悬空的链接使写文件的命令拒绝且不创建输出目录。命令行中误给验证工具参数的提示
改为指名所属命令。

合同、P18-P10 的描述与锚点、本报告中关于 feature 合并的一句（原先前后矛盾）随之更新。

提交 `326f01a` 之后的自查又改了四处（同一轮，另行提交）：

- `governance/package_boundary.rs` 达到 635 行，超过 `AGENTS.md` 的 500 行审视线，按职责拆出 `package_boundary/scripts.rs`（PowerShell 引用）与
  `package_boundary/workflow.rs`（job 与步骤），搬动的函数体未改，测试没有搬动（路径规则另增了用例）。
- 构建脚本引入数据的条件一度写成“计入该脚本的每个平台都计入”；两个平台都计入每个成员的 `build.rs`，这与“两个平台都计入”等价，改回后者。
- 构建脚本字面量中的 `..` 原先只按 `/`、`\` 切分，`cargo:rerun-if-changed=../x` 会漏过；改为按非路径字符切分，并拒绝开头或 `=` 之后的绝对路径。
- Info.plist 只接受 `build-app.sh` 写出的 UTF-8 XML：UTF-16 或二进制形式中重复的键看不出来，plutil 会取其中一个值；新增 UTF-16、二进制与非 UTF-8
  字节的负例。

**本轮收紧的合同（需要 Develata 确认）：** 这些规则写进了 `docs/plan/11`，影响范围超出打包 crate 本身：Windows 不计入的成员目录
（`apps/stickymd-mac/`、`crates/stickymd-posix/`、`tools/stickymd-smoke/`）不能有构建脚本；Windows 宿主的 `build.rs` 不能引入自己目录中的数据，
也不能指名 `assets/` 等成员之外的路径；打包 crate 永远不能使用 `include!` 与 `#[path]`。目前的代码都满足（这三个目录没有 `build.rs`，
`apps/stickymd-win/build.rs` 在运行时读 `StickyMD.manifest`，不 include），将来需要时按合同修改的流程放宽。


已验证（本次提交的工作树，macOS 27.2 arm64）：fmt、Clippy（workspace 除 `stickymd-win` 外的 macOS 目标；两个工具 crate 的 Windows 目标）通过；
workspace 测试（不含 `stickymd-win`，它在 macOS 上不编译）1853 项通过，0 失败；`phase 00` PASS。未验证：Windows 上的编译与 PowerShell 包装测试、
真实 dispatch。`326f01a` 与本次提交都未经过跨模型复审。

## 第五轮复审（`326f01a`、`aedc98b`）：两份都不通过，需要 Develata 决定（2026-11-22）

用户要求“再审一轮”。按路径分两份跨模型复审：A 审边界检查器（`governance/package_boundary*`），B 审打包 crate、合同、workflow 与验收行。
两份都判为不通过：A 九项 BLOCKER、六项 SHOULD_FIX，B 五项 BLOCKER、四项 SHOULD_FIX。

**判断：词法检查这条路不收敛。** 检查器是开放世界的：它解析任意 Rust、PowerShell 与 YAML，再设法认出坏的写法。前五轮每轮都扩了解析面，
每次扩面都带来新的绕过。复审的 BLOCKER 标准（“验证代码影响发布字节而检查不失败”）是本报告作者在复审说明中写的，比合同承诺的
（词法检查，有意绕开由代码审查承担）更强，任何拒绝表都达不到。所以本轮只修与设计无关的项，其余停下来等决定。

**本轮已修（与设计无关）：**

- Info.plist：键元素须为 `<key>` + 普通名字 + `</key>`（无实体、CDATA、注释、属性或空白），因此重复的键按字节可数；缺键由结构判定（字节中没有
  该键才是旧 bundle），写了键却读不出顶层字符串即报错，不再把 plutil 的失败当作缺键。新增实体、CDATA、`<key >`、两个不同摘要、真实键加注释、
  损坏文档、声明为 UTF-16 的文档、完整二进制 plist 的负例，各自断言拒绝原因。（B 阻断 1、SHOULD_FIX 3）
- 打包 crate 的每个 target 须是检查实际读过的 `.rs` 文件（`path = "src/entry.in"` 被拒）；根 package 的清单 `Cargo.toml` 不再报错。（A B2 前半、A S4、B 阻断 2 前半）
- Syft 缓存写入前也经输出守卫，祖先中的链接不能把它引进保留存储。（B SHOULD_FIX 1）
- 顺序测试改用拆分前后都保留的目录，并补“缺可执行文件先于包计划”。（B SHOULD_FIX 2）
- 脚本：验证工具的 crate 名 `stickymd_smoke` 与包装函数名按不分大小写匹配；同一行的每个点源分别检查，仓库路径形式的点源允许。（A B8 ①③、B 阻断 4 ①②）
- 合同措辞对齐实现：构建脚本的绝对路径只检查开头与 `=` 之后；PowerShell 规则只管 `.ps1` 引用；报告中 `.PS1` 与“测试文件不动”两句改正。
  README、阶段任务表、分类器注释与 macOS runner 一句同步。（B 阻断 3、4 ③，B SHOULD_FIX 4，B NICE_TO_HAVE）
- **B 阻断 5（CLI 提示文字是 Windows 行为变化）不成立：** `main` 上没有 `stickymd-package`（`git ls-tree -d main tools/stickymd-package` 为空），
  这两条提示属于本分支新增的二进制；与 `main` 相比，正常打包路径、拒绝结果与退出码都未变。

**未修，留给决定：** `build-package` 中加入 `cargo test --workspace` 不被发现（A B1）；成员目录按单个探针判定是否计入（A B3）；Unicode 标识符与
Rust 特有空白（A B4）；`use` 给 include 宏起别名、宏展开处与定义处目录不同（A B5、B 阻断 2 后半）；经排除区符号链接的 `..`（A B6）；
引号中的 `<#`（A B7）；`ps1_tokens` 不以 `|` 分隔、点后用制表符（A B8 ② 与 ③ 余项）；`working-directory` 与 `Set-Location`（A B9）；YAML 缩进与
`needs` 的列表写法（A S1）；workflow 检查比合同的运行要求窄（A S2）；大写 `.PS1` 的正例（A S3）；shebang 与转义字符的误报（A S5）；部分负例
没有只破坏一个条件（A S6）；`cfg_attr` 无界递归（A NEEDS_VERIFICATION）。

**闭合世界方案的可行性（实测数量）：** 打包 crate 中 `include_str!` 只有 2 处，都是 `include_str!("package_readme.txt")`；`cfg_attr` 2 处，都是
`allow(dead_code)`；两个 `build-package` job 的 `run`/`uses` 步骤分别为 8 与 11 个；打包 crate 的注释中提到验证工具名 8 处，Windows 产出字节的
四个脚本中 0 处；打包 crate 有 14 个文件含非 ASCII 字符（是否都在注释与字面量中未核对）。

**选项：**

1. **闭合世界（推荐）：** 检查改为精确清单：固定的 include 位置、固定的 `build-package` 步骤文本、固定的脚本集合，清单之外一律拒绝；注释与字面量之外
   不允许非 ASCII，验证工具名连注释中也不允许，两个注释词法器随之删除。合同的“边界的证明”按实际执行的规则重写，复审标准与合同对齐。代价：改
   `build-package` 的步骤或新增 include 都要同时改清单（清单本身在 `stickymd-smoke` 中，不计入指纹，但改动可见、可审）。
2. **保留词法守卫：** 合同写明它是尽力而为的守卫，拦住常见写法；完整性由代码审查承担；复审不再按“任何依赖都须被检查拦住”的标准判定。

**一并待确认的三项收紧**（第四轮写入合同，尚未得到确认）：Windows 不计入的成员目录（`apps/stickymd-mac/`、`crates/stickymd-posix/`、
`tools/stickymd-smoke/`）不能有构建脚本；Windows 宿主的 `build.rs` 不能引入自己目录中的数据，也不能指名成员之外的路径；打包 crate 永远不能
使用 `include!` 与 `#[path]`。在决定之前，这三项与“边界的证明”的现行文字都按待批准的提案对待，本分支不合入。


已验证（本次提交的工作树，macOS 27.2 arm64）：fmt、Clippy（workspace 除 `stickymd-win` 外的 macOS 目标；两个工具 crate 的 Windows 目标）通过；
workspace 测试（不含 `stickymd-win`）1856 项通过，0 失败；`phase 00` PASS。未验证：Windows 上的编译与 PowerShell 包装测试、真实 dispatch。
本次提交未经跨模型复审；在上面的决定作出之前不再发起复审。

## Resolution（2026-11-22）：闭合世界

Develata 的回复：“改成闭合世界”。

合同 `11` #platform-artifact-reuse 的“边界的证明”据此重写为清单，每条规则只用与固定清单或固定副本的逐字比较、对全文的原样搜索、Cargo 与 rustc
自己报告的事实、Git 索引中的路径与模式；不再解析 Rust、PowerShell 或 YAML 的语法，旧的 Rust 词法器、PowerShell 注释与绑定规则、按缩进取字段的 YAML 读取一并删除。
写合同前核实的事实：

- rustc 的 dep-info 登记编译单元的本地源文件与文件依赖（不逐项列出标准库等二进制依赖）：在一个临时 crate 上，外部模块、`#[path]`、`include!`、经 `use core::include_str as inc` 起别名的
  `inc!`、宏展开中的 `include_str!` 与 `include_bytes!("../../outside/e.bin")` 都出现在 dep-info 中，后者按原样写为 `src/../../outside/e.bin`。
  打包 crate 本身的 dep-info 是 workspace 相对路径，非 `.rs` 的只有两个 `package_readme.txt`。
- 固定的 macOS 发布构建只选择 `stickymd-package`（`build-app.sh` 与 workflow 中的 `cargo run`）与 `stickymd-mac`（`build-app.sh` 的 `cargo build`），
  二者的依赖都不含 `stickymd-win`，所以 Windows 宿主的构建脚本只在 Windows 的 workspace 构建中运行。
- Windows 产出字节的脚本取 8 个的保守清单：6 个从固定的 `build-package` job 可达，`generate-third-party-notices.ps1` 是独立入口，
  `archive-facts.ps1` 由打包库为校验与晋升启动；它们、`build-app.sh` 与 `apps/stickymd-win/build.rs` 中不分大小写都没有验证工具的名字。打包 crate 中
  编译器读过的文件里有 8 处注释提到验证工具（按 `4e1ad43` 计数），已在 `33c3b50` 中改写。
- 两个发布 workflow 都没有 YAML 锚点或别名；`build-package` job 不使用本地 action，只受 workflow 头部（含 workflow 级 `env`）与它自身文本影响。

第四轮写入合同、一并待确认的三项收紧，在闭合世界下的去向（不是逐项批准，而是被新规则取代）：

- “Windows 不计入的成员目录不能有构建脚本”变为清单成员资格：workspace 的构建脚本恰为 `apps/stickymd-win/build.rs`，要加一个须先改清单。
- “Windows 宿主的 `build.rs` 不能引入自己目录中的数据”取消：它只在 Windows 构建中运行，成员目录计入 Windows 指纹，只保留全文中没有
  `../`、`..\` 与 `".."` 三个字面量（`0..SIDE` 这类范围写法不受影响）；它自己的模块与引入的文件读什么归入剩余责任。
- “打包 crate 永远不能使用 `include!` 与 `#[path]`”取消：编译器读入的文件由 dep-info 列出，只要都在打包 crate 内、被跟踪且两平台都计入即可。

剩余责任写进合同：清单中的文件在运行时做什么不能静态证明，名字搜索是守卫，其余由代码审查承担。复审标准随之改为：合同中写明的规则没有执行、
清单可以不被察觉地漂移、在真实仓库上失败，才是 BLOCKER。

### 合同复审（`4e1ad43`）的处理

跨模型复审判为不通过，十项 BLOCKER。处理：

- 测试 target 不在 lib 与 bin 的 dep-info 中（1）：规则只覆盖构建发布字节的 target，测试、bench、example 不进入发布字节。
- dep-info 的范围（2）与 profile（3）：结论收窄为“rustc 登记的本地源文件与文件依赖”，只证明 Rust 前端的输入；检查按固定 workflow 实际使用的
  dev 与 release 两个 profile 各运行一次；标准库与链接阶段的输入来自固定的工具链。
- 构建脚本（4、5）：报告与合同统一为三个字面量；构建脚本自己的模块与引入的文件归入剩余，名字搜索扩大到 `crates/` 与 `apps/` 下的全部跟踪文件，
  覆盖它们所在的目录；“只构建 `stickymd-mac`”改为按固定 job 实际选择的包陈述。
- 脚本清单（6）：写明是保守清单，6 个可达，另 2 个保守纳入：`generate-third-party-notices.ps1` 是独立入口，`archive-facts.ps1` 由打包库直接启动。
- workflow（7、8）：逐行与锚点规则不能证明 YAML 节点归属，改为每个发布 workflow 的全文等于固定副本；名字搜索覆盖副本中 `build-package` 一段。
- Git 索引（9）：列为第四类事实，检查在被检提交的干净检出上运行。
- 新成员落在排除区（10）：每个 workspace 成员的清单须两个平台都计入。
- SHOULD_FIX：剩余责任补上“已审文件在运行时选择读取的被排除文件”“检查器与固定副本本身在验证工具中”；复审标准补上“规则与它声称证明的性质
  不等价”。分类器注释与 P18-P10 随实现更新。

产品 crate 读取 `docs/` 等其他被排除区域的文件不属于本边界（`docs/` 的排除早于本次改动，来自 Phase 18 的分类器），在合同的剩余中写明，留给后续。

复审标准（补充）：合同中写明的规则没有执行、清单可以不被察觉地漂移、规则与它声称证明的性质不等价、在真实仓库上失败，才是 BLOCKER。

### 合同复审（`cdd2ff6`）的处理

窄范围复审判为不通过，五项 BLOCKER。其中两项针对 `cdd2ff6` 时尚未落地的实现，已在 `b69f5b1` 中完成：`crates/` 与 `apps/` 下两处提到验证工具的
注释已改写；双 profile、整份 workflow 副本、产品目录扫描与成员清单分类都已接入。另三项改合同：

- 编译期由环境变量选择文件（`include!(concat!(env!("GITHUB_JOB"), ".rs"))`）：dep-info 中不得有环境读取（`# env-dep:`）；打包 crate 现在没有。
- 链接输入：标准库归固定的 Rust 工具链，原生链接器、SDK 与导入库（`Bcrypt.lib`）归平台构建环境，不由本检查证明。
- 构建脚本模块引入的、位于 `apps/` 与 `crates/` 之外的文件：名字搜索只覆盖这两个目录，其余归入剩余。三个字面量不扩大到整个成员目录：
  `apps/stickymd-win/` 中有 4 个文件合法地含 `../`（清单中的路径依赖与产品代码）。

SHOULD_FIX 与 NICE_TO_HAVE 已改：两个保守纳入脚本的调用方向分别写明；“8 处注释”标明计数所在的提交。

### 合同复审与实现审查（`779dd19`）的处理

两份跨模型审查都判为不通过：合同窄复审确认前一轮五项已关闭，另提四项 BLOCKER；实现审查九项 BLOCKER。两者大部分重叠，处理如下（合同与实现
同轮修改）：

- **编译单元的身份（合同 B2、实现 1、5）：** 每次检查新建私有目录，同时作为 Cargo 的 target 与 build 目录；每个 profile 中，打包 crate 的每个 lib 与
  bin target 恰有一个新编译（`fresh: false`）的单元，它的 dep-info 登记它自己的入口；任一缺失、重复或复用即拒绝。带 `required-features` 而被跳过的
  bin、空的 `.d`、旧的缓存都在这里失败。
- **dep-info 的读取（实现 2）：** 规则行与逐文件行须列出同一组文件；无法归类的行、以 `#` 开头但不是环境读取的行都按文件处理并参与这一核对。
- **反斜杠（合同 B3、实现 3）：** 只在 Windows 上视为分隔符，其他平台上含 `\` 的路径被拒。
- **清单可悄悄缩短（实现 4）：** 测试按合同原文核对脚本清单、构建脚本清单与 profile；workflow 副本须恰为分类器计入的 `.github/` 文件；新增只在
  dev 或只在 release 下才登记的探针文件。
- **跨 job 的编译配置（合同 B1、实现 6）：** 检查只能观察本 job 的编译，跨 job 的有效配置相同写为合同中的前提；打包 crate 不得声明 feature，
  workspace 的 feature 合并因此改变不了它。审查建议的另一种做法（在 `build-package` 中保留 dep-info 并在 `verify-artifact` 中检查）需要改动发布
  artifact 的组成，本轮不采用。
- **成员清单（实现 7）：** 清单须是 Git 索引中的普通文件，按索引中的路径分类。
- **二进制名与构建脚本归属（实现 8）：** 二进制名按 Unicode 小写比较；构建脚本清单改为成员与路径成对，且没有成员声明依赖 `stickymd-win`。
- **脚本的编码与名字边界（合同 B4、实现 9）：** 清单中的脚本须是合法 UTF-8；`.ps1` 名前一个字节须是 ASCII 且不是路径分隔符。
- **SHOULD_FIX：** Cargo 一律取 PATH 上的 `cargo`（rustup 按 `rust-toolchain.toml` 选择工具链），不再取继承的 `CARGO`；拒绝环境读取时只报告变量名；
  编译失败时附上 stdout 中的编译错误。非 UTF-8 的无关索引路径仍使检查失败（偏保守，未改）。

### 当前状态（`30c26b2` 之后，2026-11-22）

自查补了一条：前提中“固定 workflow 不设置这些变量”原先没有检查，现在是对两份固定副本的原样搜索（不分大小写），出现改变编译的变量即拒绝。

**已验证（macOS 27.2 arm64，本节所在提交的工作树）：** fmt；Clippy（workspace 除 `stickymd-win` 外的 macOS 目标；两个工具 crate 的 Windows
目标）；workspace 测试（不含 `stickymd-win`）全部通过；`phase 00` PASS；真实仓库上闭合世界检查通过（dev 与 release 两个 profile 在新建私有目录中
编译，44 个登记文件、无环境读取）；第三方声明的两个摘要与迁移前相同（`33c3b50` 上复核：Windows `360d0993…`、macOS `8c7875b2…`）。

**推断：** 两处产品注释的改写（`b69f5b1`）不新增失效的收据：macOS 产品输入指纹覆盖 `apps/stickymd-mac/` 全部文件，本分支在 `fc6fc09` 已改
`build-app.sh`；Windows 的模块收据已因改 `11` 全部失效。仓库没有 Dependabot 或 Renovate 配置，所以整份 workflow 的固定副本不会被自动更新打断。

**未验证：** dep-info 读取与私有目录编译从未在 Windows 上运行（Windows 路径分隔符、`.rmeta` 命名、`CARGO_BUILD_BUILD_DIR` 的行为），第一次
Windows `self-check` 就是证据；Windows 上的 PowerShell 包装测试；三个 job 的真实 dispatch；`30c26b2` 及其后的提交尚未经过跨模型复审。

**留给后续：** 产品 crate 读取 `docs/` 等其他被排除区域的文件不属于本边界；`docs/` 的排除来自 Phase 18 的分类器，早于本次改动。

### 最后一轮窄复审（`2107a55`）的处理

合同与检查器各一份跨模型复审，都判为不通过；前一轮的 B2–B4 与实现九项中的大部分确认关闭。按计划只修具体的代码与措辞缺陷，然后停下：

- 合同与代码清单单向核对（两份复审都指出）：测试改为按唯一标题截取各子条，脚本清单、构建脚本对、profile 与改变编译的字面串都做双向集合比较。
- 改变编译的变量（合同）：措辞收窄为“固定副本不含下列字面串”，其他拼写与变量、各调用点对环境的处理明确留在前提中；补 `RUSTC=` 的单独负例。
- 经非成员的传递依赖（合同）：改为原样检查 `Cargo.lock`，含所属成员名的行只能是它自己的 `name = "stickymd-win"`；直接依赖的检查随之删除。
- 二进制名（合同）：限定为 ASCII 字母、数字、`_`、`-`，不分大小写判重（Windows 按卷的大写表比较，σ 与 ς 会相撞）。
- Git 索引中的反斜杠路径（检查器）：一律拒绝。
- 显式 `crate-type`（检查器）：`rlib` 等库的各种 kind 都按 lib 匹配。
- 文件起点的脚本名（两份）：合同写明文件起点也是合法边界，补正例。
- SHOULD_FIX：环境读取先于其他错误报告，不回显值；编译失败时即使 stdout 混有非 JSON 行也保留编译错误；补 UTF-16 脚本（带与不带 BOM）的负例。

**交给 Develata 的前提问题（未改，不评级）：** macOS 的 `build-app.sh` 以 `env -i` 清空环境后运行打包工具（`app-inputs`），而检查器与 workflow
中的其他打包调用继承环境。若 runner 环境带有改变编译的变量，同一 job 里两次编译打包 crate 的配置就不同，检查器观察到的是继承环境的那一次。
合同已把“各调用点对环境的处理”写入前提；要把它变成检查，可选：在检查器中再以与 `build-app.sh` 相同的清空环境编译一次，或让两处调用使用同一环境。

**仍未验证：** 闭合世界检查从未在 Windows 上运行；本节所在提交未经复审。


### Resolution（2026-11-22）：`env -i` 调用点

Develata 的回复：“保留为前提”。

`build-app.sh` 以 `env -i` 清空环境后运行打包工具、检查器与 workflow 中的其他调用继承环境，这一差异留在合同 `11` #platform-artifact-reuse 的
“前提”中（“各调用点对环境的处理”），不改成检查。合同文字不变。

## Windows 输出守卫的修复与收口（2026-11-22）

**来由：** `feat/product-input-closure`（基于本分支 `51ba15a`）的 CI [run 45028821289](https://github.com/Develata/StickyMD/actions/runs/45028821289)
中，从本分支迁来的打包代码第一次在 Windows 上运行，`stickymd-package` 的两个测试失败：`atomic.rs` 的
`ambiguous_spellings_are_refused_before_writing`（守卫放行了 `NUL:`）与 `commands.rs` 的 `release_writers_cannot_target_qualification_storage`
（守卫放行了 `dist/evidence\..\SHA256SUMS.txt`，命令随后以 os error 3 失败）。同一次 run 中 Windows 的 “governance contracts” 任务通过，
即闭合世界的边界检查已在 Windows 上运行过一次。

**根因：** 拆分（`fc6fc09`）时输出守卫改为先以 `std::path::absolute` 解析再检查。Windows 上它调用 `GetFullPathNameW`，会折叠 `..`
（守卫看不到路径经过 `dist/evidence`）并把 `NUL:` 映射为设备路径 `\\.\NUL`（拼写规则看不到设备名）。macOS 上 `absolute` 保留 `..`，
所以那里不出错。这两个测试断言本身是拆分时新加的，拆分前守卫也用 `absolute`，但那时没有测试覆盖这两种输入。

**修复（最终形态，`9ca7942`）：** 守卫与写入器用同一个解析函数 `std::path::absolute`；拼写规则先作用在调用方给出的原样路径上
（与写入器相同），再作用在解析后的路径上；然后检查解析后的路径本身，以及它的每个词法祖先（`create_dir_all` 正是逐个创建这些祖先），
只跳过仅由盘符与根组成的祖先；`path_identity::normalize` 只在 Windows 上把 `\` 当作分隔符；证据树中指向根（`/`、整个盘）的链接
使其下的一切都受保护。中间的 `fb97b82`、`47474cc`、`d37be8b`、`b635e98`、`654a852`、`99629ee`、`44df73b` 是按跨模型复审逐轮修正的过程，
其中手写 Win32 路径解析的尝试（`47474cc`–`d37be8b`）已全部删除。

**Resolution：** Develata 的回复：“接受，就此收口”。收口标准：

- 守卫**错误放行**本应拒绝的路径（真漏洞）：仍须修复，并只对该修复做窄复审。
- 守卫只**过度拒绝**发布 workflow 从不会传入的特殊拼写或环境：在报告中记为已知的保守（fail-closed）行为，不再改代码、不再复审。

**按此标准记录的已知过度拒绝与待验证项：**

- 输出为裸盘符 `C:`，而该盘的当前目录恰好名为 `…\stickymd\qualification-ledger` 之类的协调者存储片段时被拒（第 7 轮）。
- 输出为服务器名后连写两个分隔符的 `\\server\\share\…` 时，祖先中的 `\\server` 也被检查；只有证据树中的链接恰好指向它时才会误拒（待在 Windows 上验证）。
- 当前目录本身是 verbatim 路径（`\\?\…`）时的解析行为与写入器一致（同一个 `absolute`），未在真实 runner 上观察。
- 证据树中指向整个盘根的链接（`9ca7942`）只在 Unix 上有测试，Windows 的规范形式（`//?/d:/`）按源码推导覆盖。
- 裸盘符的行为没有在 Windows 子进程中做回归测试（测试无法安全地改变并行测试的当前目录）；由 `check_ancestors` 首行无条件检查解析后的路径
  与 `checked_ancestors("C:")` 为空的测试共同支撑。

复审：`44df73b`（裸盘符）与 `9ca7942`（根链接）的窄复审均为 PASS。

**已验证（macOS 27.2 arm64）：** fmt；Clippy（macOS 目标与 `x86_64-pc-windows-msvc` 目标，含测试代码）；`stickymd-package` 与 `stickymd-smoke`
的测试全部通过（707 项）；共享测试从 `/` 运行也通过。**未验证：** Windows 上的实际运行（仅 Windows 编译的测试从未运行），留给下一次 CI。
