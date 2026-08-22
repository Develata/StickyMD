# Phase 09 — Pre-Release Convergence

## Completion State

Implementation Complete — release validation incomplete.

## Goal

在不增加产品能力的前提下，收口 Phase 0–8 的未决证据，实测并加固启动、内存、CPU、输入与预览性能，完成数据安全回归、供应链审计、portable RC 打包与 release 工作流静态验证，并给出诚实的发布结论。

## Inputs

- USER 已明确批准进入 Phase 9，并要求长期可维护性、高内聚低耦合、简洁高效算法与低内存优先。
- USER 对启动门的补充决策：先继续以 cold p95 ≤300 ms 为目标；合理、低复杂度且不牺牲正确性的优化仍无法达到时，允许将发布硬门放宽为 p95 ≤400 ms。放宽时必须同时记录 300 ms 为 `USER WAIVED`，超过 400 ms 仍为 blocker。
- Starting commit: `318037fe9be4ddbb41785eb723e6ebea9b40c390`。
- Phase 8 recommendation: `APPROVE Phase 9 WITH CONDITIONS`。
- 冻结输入：[`Phase 9 prompt`](../phases/2026-08-21-phase-09-pre-release-convergence.md)。

## Scope

1. 9A release blocker 与 inherited-condition 清单。
2. 9B 可输入 readiness 信号、阶段里程碑、20 cold + 20 warm copied-Release 测量与单因素优化。
3. 9C 全量人工验收矩阵；没有真实收据的项目保持 `NOT TESTED`。
4. 9D 原子保存/OCC/恢复/资产/导出/零网络的可靠性回归。
5. 9E 当前提交上的内存、CPU、延迟、4K transient peak 与 leak stress。
6. 9F 依赖、advisory、许可证、网络与 SBOM 审计。
7. 9G–9I portable RC package、release workflow 与本地 RC 验证。
8. 9J AC-001..030、baseline、架构 review 与发布建议。

## Deliverables

- `tools/smoke/phase-09.ps1` 与 Rust CLI Phase 09 路由。
- `docs/acceptance-cases/phase-09.md`。
- Phase 9 task、blocker、startup、manual、performance、supply-chain、workflow、readiness reports。
- portable package/verify/SBOM scripts与 pinned release workflow。
- release-facing README/README.zh/CHANGELOG/SECURITY/CONTRIBUTING/checklist。

## Verification

- 每个性能结论使用当前 Release artifact、冻结 fixture 与规定样本数。
- `tools/smoke/phase-09.ps1`、`-Performance`、`-Release`、`-Package`。
- `tools/smoke/all.ps1 -Ci` 只运行可无界面的合并任务。
- fmt、clippy、workspace tests、Release build、cargo deny、diff check、unsafe/forbidden/network scans。
- 最多三个只读 reviewer；主 Agent 负责裁决。

## Out of Scope

- 新产品功能、旧架构兼容层、WebView/网络/更新器/遥测/数据库。
- 为启动数字牺牲 CJK/Emoji fallback、IME 正确性、数据安全或引入第二文本 authority。
- 自动 tag、push、发布 GitHub Release 或把版本擅自改为 1.0.0。

## Risks

- Warm-start 字体扫描与初始 source shaping 仍超过冻结门槛；后续只接受有测量依据且不削弱
  CJK/Emoji fallback 的优化。
- 真实 IME、视觉、物理显示器、系统托盘、崩溃时序、Clean VM 等环境证据仍未关闭。
- `ttf-parser` 的已知上游维护 advisory 继续按 risk report 跟踪，不以盲目升级换取表面清零。

## Result

Phase 9 implementation and the automated convergence pass are complete. Independent review findings
were applied: diagnostic traces now publish atomically without overwrite, exact runtime notices are
generated from the locked Windows graph, idle CPU uses five independent 60-second samples, and leak
stress covers persistence, conflict and image lifecycles. Cold startup p95 is 277.205 ms and passes
the original 300 ms gate, so the USER-authorized 400 ms fallback was not used. Warm startup p95 is
342.891 ms and remains above its unchanged 180 ms gate. Exact clean-source commit `eb687b2` passed
package/SBOM/license/checksum/PE and copied-runtime verification; all real-environment manual rows
remain `NOT TESTED`. The release recommendation is therefore `NOT RC READY`, and no tag, push or
GitHub Release was created.
