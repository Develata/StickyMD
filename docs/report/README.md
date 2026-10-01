# Report 索引

本目录存放有时间属性的分析证据（风险报告、阶段分析、冲突报告）。目录规则见
[`AGENTS.md`](AGENTS.md)。

报告中的 `NOT READY`、`pending` 或 `NOT TESTED` 只描述报告生成时的状态，不应被解释成
仓库当前发布状态。当前公开版本是 `v0.1.1`，采用明确的 USER 发布特例，技术 readiness
仍为 `NOT_READY`；用户可见结论见 [`0.1.1 release notes`](../release-notes/0.1.1.md)。
后续工具维护和普通 CI 成功不继承该版本的 artifact 身份或特例。动态 exact-artifact 收据位于被忽略的
`dist/evidence/`，不会回写历史报告制造“事后通过”。

## 当前实现与维护入口

- [Preview 渲染规则与优先级](2026-09-30-preview-rendering-rules.md)：语法/样式/布局/调度/缓存的详细源码说明与已知合同差异；阅读时结合 [2026-10-01 勘误](2026-09-30-preview-rendering-rules.md#review-2026-10-01)和 [本地图片、列表、字体修复](2026-09-30-preview-rendering-rules.md#fixes-2026-10-01)。各次测试保留日期与范围，不是新渲染合同。
- [本地选测、兼容性夹具与耗时汇总](2026-09-30-local-development-verification.md)：`dev-check` / `timings`、后续 review 和 Windows CI 路径别名修复；按 Resolution 保留失败与复验记录。
- [发布 CLI 内部复用](2026-09-30-release-cli-reuse.md)：验包、版本与 CI 规则复用及局部测量。
- [包清单、README 与 Syft 缓存](2026-09-30-package-staging-syft-cli.md)：稳定产物语义、身份校验和双宿主行为。
- [ZIP 落盘与发布收尾](2026-09-30-release-cli-finalization.md)：冲突、并发、checksum 与阶段路由维护。
- [阶段路由、发布工作流与哈希](2026-09-30-phase-routing-release-workflows-cng.md)：后续 Rust CLI 收拢及 Windows adapter 边界。
- [资源场景等价与分批记录](2026-09-30-resource-equivalence-and-batching.md)、[续跑计划](2026-09-30-resource-resume-planning.md)、[实际诊断复审](2026-09-30-resource-live-review.md)：区分完整资格化、定向诊断、历史复用和测量范围。

## v0.1.1 发布与资格化历史

- [发布决定](2026-09-29-v0.1.1-release-authorization.md)：exact source、USER 特例与未关闭的技术 readiness。
- [独占桌面资格化](2026-09-29-v0.1.1-exclusive-desktop-qualification.md)：该次桌面运行的输入与结果。
- [桌面资格化记录](2026-09-28-v0.1.1-desktop-qualification.md)、[较早桌面证据](2026-09-26-v0.1.1-desktop-evidence.md)、[较早 readiness](2026-09-25-v0.1.1-readiness.md)：按日期与候选身份阅读，不互相覆盖。
- [正式发布说明](../release-notes/0.1.1.md)：公开版本的行为、身份与已知缺口。

## v0.1.0 收口入口

- [`phase-14-final-qualification.md`](phase-14-final-qualification.md)：Phase 14 最终资格化 source template 与证据通道。
- [`phase-14-release-policy.md`](phase-14-release-policy.md)：v0.1.0 发布门与 USER authority 政策。
- [`phase-14-module-success-ledger.md`](phase-14-module-success-ledger.md)：模块输入指纹与 last-success 复用设计。
- [`phase-14-memory-attribution.md`](phase-14-memory-attribution.md)：Release 内存归因与有界优化结论。
- [`phase-14-portable-runtime-hardening.md`](phase-14-portable-runtime-hardening.md)：Windows portable runtime 与开发者运行库边界。
- [`../release-notes/0.1.0.md`](../release-notes/0.1.0.md)：正式发布说明、身份与已知验证缺口。

## Phase 0–8

- [`phase-00-repository-governance-check.md`](phase-00-repository-governance-check.md)
- [`phase-00-governance-revalidation.md`](phase-00-governance-revalidation.md)
- [`phase-00-03-architecture-convergence.md`](phase-00-03-architecture-convergence.md)
- [`phase-01-technical-spike-report.md`](phase-01-technical-spike-report.md)
- [`phase-01-dependency-baseline.md`](phase-01-dependency-baseline.md)
- [`phase-01-performance-baseline.md`](phase-01-performance-baseline.md)
- [`phase-01-windows-api-baseline.md`](phase-01-windows-api-baseline.md)
- [`phase-02-core-document-model.md`](phase-02-core-document-model.md)
- [`phase-03-dependency-delta.md`](phase-03-dependency-delta.md)
- [`phase-03-source-editor-ime.md`](phase-03-source-editor-ime.md)
- [`phase-03-manual-ime-checklist.md`](phase-03-manual-ime-checklist.md)
- [`phase-04-dependency-delta.md`](phase-04-dependency-delta.md)
- [`phase-04-portable-persistence.md`](phase-04-portable-persistence.md)
- [`phase-05-dependency-delta.md`](phase-05-dependency-delta.md)
- [`phase-05-markdown-native-preview.md`](phase-05-markdown-native-preview.md)
- [`phase-06-dependency-delta.md`](phase-06-dependency-delta.md)
- [`phase-06-ratex-native-math.md`](phase-06-ratex-native-math.md)
- [`phase-07-dependency-delta.md`](phase-07-dependency-delta.md)
- [`phase-07-windows-clipboard-formats.md`](phase-07-windows-clipboard-formats.md)
- [`phase-07-managed-images-export.md`](phase-07-managed-images-export.md)
- [`phase-08-dependency-delta.md`](phase-08-dependency-delta.md)
- [`phase-08-windows-api-delta.md`](phase-08-windows-api-delta.md)
- [`phase-08-windows-desktop-shell.md`](phase-08-windows-desktop-shell.md)

## Phase 9–11

- [`phase-09-inherited-conditions.md`](phase-09-inherited-conditions.md)
- [`phase-09-manual-acceptance.md`](phase-09-manual-acceptance.md)
- [`phase-09-performance-final.md`](phase-09-performance-final.md)
- [`phase-09-portable-package.md`](phase-09-portable-package.md)
- [`phase-09-release-blockers.md`](phase-09-release-blockers.md)
- [`phase-09-release-readiness.md`](phase-09-release-readiness.md)
- [`phase-09-release-workflow.md`](phase-09-release-workflow.md)
- [`phase-09-reliability.md`](phase-09-reliability.md)
- [`phase-09-startup-gate-review.md`](phase-09-startup-gate-review.md)
- [`phase-09-startup-hardening.md`](phase-09-startup-hardening.md)
- [`phase-09-supply-chain.md`](phase-09-supply-chain.md)
- [`phase-10-automation-consolidation.md`](phase-10-automation-consolidation.md)
- [`phase-10-ux-corrections.md`](phase-10-ux-corrections.md)
- [`phase-10-startup-requalification.md`](phase-10-startup-requalification.md)
- [`phase-10-rc-requalification.md`](phase-10-rc-requalification.md)
- [`phase-11-b-final-interaction-amendment.md`](phase-11-b-final-interaction-amendment.md)
- [`phase-11-blocker-classification.md`](phase-11-blocker-classification.md)
- [`phase-11-manual-acceptance.md`](phase-11-manual-acceptance.md)
- [`phase-11-performance-final.md`](phase-11-performance-final.md)
- [`phase-11-rc-readiness.md`](phase-11-rc-readiness.md)
- [`phase-11-warm-startup-analysis.md`](phase-11-warm-startup-analysis.md)
- [`phase-11-warm-startup-gate-reassessment.md`](phase-11-warm-startup-gate-reassessment.md)

## Phase 12–14

- [`phase-12-final-qualification.md`](phase-12-final-qualification.md)
- [`phase-12-release-decisions.md`](phase-12-release-decisions.md)
- [`phase-12-release-handoff.md`](phase-12-release-handoff.md)
- [`phase-13-qualification-plan.md`](phase-13-qualification-plan.md)
- [`phase-13-final-qualification.md`](phase-13-final-qualification.md)
- [`phase-14-startup-attribution-plan.md`](phase-14-startup-attribution-plan.md)
- [`phase-14-release-policy.md`](phase-14-release-policy.md)
- [`phase-14-final-qualification.md`](phase-14-final-qualification.md)
- [`phase-14-exact-candidate-qualification-blockers.md`](phase-14-exact-candidate-qualification-blockers.md)
- [`phase-14-candidate-defect-remediation.md`](phase-14-candidate-defect-remediation.md)
- [`phase-14-ci-and-smoke-modularization.md`](phase-14-ci-and-smoke-modularization.md)
- [`phase-14-g3-exact-automation.md`](phase-14-g3-exact-automation.md)
- [`phase-14-g4-exact-automation.md`](phase-14-g4-exact-automation.md)
- [`phase-14-g5-exact-automation.md`](phase-14-g5-exact-automation.md)
- [`phase-14-global-module-review.md`](phase-14-global-module-review.md)
- [`phase-14-global-module-rereview.md`](phase-14-global-module-rereview.md)
- [`phase-14-memory-attribution.md`](phase-14-memory-attribution.md)
- [`phase-14-module-success-ledger.md`](phase-14-module-success-ledger.md)
- [`phase-14-portable-runtime-hardening.md`](phase-14-portable-runtime-hardening.md)
- [`phase-14-pre-release-module-audit.md`](phase-14-pre-release-module-audit.md)
- [`phase-14-preview-selection-geometry-design.md`](phase-14-preview-selection-geometry-design.md)
- [`phase-14-real-ime-automation-design.md`](phase-14-real-ime-automation-design.md)
- [`phase-14-resources-failure-triage.md`](phase-14-resources-failure-triage.md)
- [`phase-14-smoke-process-isolation.md`](phase-14-smoke-process-isolation.md)
- [`phase-14-split-sync-find-replace-scope-question.md`](phase-14-split-sync-find-replace-scope-question.md)

## 跨阶段验证与风险

- [表格公式源码范围](2026-09-08-table-math-source-ranges.md)：GFM 分列与公式复制/导出坐标。
- [运行时审查](2026-09-07-runtime-audit.md)、[垂直滚动条维护](2026-09-25-vertical-scrollbars.md)：按日期记录实现与复审。
- [发布规则迁移](2026-09-22-release-cli-migration.md)、[输出收尾](2026-09-25-release-output-finalization.md)：Rust 规则权威与 PowerShell 适配。
- [无界面并行检查](2026-09-29-headless-test-parallelism.md)、[集成测试合并](2026-09-29-headless-integration-consolidation.md)、[许可证输出预检](2026-09-29-notices-preflight-optimization.md)：对应批次的实测与未验证边界。
- [资源验证优化](2026-09-29-resource-verification-optimization.md)、[当日实际诊断](2026-09-29-resource-optimization-live-diagnostic.md)、[独立复审](2026-09-29-optimization-independent-review.md)、[后续诊断设计](2026-09-30-resource-diagnostics-design.md)：历史耗时不代表当前全部验收有效。
- [`phase-verification-harness-architecture.md`](phase-verification-harness-architecture.md)
- [`RISK-exact-candidate-remote-build-identity.md`](RISK-exact-candidate-remote-build-identity.md)
- [`RISK-source-font-startup.md`](RISK-source-font-startup.md)
- [`RISK-ttf-parser-unmaintained.md`](RISK-ttf-parser-unmaintained.md)

本索引只保证文件可发现性；每份报告中的结论和状态必须结合其生成时间、source/candidate
identity 与后续 release notes 阅读，不能凌驾于当前 `docs/plan/`。
