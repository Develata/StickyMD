# StickyMD Release Checklist

本文件是未来版本的可执行发布清单，不是自动化证据本身。正式规则以
[`docs/plan/11_testing_and_release.md`](plan/11_testing_and_release.md) 为准；每次发布的 source、artifact、
收据和 USER authority 必须重新建立，不能从旧版本继承。

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

## 2. Remote Release Exact Artifact

- [ ] 获得 exact Source Freeze 的 PUSH authority。
- [ ] 只 push 被批准的 SHA；该 SHA 的普通 GitHub CI 全绿。
- [ ] 获得一次 candidate-only `release.yml` dispatch authority。
- [ ] workflow 从批准的 exact source 构建且成功；不创建 tag 或 Release。
- [ ] 记录唯一 run、attempt、artifact id/name，不自动选择“最新同 commit build”。
- [ ] Rust qualification CLI 按记录的 run/name 下载 ZIP、`SHA256SUMS.txt` 与 SBOM。
- [ ] 下载副本与用户指定副本逐字节一致。
- [ ] checksum、SBOM、包结构、license/notices 与 source identity PASS。
- [ ] exact EXE 的普通与 delay-load import gate PASS，无外置 developer runtime。
- [ ] artifact 通过验证后原子 Promote 到 `dist/exact-candidate/`；失败不半更新既有 candidate。

## 3. Artifact-bound Qualification

- [ ] Runtime receipt 绑定 Promoted Candidate。
- [ ] 30 cold + 30/50 warm（以当前 plan 为准）Performance receipt 绑定 Promoted Candidate。
- [ ] warm cohort 使用正式 `1000 ms` 进程间隔；rapid-restart 诊断不冒充 warm receipt。
- [ ] Resources receipt 绑定 Promoted Candidate，所有 hard gate PASS。
- [ ] G3、G4、G5 当前输入指纹有 compatible last-success；stale 模块只重跑受影响部分。
- [ ] exact-byte package、checksum、SBOM、PE 与 portable-runtime gate 不通过模块 ledger 跳过。
- [ ] GUI 前环境为 `VALID`；环境阻塞保持 `NOT TESTED`，不制造产品 PASS/FAIL。
- [ ] smoke-owned child 没有遗留；用户自己的 StickyMD 进程未被工具终止。

## 4. Manual Acceptance

- [ ] 每项 Tier A 有 exact-bound `MANUAL_PASS` 或 USER 明确的 case/group waiver。
- [ ] 每组 Tier B 有 exact-bound `MANUAL_PASS` 或绑定 version/source 的明确 waiver。
- [ ] Tier C `NOT TESTED` 已如实记录；任何已观察到的 `MANUAL_FAIL` 仍阻断。
- [ ] 真实输入法候选窗、视觉、任务栏/Alt+Tab、托盘、dock、主题、透明度、渲染观感按适用矩阵确认。
- [ ] Clean VM、物理多屏、DPI、显示器拔插、RDP 与负坐标按本次发布政策处理。
- [ ] 人工 receipt 记录 source、EXE、ZIP、version、Windows build、环境、case 和结果。

## 5. Readiness

- [ ] `release-readiness.json` 绑定当前 source、EXE、ZIP 与 SBOM。
- [ ] Readiness 结果严格为 `READY`，无 blocker、identity mismatch 或 schema corruption。
- [ ] Release notes 与 README 说明系统要求、unsigned policy、校验方法和已知验证缺口。
- [ ] 将要发布的 ZIP 正是已验收 Promoted Candidate，不从本地重新 build。

## 6. Tag、Draft 与 Publish

以下是三个独立 authority，必须依次单独获得 USER 授权：

- [ ] **TAG**：在 exact source 创建并验证版本 tag；不重建产品。
- [ ] **DRAFT-RELEASE**：从指定历史 workflow run 下载同一 artifact，重新验证 hash，生成 attestation，
      创建 draft；不重建产品。
- [ ] 人工检查 draft 标题、正文、资产数量、文件名、大小、checksums、SBOM 与 attestation。
- [ ] **PUBLISH**：重新下载并验证既有 draft assets，只公开该 draft，不替换资产。
- [ ] 公开 Release 的 tag、source、ZIP/EXE/SBOM hash 与已批准 identity 完全一致。

## 7. 发布后

- [ ] 从公开 Release 重新下载全部资产并独立验证。
- [ ] Release 页面不是 draft/prerelease（除非该版本明确如此）。
- [ ] README 的下载链接、系统要求、Security 与 release notes 可访问且互相一致。
- [ ] 记录 Release URL、发布时间、workflow runs、hash 与剩余风险。
- [ ] 不修改已有 tag 或替换已公开资产；修复进入新版本和新的完整证据链。
