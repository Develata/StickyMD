# Phase 18 macOS 安装版与公开分发

## Completion State

In Progress（2026-11-19 开始）。

## Goal

从下一版本起，在 GitHub Release 上与 Windows 同版公开发布 macOS arm64 的 ad-hoc 签名 ZIP；在第一个公开包之前，
让 `StickyMD.app` 直接放进“应用程序”时成为安装版，便签存在 `~/Library/Application Support/StickyMD/<键>/`。
Windows 的产品行为、发布文件名与字节不变。

## Inputs

- [分发报告（修订版）](../report/2026-11-19-macos-public-distribution-decision-r2.md)的各 Resolution：独立的 macOS 发布物、
  两边一起发、跨提交沿用、ad-hoc 签名。
- [安装版报告（修订版）](../report/2026-11-19-macos-installed-note-location-r2.md)的 Resolution：6 项全部批准。
- 合同：`01` Note Root、Platform Build Input Fingerprint、Release Attachment Set；`02` #portable-and-installed；`05` #note-root；
  [`11` Phase 18 节](../plan/11_testing_and_release.md#phase-18-macos-distribution)、#macos-release-artifact、#platform-artifact-reuse、
  #macos-release-gate。

## Scope

| 里程碑 | 状态 | 内容 |
| --- | --- | --- |
| M1 | 完成，待审查修正 | 合同落盘：plan 01、02、04、05、08、09、11；`AGENTS.md`、overview、features、coverage matrix；Phase 18 矩阵与入口 |
| M2 | 代码与测试完成 | 安装版：`apps/stickymd-mac` 的位置判定、键、启动与持久化；`crates/stickymd-posix` 的家目录查询、Note Root 句柄链与加锁 |
| M3 | 待开始 | `release-macos.yml`、按平台的第三方声明、macOS SBOM 与 checksum、候选收据与本地晋升、`promote-release.yml` 的完整附件比对、平台构建输入指纹与沿用 |
| M4 | 待开始 | 本机 fmt、clippy、测试；`mac-test` 中的 `mac-gui`；候选包上的下载路径 |
| M5 | 待开始 | macOS 就绪核对（`11` #macos-release-gate），下一版本发布前完成 |

## Deliverables

- 合同与投影（M1）。
- 安装版实现与测试（M2），矩阵 L 系列。
- 发布 workflow 与 `stickymd-smoke` 的 macOS 打包、声明、SBOM、checksum、晋升、指纹与沿用命令（M3），矩阵 P 系列。
- 验证记录（M4）写进报告，矩阵 D 系列。

## Verification

- `tools/smoke/phase-18.ps1`（`stickymd-smoke phase 18`）运行 Phase 18 的自动化测试。
- `phase 00` 检查矩阵与入口。
- 每版的 macOS 发布证据记在候选报告与 ignored `dist/evidence/`，不写进矩阵（`11` Phase 18 节）。
- Windows：`apps/stickymd-mac/`、`crates/stickymd-posix/` 不进入任何 Windows 资格化模块；共享层不改。合同改动按现行规则使本版 Windows
  资格化整体重来，与 Phase 17 同版。

## Out Of Scope

Developer ID 与公证、DMG、pkg、Homebrew cask、Mac App Store、自动迁移便签、Intel 与 universal 包、Debian 交付形态、补发 v0.2.0 的 macOS 包。
