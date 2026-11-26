# StickyMD Release Checklist

本文件是未来版本的可执行发布清单，不是自动化证据本身。正式规则以
[`docs/plan/11_testing_and_release.md`](plan/11_testing_and_release.md) 为准；每次发布的 source、artifact、
收据和 USER authority 必须重新建立，不能从旧版本继承。

## v0.2.0 已发布记录

- Release：[StickyMD v0.2.0](https://github.com/Develata/StickyMD/releases/tag/v0.2.0)，release id `450091881`
- 发布时间：`2026-11-18T14:25:15Z`；正式版，非 draft/prerelease，GitHub latest 指向本版本。
- Exact source / tag：`fc97e83adfc6dcd3d29b42f924d9476a120647ac`。
- Source CI：`44044061626`；candidate workflow：`44046334056` / attempt 1，artifact `14313335997`，均成功。
- ZIP SHA-256：`8d35e2f188073c6f3ee91de577990e016f8f982797547e823b4c739ba6b70361`
- EXE SHA-256：`1b60231af5099608ea6e323a0a2506e64fa7195e237bdcd16b162ce204a599b1`
- SBOM SHA-256：`0fdabd2346471fbc8a056115fc95dac56102cd4ad404c345c83a3c55a49e2165`
- Tag：`44322899347`；draft：`44323187556`；publish：`44323864385`，全部成功。
  两个会话并行 dispatch，另有 tag 校验 `44323004943` 与重传相同字节的 draft `44323282286`，均成功；经过见发布记录。
- 晋升沿用已验收候选字节；公开三个附件不带认证下载后与原候选逐字节一致；ZIP/SBOM provenance 与 ZIP SPDX 证明核验通过。
- Readiness `READY`（Windows 11 25H2 26200.9974，Windows 会话运行）：
  - 10 个资格化模块在本候选上 `RAN_PASS`；
  - Tier A 15 项 `MANUAL_PASS`，P12-M05 由 USER 按场景豁免；
  - Tier B 单屏 DPI 组由 USER 按组豁免；
  - previous 目标（24H2）、Tier O 与 Tier C 保持 `NOT TESTED`。
  - 没有使用整版 `NOT_READY` 例外。
- macOS 版本机构建，不提供下载；Phase 16 的验收绑定发布前的构建，0.2.0 构建没有重跑。

具体边界、产物摘要与公开核验结果见[候选准备与发布记录](report/2026-11-17-v0.2.0-candidate-preparation.md)
和[发布说明](release-notes/0.2.0.md)。

## v0.1.3 已发布记录

- Release：[StickyMD v0.1.3](https://github.com/Develata/StickyMD/releases/tag/v0.1.3)
- 发布时间：2026-10-23 11:18:57（America/New_York）；正式版，非 draft/prerelease，GitHub latest 指向本版本。
- Exact source / tag：`1d943497b91b704a34e32b3150a923d5113a5f9d`。
- Source CI：`39848336424`；candidate workflow：`39848983265` / attempt 1，artifact `12458646214`，均成功。
- Tag：`39974103329`；draft：`39975953298`；publish：`39977030651`，全部成功。
- 晋升沿用已验收候选字节；公开三个附件独立下载后的 SHA-256 与原候选一致，来源与 SBOM 证明核验通过。
- 10 个资格化模块均为 RAN_PASS 或 REUSED_PASS：最终候选的 G4/五类 Resources 实测通过，Runtime/Performance/G3/G5 兼容复用通过；P14-A83（物理输入注入的桌面取证）仍为 `NOT TESTED`，不属上述通过范围。
- 本机补充检查与 USER 四组实际人工观察通过；另一系统、多屏/混合 DPI、干净 VM 等未测范围及正式人工记录缺口如实保留。
- USER 已明确批准仅限本版本的 tag、draft 与 publish 例外；技术 readiness 保持 `NOT_READY`。

具体边界、产物摘要与公开核验结果见[发布记录](report/2026-10-22-v0.1.3-candidate-preparation.md)
和[发布说明](release-notes/0.1.3.md)。以下历史版本保持各自身份；本次例外不改变未来版本的发布合同。

## v0.1.2 已发布记录

- Release：[StickyMD v0.1.2](https://github.com/Develata/StickyMD/releases/tag/v0.1.2)
- 发布时间：`2026-10-02T06:26:54Z`；正式版，非 draft/prerelease，GitHub latest 指向本版本。
- Exact source / tag：`e36e61d65536c0cbaa84ee4004f1a289fbcb91dd`
- Candidate CI：`36964107323`，9/9 PASS；candidate workflow：`36964744450` / attempt 1，artifact `11209891744`。
- ZIP SHA-256：`eeb8e7bd2661833b75acafaacd16ccd7290a39d14839e2ab8d8c106abd06b289`
- EXE SHA-256：`9a8fac57580bb1c612594617803056e24eab2918bf82ada161b1a1e03a7cf23a`
- SBOM SHA-256：`d6ed71c2d81ece81fd16bd381e21e672d215c6f9f4c4dffb2869744d9d34a30b`
- Tag 校验：`36972870040`；draft：`36973016557`；publish：`36973520526`，全部成功。
- 晋升 workflow revision 与产品 source 均为上述 `e36e61d`；晋升只使用原候选附件。
- 公开三个附件重新下载后 SHA-256 与原候选一致；ZIP/SBOM provenance 和 ZIP SPDX attestation 校验通过。
- 技术 readiness 保留 `NOT_READY`，未测项保持 `NOT TESTED`；采用本版本明确的 USER 发布特例。

完整 Runtime/Performance/Resources/G3/G4/G5、本地 exact-bound headless qualification 收据及
强制人工收据尚未补齐。USER 在看到具体候选、缺口和完整验收耗时后决定先发布本版本；
该决定不改变未来版本的工程门，也不继承旧版本人工 PASS。范围和验证记录见
[发布决定](report/2026-10-02-v0.1.2-release-preparation.md)与[发布说明](release-notes/0.1.2.md)。

## v0.1.1 已发布记录

- Release：[StickyMD v0.1.1](https://github.com/Develata/StickyMD/releases/tag/v0.1.1)
- 发布时间：`2026-09-29T12:50:39Z`；正式版，非 draft/prerelease。
- Exact source / tag：`9a0a00a1fe143cf25fa4208a54ec5dfea7f3e7c9`
- Candidate CI：`36565543814`，PASS；candidate workflow run：`36565605768` / attempt 1。
- ZIP SHA-256：`c8a46192df50d580354363e05545787e186788f27a103978dcfe039252ff27a3`
- EXE SHA-256：`c398e969c0b2e8319a1cac2d66278feca57177a1d467b01cac529662ba23fa5a`
- SBOM SHA-256：`fefff9935883369d02627b78a9c2661a7063828cd4383e07334214f722413457`
- 技术 readiness：保留 `NOT_READY`；发布依据为本版本明确的 USER exception。
- Tag 校验 run：`36569493129` / attempt 2；draft run：`36569914568`；publish run：`36570737949`。
- 晋升 workflow revision：`6daccc1c21a1a71c500aed68cb23d71c9160d77d`；其 CI `36569445309` PASS。
  该维护提交修复晋升 runner，不是产品源码 tag，也没有重建候选。
- 公开三个附件重新下载后与原候选 SHA-256 全部一致；ZIP/SBOM provenance 和 ZIP 的 SBOM
  attestation 均通过仓库、workflow 路径及 signer digest 约束校验。
- 发布形态：unsigned Authenticode、Windows 11 x64 portable ZIP。

USER 接受启动 p95 超过 550 ms、G4 首次失败后单独复测通过，以及资源/G5 修复后的完整复验、
人工视觉、Clean Windows VM、多屏/DPI 尚未完成的缺口。原始失败和未验证状态不改写为 PASS，
本次例外不改变未来版本的工程门。完整范围与证据见
[发布决定](report/2026-09-29-v0.1.1-release-authorization.md)及
[发布说明](release-notes/0.1.1.md)。

## v0.1.0 已发布记录

- Release：[StickyMD v0.1.0](https://github.com/Develata/StickyMD/releases/tag/v0.1.0)
- Exact source：`64690ab8f86f63f3cbfeabbb0961276978c8f26d`
- Candidate workflow run：`33313844481`
- ZIP SHA-256：`ef700a04d728eee0b575b6c8fe57577166c426f19a8214a27939005a549bde83`
- EXE SHA-256：`d45556230d81fb12b4b13b9f7a93722c3474564077036aa56d3c7a6745f32f11`
- SBOM SHA-256：`359fd64f7b1b1ef1b577071577badc383bdee4e4ebfb395739e49d4ff91123b0`
- Final readiness：`READY`
- Tag operation run：`33320693967`
- Draft operation run：`33320827172`
- Publish operation run：`33320974588`
- 发布形态：unsigned Authenticode、Windows 11 x64 portable ZIP。

`v0.1.0` 对 Clean Windows 11 VM、真实双显示器同 DPI、真实双显示器 mixed DPI 和运行中拔除
当前显示器采用了绑定该版本/source 的明确 USER waiver。Sleep/resume、RDP 重连与物理负坐标
显示器为非阻塞 Tier C `NOT TESTED`。完整用户可见说明见
[`release-notes/0.1.0.md`](release-notes/0.1.0.md)。

## 1. Source Freeze

- [ ] 版本、目标平台、unsigned/signed policy 与发布范围已经由 USER 明确批准。
- [ ] 所有必须进入发布包和 Git tag 的 README、License、Security、Notices 与 release notes 已提交。
- [ ] 工作树 clean；记录 exact HEAD、workspace version、`Cargo.lock` SHA-256 与目标平台。
- [ ] `cargo fmt --check` PASS。
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` PASS。
- [ ] `cargo test --workspace --locked` PASS。
- [ ] `cargo deny check` PASS 或所有 advisory 均有当前版本的明确处置。
- [ ] `./tools/smoke/all.ps1 -Ci` PASS；CI 分片并集与完整任务图一致。
- [ ] Source Freeze receipt 已生成，并且后续 tracked source/tooling/contract 修改会使其 stale。
- [ ] Freeze 后通过 `qualification decision --key=RELEASE-VERSION --status=USER_APPROVED --evidence=<本次明确授权>`
      记录 USER 已批准的当前版本；历史模板不会自动批准新 Freeze 的版本。

## 2. Remote Release Exact Artifact

- [ ] 获得 exact Source Freeze 的 PUSH authority。
- [ ] 只 push 被批准的 SHA；该 SHA 的普通 GitHub CI 全绿。
- [ ] 获得一次 candidate-only `release.yml` dispatch authority。
- [ ] workflow 从批准的 exact source 构建且成功；不创建 tag 或 Release。
- [ ] 记录唯一 run、attempt、artifact id/name，不自动选择“最新同 commit build”。
- [ ] Rust qualification CLI 核对记录的 artifact id 属于记录的 run 与 head、名称一致且未过期，再按该 id 下载 ZIP、`SHA256SUMS.txt` 与 SBOM。
- [ ] 下载副本与用户指定副本逐字节一致。
- [ ] checksum、SBOM、包结构、license/notices 与 source identity PASS。
- [ ] exact EXE 的普通与 delay-load import gate PASS，无外置 developer runtime。
- [ ] artifact 通过验证后原子 Promote 到 `dist/exact-candidate/`；失败不半更新既有 candidate。
- [ ] Windows 发布物沿用祖先提交 S 上的构建时（`11` #platform-artifact-reuse），`release verify-reuse --platform windows-x64
      --build-source <S> --release-source <R>` PASS，`dist/evidence/release-reuse-windows-x64.json` 已写出。

## 2M. macOS Release Exact Artifact（Phase 18 起）

两个平台同版发布；macOS 门不过，整个版本不发布（`11` #macos-release-gate）。

- [ ] 获得一次 candidate-only `release-macos.yml` dispatch authority；它运行在 `xcode-27`，先选定 Xcode 27.2。
- [ ] workflow 成功；记下 run、成功的 attempt 与 artifact id（名为 `stickymd-macos-arm64-release-<attempt>`），核对 Summary 中的构建记录。
- [ ] 本机 macOS，在干净的 R 检出上（HEAD 为 R、工作树干净；否则晋升在下载之前即拒绝）：`release macos-promote --run-id <run>
      --attempt <n> --release-source <R> --zip <下载的 ZIP>` PASS，`dist/exact-candidate-macos/`、
      `dist/evidence/release-candidate-macos.json` 与晋升复核记录 `dist/evidence/release-promotion-macos.json` 已写出，签名方式为观察到的
      `AD_HOC`。在别的提交上晋升过的候选（包括本清单之前晋升的），运行 `macos-ready` 之前在 R 上再晋升一次：同一 attempt 得到逐字节相同的
      receipt，绑定其摘要的 USER 记录仍然成立；缺复核记录或记录不属于 R 与这份 receipt 时，包完整性与 P16-P01 为 `NOT TESTED`。
      被替换的旧 staging 中若有晋升不会写的内容（例如在暂存 `.app` 旁写下的 `StickyMD/note/`），它会被保留并打印其位置，不删除。
- [ ] `release macos-candidate --release-source <R>` PASS（staged 字节、bundle 清单与可执行文件与 receipt 一致，构建来源可沿用）。
- [ ] macOS 就绪按 `11` #macos-release-gate 逐行成立（M5 的核对工具完成前，这一项不能勾选）。

## 3. Artifact-bound Qualification

- [ ] Runtime receipt 绑定 Promoted Candidate。
- [ ] 30 cold + 30/50 warm（以当前 plan 为准）Performance receipt 绑定 Promoted Candidate。
- [ ] warm cohort 使用正式 `1000 ms` 进程间隔；rapid-restart 诊断不冒充 warm receipt。
- [ ] Resources receipt 绑定 Promoted Candidate，所有 hard gate PASS。
- [ ] G3、G4、G5 当前输入指纹有 compatible last-success；stale 模块只重跑受影响部分。
- [ ] 先运行 `qualification modules` 查看克隆级共享账本（`LEDGER_STORE=` 行）中各模块 `REUSED_PASS` / `RUN_REQUIRED`，只重跑后者。
- [ ] 发布归档复制 readiness 引用的共享账本 evidence 与 G5 截图，不只复制 worktree 的 `dist/evidence/`。
- [ ] exact-byte package、checksum、SBOM、PE 与 portable-runtime gate 不通过模块 ledger 跳过。
- [ ] GUI 前环境为 `VALID`；环境阻塞保持 `NOT TESTED`，不制造产品 PASS/FAIL。
- [ ] smoke-owned child 没有遗留；用户自己的 StickyMD 进程未被工具终止。

## 4. Manual Acceptance

- [ ] 每项 Tier A 有 exact-bound `MANUAL_PASS` 或 USER 明确的 case/group waiver。
- [ ] 每组 Tier B 有 exact-bound `MANUAL_PASS` 或绑定 version/source 的明确 waiver。
- [ ] Tier C `NOT TESTED` 已如实记录；任何已观察到的 `MANUAL_FAIL` 仍阻断。
- [ ] 可选人工项（Tier O 与 `previous` 角色下的全部项）的 `NOT TESTED` 已如实列出；它们不阻断、不需要 waiver，已观察到的 `MANUAL_FAIL` 仍阻断（USER 决定 2026-10-31）。
- [ ] 真实输入法候选窗、视觉、任务栏/Alt+Tab、托盘、dock、主题、透明度、渲染观感按适用矩阵确认。
- [ ] 中文输入法开启但未组字时，Esc、F6、F7、Ctrl+S 照常生效（`app/input.rs` 丢弃输入法声明的按键；USER 决定 2026-10-25 保留该行为并在人工验收时确认）。
- [ ] Clean VM、物理多屏、显示器拔插、RDP 与负坐标属于 Tier O；单屏 DPI（M38–M40、M47）属于 Tier B，按上文处理。
- [ ] 人工 receipt 记录 source、EXE、ZIP、version、Windows build、环境、case 和结果。

## 5. Readiness

- [ ] `release-readiness.json` 绑定当前 source、EXE、ZIP 与 SBOM。
- [ ] Readiness 结果严格为 `READY`，无 blocker、identity mismatch 或 schema corruption。
- [ ] 不使用合同外的整版 `NOT_READY` 发布例外：未满足项按第 4 节逐项（Tier A case/group、Tier B group）取得 USER waiver 后计入 `READY`（USER 决定 2026-10-25）。
- [ ] Release notes 与 README 说明系统要求、unsigned policy、校验方法和已知验证缺口。
- [ ] 将要发布的 ZIP 正是已验收 Promoted Candidate，不从本地重新 build。

## 6. Tag、Draft 与 Publish

以下是三个独立 authority，必须依次单独获得 USER 授权：

- [ ] 每次 dispatch 携带：发布提交 R、tag、Windows run id 与 artifact id、macOS run id、attempt 与 artifact id，以及 Release
      Attachment Set（六个文件 `name=sha256`，逗号分隔）。
- [ ] **TAG**：从 `main` dispatch；R 包含在 `main` 中，两个候选核对通过后创建并验证版本 tag；不重建产品。
- [ ] **DRAFT-RELEASE**：从 tag dispatch（运行的 SHA 必须等于 R）；按 artifact id 下载两个平台的同一 artifact，核对六件，
      为两个平台生成 attestation，创建 draft；草稿上残留集合外的旧附件时拒绝；上传后草稿恰为六件。不重建产品。
- [ ] 人工检查 draft 标题、正文（两个平台各自的构建来源）、资产数量、文件名、大小、checksums、SBOM 与 attestation。
- [ ] 第一次跨提交沿用时，PUBLISH 前查看实际签发的 attestation statement，确认它如何表述构建来源（未验证项）。
- [ ] PUBLISH 之前：仓库 ruleset 禁止更新与删除 `refs/tags/v*`；列清能写 release 的人、App、token 与 workflow，暂停本 workflow 以外的自动写入；
  PUBLISH 运行期间不从网页或 CLI 改动草稿与 tag（`11` 发布晋升第 5 条）。
- [ ] **PUBLISH**：从 tag dispatch；重新下载草稿全部附件，比对完整的文件名集合与哈希，公开前后按 release id 复读并核对 tag 仍指向 R，
  只公开该 draft，不替换资产；不符时 job 失败，不自动改回草稿。
- [ ] 公开 Release 的 tag、source、两个平台的 ZIP/SBOM/checksum hash 与已批准 identity 完全一致。

## 7. 发布后

- [ ] 从公开 Release 重新下载全部资产并独立验证。
- [ ] Release 页面不是 draft/prerelease（除非该版本明确如此）。
- [ ] README 的下载链接、系统要求、Security 与 release notes 可访问且互相一致。
- [ ] 记录 Release URL、发布时间、workflow runs、hash 与剩余风险。
- [ ] 不修改已有 tag 或替换已公开资产；修复进入新版本和新的完整证据链。
