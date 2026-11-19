# Phase 18 Acceptance Matrix

本矩阵验证 macOS 安装版与公开分发的实现（[`11` Phase 18 节](../plan/11_testing_and_release.md#phase-18-macos-distribution)，
[`05` Note Root 判定](../plan/05_document_persistence.md#note-root)，[`11` macOS 发布物](../plan/11_testing_and_release.md#macos-release-artifact)、
[跨提交沿用](../plan/11_testing_and_release.md#platform-artifact-reuse)）。它不是资格化 suite，不产生 exact-candidate 证据，也不进入 Windows 的
全阶段 readiness。未运行的项保持 `NOT TESTED`。

每版的 macOS 发布证据（候选收据、虚拟机运行、R03、USER waiver）按 [`11` macOS 发布门](../plan/11_testing_and_release.md#macos-release-gate)
记在该版的候选报告与 ignored `dist/evidence/`，不写进本表：`docs/acceptance-cases/` 下的改动会使 Windows 全部资格化模块的收据失效。

## L 安装版便签位置（M2）

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P18-L01 | 位置判定纯函数：bundle 与开发构建；`/Applications` 与 Data 卷别名；`~/Applications` 不存在、悬空、指向别处；命中优先；其他错误无法判定；子文件夹与 `/System/Applications` 不是候选；随机化路径为便携版 | Automated | `note_root::placement::tests`（[placement.rs:137](../../apps/stickymd-mac/src/note_root/placement.rs#L137) 命中优先，[:164](../../apps/stickymd-mac/src/note_root/placement.rs#L164) 不存在、悬空与别处为便携，[:180](../../apps/stickymd-mac/src/note_root/placement.rs#L180) 无命中时的错误无法判定）与 `note_root::tests`（[tests.rs:136](../../apps/stickymd-mac/src/note_root/tests.rs#L136) 按身份比较、经链接的别名与 `~/Applications` 链接，[:157](../../apps/stickymd-mac/src/note_root/tests.rs#L157) 子文件夹、下载目录与随机化路径为便携且不建任何目录，[:190](../../apps/stickymd-mac/src/note_root/tests.rs#L190) 读不到的候选与未知家目录） | AUTOMATED PASS |
| P18-L02 | 键：bundle 目录名去掉 `.app` 后做 NFC 规范化；为空、`.`、`..` 或无法无损转成 UTF-8 时无法判定 | Automated | [placement.rs:199](../../apps/stickymd-mac/src/note_root/placement.rs#L199) 去掉 `.app` 后 NFC、保留大小写；[:214](../../apps/stickymd-mac/src/note_root/placement.rs#L214) 空、`.`、`..` 与非 UTF-8 被拒；经真实入口：[tests.rs:394](../../apps/stickymd-mac/src/note_root/tests.rs#L394) 名为 `.app` 的 bundle 报错且没有任何写入 | AUTOMATED PASS |
| P18-L03 | 家目录取自用户数据库（`getpwuid_r` 查当前 uid），不读 `$HOME` | Automated | `stickymd-posix` 的 [user.rs:90](../../crates/stickymd-posix/src/user.rs#L90) 与 shell 的 `~user` 一致；[:98](../../crates/stickymd-posix/src/user.rs#L98) 子进程里把 `$HOME` 改掉后结果不变 | AUTOMATED PASS |
| P18-L04 | 安装版首次启动建立 `StickyMD/<键>/note/` 布局；起点 `Application Support` 不存在或不是目录时提示后退出，不去创建 | Automated | [tests.rs:110](../../apps/stickymd-mac/src/note_root/tests.rs#L110) 两个位置都得到 `StickyMD/StickyMD`，两级目录权限不含组与其他人，加锁前只有这两级；[:217](../../apps/stickymd-mac/src/note_root/tests.rs#L217) `Application Support` 不存在时报错且不创建；布局由 [paths.rs:241](../../apps/stickymd-mac/src/paths.rs#L241) 相对句柄建立 | AUTOMATED PASS |
| P18-L05 | 已存在的 `StickyMD/` 或 `<键>/` 是符号链接、不是目录或属于他人时提示后退出，其中没有新增任何东西 | Automated | [tests.rs:231](../../apps/stickymd-mac/src/note_root/tests.rs#L231) 经真实入口：`StickyMD/` 是符号链接、`<键>` 是文件时报错，整棵树不变。属于他人：只在同一测试中对 `check_level` 单独断言（非 root 用户建不出他人的目录），生产路径对每一级都调用它；posix 层 [child_directory.rs:114](../../crates/stickymd-posix/src/child_directory.rs#L114) | AUTOMATED PASS |
| P18-L06 | 输掉锁的进程自身不创建任何文件；单个进程在创建目录后中断，只留下空目录，下次启动照常接受 | Automated | [tests.rs:266](../../apps/stickymd-mac/src/note_root/tests.rs#L266)：预先只有空的两级时照常启动；输掉锁的副本在解析与取锁之后，家目录下的整棵树不变。`main.rs:69` 在取锁结果为第二实例时立即返回，可写检查与布局都在其后（代码审查，未单独运行 `main`） | AUTOMATED PASS |
| P18-L07 | 实例锁取在 Note Root 句柄上；启动阶段与运行期共用同一个 `note/` 句柄，不再按路径重开 | Automated | 实例锁：[instance.rs:259](../../crates/stickymd-posix/src/instance.rs#L259) 锁跟随句柄而非路径；启动：[bootstrap.rs:281](../../apps/stickymd-mac/src/startup/bootstrap.rs#L281) 两个目录各放不同的正文与配置，启动读到的是已打开目录里的那一份；正文重建只在已锁定的 Note Root 内：[storage.rs:638](../../apps/stickymd-mac/src/persistence/storage.rs#L638)，启动期间不重建：[:667](../../apps/stickymd-mac/src/persistence/storage.rs#L667) | AUTOMATED PASS |
| P18-L08 | 同一个键的两个副本互斥（发布门）：覆盖两个安装位置，以及大小写敏感与不敏感两种卷 | Automated | [tests.rs:282](../../apps/stickymd-mac/src/note_root/tests.rs#L282) 系统与家目录两个位置、同名与只差大小写（默认的不区分大小写卷）都是第二实例；[:323](../../apps/stickymd-mac/src/note_root/tests.rs#L323)（`--ignored`，区分大小写的 APFS 映像，2026-11-19 本机通过）只差大小写得到两张 | AUTOMATED PASS |
| P18-L09 | 不同键的两个副本可以同时运行 | Automated | [tests.rs:282](../../apps/stickymd-mac/src/note_root/tests.rs#L282) 中 `StickyMD copy` 与 `StickyMD` 同时为第一实例 | AUTOMATED PASS |
| P18-L10 | P16-S11、P16-S14 在安装版下照旧成立 | Automated | S14：[tests.rs:372](../../apps/stickymd-mac/src/note_root/tests.rs#L372) 安装版替换 `note/` 后配置仍写入原目录；删除后配置保存失败，新目录里没有写入。S11 的监视器只按 Note Directory 的路径注册，与模式无关，沿用 `platform::macos::file_watch::tests` | AUTOMATED PASS |
| P18-L11 | 不可写提示保留“当前目录不可写”前缀；bundle 内的便携版建议拖进“应用程序”或移动整个文件夹，安装版指明 Note Root | Automated | [paths.rs:285](../../apps/stickymd-mac/src/paths.rs#L285) 三种模式的提示都以“当前目录不可写”开头；[tests.rs:405](../../apps/stickymd-mac/src/note_root/tests.rs#L405) 安装版建不出目录时同样带这个前缀；[paths.rs:183](../../apps/stickymd-mac/src/paths.rs#L183)（`--ignored`，exFAT）卷提示 | AUTOMATED PASS |
| P18-L12 | 唤醒记录：显式启动第二个进程、对同一 bundle 再次打开、在访达里双向打开两个不同键的安装副本，分别记录 PID、Note Root 与显示的窗口（只记录，不作门） | Manual | 尚未运行 | NOT TESTED |
| P18-L13 | Windows 不变：`apps/stickymd-mac/` 与 `crates/stickymd-posix/` 不进入 Windows 资格化模块与 Windows CI lane；共享 crate 不改 | Automated | [fingerprint/tests.rs:637](../../tools/stickymd-smoke/src/qualification/module_ledger/fingerprint/tests.rs#L637) 与 [ci/selection/tests.rs:89](../../tools/stickymd-smoke/src/ci/selection/tests.rs#L89)；`cargo check -p stickymd-posix -p stickymd-mac --target x86_64-pc-windows-msvc` 通过（两者在 Windows 上为空）；四个共享 crate 与 `apps/stickymd-win` 无改动 | AUTOMATED PASS |

## P 发布流水线（M3）

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P18-P01 | 第三方声明按平台的依赖图生成：Windows 的输出与改动前相同；macOS 以 `stickymd-mac` 为根、`aarch64-apple-darwin` 为目标 | Automated | 尚未实现 | NOT TESTED |
| P18-P02 | macOS 包：ZIP 成员白名单、`StickyMD/StickyMD.app` 与说明文件、README 的 Source commit 行与“macOS 27+、Apple Silicon、更早版本未验收”、不含 `note/`；解出后签名完整、签名方式为 ad-hoc 与架构校验 | Automated | 尚未实现 | NOT TESTED |
| P18-P03 | macOS SBOM 与 `SHA256SUMS-macos-arm64.txt`：两行、文件名白名单、哈希与文件一致 | Automated | 尚未实现 | NOT TESTED |
| P18-P04 | 本地晋升：按 attempt 查询结论、按 artifact id 下载、与用户副本逐字节比对；任何校验失败都不半更新 staging，也不写 receipt；receipt 不含绝对路径与发布提交 | Automated | 尚未实现 | NOT TESTED |
| P18-P05 | Platform Build Input Fingerprint：在 Git tree 上计算；排除表逐项；未分类路径与每个成员的 `Cargo.toml` 算作输入；沿用要求祖先关系与指纹相同两个条件 | Automated | 尚未实现 | NOT TESTED |
| P18-P06 | 构建来源 S 与发布提交 R 分开核对：远端记录、Promote、resolver 与 readiness 接受可沿用的 S；同一 artifact 原样重新 Promote 得到同一 receipt，按其摘要绑定的人工日志仍可用；沿用记录写明 S、R 与两侧指纹 | Automated | 尚未实现 | NOT TESTED |
| P18-P07 | 晋升 workflow 的附件集合比对：缺件、多件、残留旧附件、哈希不符都拒绝 | Automated | 尚未实现 | NOT TESTED |
| P18-P08 | `release-macos` workflow 在 `xcode-27` 上选定 Xcode 27.2 构建，构建记录齐全，artifact 名带 attempt | Manual | 需要 push 与 USER 授权的 dispatch | NOT TESTED |
| P18-P09 | 对真实候选运行晋升 workflow 的 TAG、DRAFT、PUBLISH | Manual | 需要 USER 分别授权 | NOT TESTED |

## D 下载路径（M4，`mac-test`，候选包）

对同一份候选 ZIP 的独立下载副本，每一种都记录实际运行路径、Note Root 身份、提示与退出码，并确认另一个 Note Root 没有写入；成功启动的，
保存后重开一次（[安装版报告](../report/2026-11-19-macos-installed-note-location-r2.md)第 8 节）。

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P18-D01 | 解压后拖进 `/Applications` 再打开：安装版，便签在 `~/Library/Application Support/StickyMD/StickyMD/note/` | Manual | 尚未运行 | NOT TESTED |
| P18-D02 | 拖进 `~/Applications` 再打开：安装版，同一个键 | Manual | 尚未运行 | NOT TESTED |
| P18-D03 | 把外层文件夹移到别处再打开：便携版，便签在 app 旁边 | Manual | 尚未运行 | NOT TESTED |
| P18-D04 | 在解压原地打开：区分被 Gatekeeper 拦下、随机化后提示退出（不写文件）、未随机化而正常以便携版启动三种情形 | Manual | 尚未运行 | NOT TESTED |
| P18-D05 | 真实下载路径：浏览器下载、访达首开（系统拦下时才在“隐私与安全性”中“仍要打开”，并记录是否被拦）、保存、退出后重开；在第一个公开包的候选上做一次 | Manual | 尚未运行 | NOT TESTED |
