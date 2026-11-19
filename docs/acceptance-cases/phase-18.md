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
| P18-L01 | 位置判定纯函数：bundle 与开发构建；`/Applications` 与 Data 卷别名；`~/Applications` 不存在、悬空、指向别处；命中优先；其他错误无法判定；子文件夹与 `/System/Applications` 不是候选；随机化路径为便携版 | Automated | 尚未实现 | NOT TESTED |
| P18-L02 | 键：bundle 目录名去掉 `.app` 后做 NFC 规范化；为空、`.`、`..` 或无法无损转成 UTF-8 时无法判定 | Automated | 尚未实现 | NOT TESTED |
| P18-L03 | 家目录取自用户数据库（`getpwuid_r` 查当前 uid），不读 `$HOME` | Automated | 尚未实现 | NOT TESTED |
| P18-L04 | 安装版首次启动建立 `StickyMD/<键>/note/` 布局；起点 `Application Support` 不存在或不是目录时提示后退出，不去创建 | Automated | 尚未实现 | NOT TESTED |
| P18-L05 | 已存在的 `StickyMD/` 或 `<键>/` 是符号链接、不是目录或属于他人时提示后退出，其中没有新增任何东西 | Automated | 尚未实现 | NOT TESTED |
| P18-L06 | 输掉锁的进程自身不创建任何文件；单个进程在创建目录后中断，只留下空目录，下次启动照常接受 | Automated | 尚未实现 | NOT TESTED |
| P18-L07 | 实例锁取在 Note Root 句柄上；启动阶段与运行期共用同一个 `note/` 句柄，不再按路径重开 | Automated | 尚未实现 | NOT TESTED |
| P18-L08 | 同一个键的两个副本互斥（发布门）：覆盖两个安装位置，以及大小写敏感与不敏感两种卷 | Automated | 尚未实现 | NOT TESTED |
| P18-L09 | 不同键的两个副本可以同时运行 | Automated | 尚未实现 | NOT TESTED |
| P18-L10 | P16-S11、P16-S14 在安装版下照旧成立 | Automated | 尚未实现 | NOT TESTED |
| P18-L11 | 不可写提示保留“当前目录不可写”前缀；bundle 内的便携版建议拖进“应用程序”或移动整个文件夹，安装版指明 Note Root | Automated | 尚未实现 | NOT TESTED |
| P18-L12 | 唤醒记录：显式启动第二个进程、对同一 bundle 再次打开、在访达里双向打开两个不同键的安装副本，分别记录 PID、Note Root 与显示的窗口（只记录，不作门） | Manual | 尚未运行 | NOT TESTED |
| P18-L13 | Windows 不变：`apps/stickymd-mac/` 与 `crates/stickymd-posix/` 不进入 Windows 资格化模块与 Windows CI lane；共享 crate 不改 | Automated | 尚未实现 | NOT TESTED |

## P 发布流水线（M3）

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P18-P01 | 第三方声明按平台的依赖图生成：Windows 的输出与改动前相同；macOS 以 `stickymd-mac` 为根、`aarch64-apple-darwin` 为目标 | Automated | 尚未实现 | NOT TESTED |
| P18-P02 | macOS 包：ZIP 成员白名单、`StickyMD/StickyMD.app` 与说明文件、README 的 Source commit 行、不含 `note/`；解出后签名与架构校验 | Automated | 尚未实现 | NOT TESTED |
| P18-P03 | macOS SBOM 与 `SHA256SUMS-macos-arm64.txt`：两行、文件名白名单、哈希与文件一致 | Automated | 尚未实现 | NOT TESTED |
| P18-P04 | 本地晋升：按 attempt 查询结论、按 artifact id 下载、与用户副本逐字节比对；任何校验失败都不半更新 staging，也不写 receipt；receipt 不含绝对路径与发布提交 | Automated | 尚未实现 | NOT TESTED |
| P18-P05 | Platform Build Input Fingerprint：在 Git tree 上计算；排除表逐项；未分类路径算作输入；沿用要求祖先关系与指纹相同两个条件 | Automated | 尚未实现 | NOT TESTED |
| P18-P06 | Windows 候选接受可沿用的构建来源；同一 artifact 原样重新 Promote 得到同一 receipt | Automated | 尚未实现 | NOT TESTED |
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
| P18-D05 | 真实下载路径：浏览器下载、访达首开时在“隐私与安全性”中“仍要打开”、保存、退出后重开；在第一个公开包的候选上做一次 | Manual | 尚未运行 | NOT TESTED |
