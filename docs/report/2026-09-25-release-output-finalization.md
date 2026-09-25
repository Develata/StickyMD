# 2026-09-25 发布工具输出收尾

## 背景与授权

本记录继续 [release CLI 迁移](2026-09-22-release-cli-migration.md)，适用 plan 11 的
`phase-verification-harness`、`release-artifact-authority` 与 `portable-windows-runtime`。
USER 在当前任务明确授权继续收尾并分批 commit + push；此前报告中的未提交状态是历史记录。
原有产品运行时、其他 Phase 投影与 Phase 05 benchmark 串行化改动不属于此次提交。
本轮不改变 plan、产品依赖、候选身份、发布权限或人工验收状态。

## 已复现的问题与根因

在同一双 PowerShell 宿主 fixture 中，让假 Syft 写出不完整内容后退出 23，事先在最终路径
保存旧 SBOM 和旧 manifest。旧 `generate-sbom.ps1` 直接将最终路径交给 Syft，失败后旧 SBOM
已被替换；`outputs-before.log` 记录了 `Failed SBOM generation modified previous outputs`。

另外，checksum 的格式化/写入与 SPDX 版本、packages、必需文件覆盖判断仍在 PowerShell；
包验证只校验 SBOM 的摘要，正确摘要不能证明其内容通过生成端的结构检查。治理中的许可证
函数名/调用次数断言也不能证明 ZIP 内实际文本符合编码要求。

最初测试目录叠加完整 SHA、中文、空格及打包临时后缀触及 Windows PowerShell 5.1 的路径长度
限制。测试输出改用另一个较短的独立临时目录后，保留中文/空格和原长路径 verifier fixture，
再执行上述相同输入基线。本轮未声称增加了生产长路径支持。

## 实现与失败边界

- `release/checksums.rs` 复用 `integrity` 的名称、摘要、成员互异与严格集合规则生成 manifest。
  `package.ps1` 调用 Rust 并保留原有 `PACKAGE_PATH`/`PACKAGE_SHA256` 输出。
- `release/sbom.rs` 统一 SPDX 2.x、非空 packages 和四个必需打包文件的结构/覆盖规则；
  生成发布与 package verifier 复用它。它不宣称完整 SPDX schema 或许可证审计。
- Syft 先写入本次独占临时目录且位于扫描 context 之外。Rust 对该文件建立私有快照，
  完成 UTF-8/JSON、内容、路径和摘要校验，再复用 `atomic_evidence` 替换最终文件。
- 两个输出逐文件原子替换，manifest 最后写入。生成/校验失败保留两个既有输出；
  SBOM 替换失败同样保留旧文件。若 SBOM 已替换而 manifest 被占用，命令失败，
  旧 manifest 与新 SBOM 不匹配时验证拒绝。Windows 文件锁回归验证该路径和临时文件清理。
- 沿用既有单文件原子设施，没有增加多文件事务、回滚账本或新的并列 authority。
  临时文件发布不是 Source Freeze、Promote，也不更新候选或资格化收据。
- ZIP/资源/Syft 获取调用/UIA 平台适配保留。Syft 版本及下载摘要固定值不变。
  PowerShell 使用调用者 SessionState 解析相对路径，并恢复 CWD、编码及 Syft 环境。
- 许可证治理改为读取实际生成 ZIP 的三个许可证成员，验证非空 UTF-8、无 BOM 和 LF。
  pin 等声明性治理检查仍保留；普通规则不再靠函数名或调用次数证明。

可选路径是继续直接写最终文件，或引入跨文件事务。前者保留已复现的数据覆盖问题；后者超出
此次本地输出维护的必要范围。当前选择先完整验证、逐文件原子替换、manifest 最后写入并在
不匹配时拒绝验证，保持已有 artifact/checksum 身份合同。

## 已执行的定向验证

证据根目录：`C:\Users\QQ\AppData\Local\Temp\stickymd-release-finish-20260925-k4m4419u`。

| 检查 | 实际结果 / 日志 |
| --- | --- |
| 同输入前后回归 | 旧输出覆盖复现失败；修复后双宿主回归通过：`outputs-before.log`、`outputs-after.log` |
| Windows smoke unit + compiled CLI | 170 + 9 通过：`windows-tests.log`；包含正确 hash 仍拒绝非法 SBOM |
| PowerShell 5.1/7 | 两个宿主均执行实际打包、失败 Syft、非法 SBOM、输出字节和环境恢复测试 |
| Linux smoke | 137 unit + 9 CLI 通过：`linux-checks.log`；Windows wrapper 在 Linux 不适用，不计入通过数 |
| Windows Clippy | `--all-targets --locked -- -D warnings` 通过：`windows-clippy.log` |
| Linux Clippy | 普通 Clippy 退出 0；严格 `-D warnings` 失败于既有 `dead_code`。收尾前独立工作树与最终代码逐项比较 warning 消息/位置一致，bin 11 条、test 6 条（5 条重复）：`linux-{baseline,final}-clippy.log`、`linux-warning-comparison.txt` |
| 格式与补丁 | `cargo fmt --all -- --check`、`git diff --check` 通过 |
| 第一批独立提交 | `f8644ea` 在干净独立工作树运行完整 Windows smoke：164 unit + 8 CLI + 2 wrapper = 174 通过，`batch1-clean-tests.log` |

## 推断与未验证事项

基于实际调用和回归，输出规则已由 Rust 单点持有，失败时不会将不完整 Syft 输出直接写到最终
SBOM。没有测量或宣称性能收益。单文件原子替换不提供两个文件一起提交的事务保证。

以上是工具维护证据，不继承 `v0.1.0` exact artifact 身份。完整产品 workspace、资源/性能/
G3/G4/G5 Campaign、人工视觉、物理多屏、Clean VM 和远程 workflow 不在此次定向验证范围。
后续独立提交的本地包检查另行追加记录；不会以旧包或旧资格化收据替代本次工具验证。
