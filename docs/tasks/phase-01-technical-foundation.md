# phase-01-technical-foundation.md - Phase 1 重建记录

## Completion State

Completed

本任务的技术基础与可删除 spike 已于 2026-08-20 完成；当时尚未关闭的人工/环境条件继续
保留在本文件的 Risks/Result 中，并由后续 release qualification 与 USER disposition 处理。

## Goal

建立可编译 production workspace 边界，并用可删除实验验证 Markdown/Math 与 portable
persistence 的高风险事实；窗口/IME 由当前 production dev shell 复核，避免保留第二套实现。

## Inputs

见 `docs/report/phase-01-dependency-baseline.md`。原 `arrayref` yanked 风险结论已在后续复核中证伪并更正。

## Scope

### Spikes

- Window/framebuffer：由当前 `stickymd-win` Release dev shell 进行五次稳定采样；产品 DPI/
  opacity/DWM 仍未验收。
- IME：自动化 pipeline 在 Phase 3；微软拼音/微信输入法人工矩阵仍 NOT TESTED。
- Markdown/Math：`experiments/phase-01/markdown-math`，6 个 contract tests + Release baseline。
- Persistence：`experiments/phase-01/persistence`，9 个 failure/recovery/writability tests + Windows smoke。

## Deliverables

- 重建的技术报告、依赖/Windows API/性能基线。
- 无重复窗口/编辑器实验实现。
- mtime-aware recovery 与保守 atomic replacement spike。
- Comrak `default-features = false` + owned projection + RaTeX baseline。

## Verification

- 稳定入口：`tools/smoke/phase-01.ps1`；`-Performance` 显式运行环境敏感 Release 测量。
- 当前自动/人工状态：`docs/acceptance-cases/phase-01.md`。

- 两个实验的 fmt/clippy/test/Release run。
- production workspace baseline and `cargo deny check` with the Windows-target policy。
- Windows-target forbidden dependency scan。
- `experiments/phase-01` 可删除性检查。

## Out of Scope

- 把 spike 直接保留为第二套 production 实现。
- 代替后续 Phase 完成真实 IME、Preview painter、持久化与 Windows shell 产品验收。
- 把当时未执行的人工/环境项目提升为 PASS。

## Risks

- 真实 Microsoft Pinyin / WeChat IME 未测试。
- RaTeX 生产 painter（不得使用 PNG encode/decode 热路径）尚未验证。
- `cosmic-text → fontdb → ttf-parser` 命中无人维护公告；精确临时例外与退出条件见
  `docs/report/RISK-ttf-parser-unmaintained.md`。
- junction/ACL/kill-mid-save/power loss 未测试。

## Result

旧 spike 与夸大的 PASS 报告已从仓库移除；保留的每项结论均区分 PASS、CONDITIONAL
与 NOT TESTED。Phase 1 不再被描述为无条件完成。
