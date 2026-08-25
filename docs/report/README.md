# Report 索引

本目录存放有时间属性的分析证据（风险报告、阶段分析、冲突报告）。
目录规则见 [`AGENTS.md`](AGENTS.md)。

## 现有报告

| 文件 | 类型 | 状态 |
| --- | --- | --- |
| [`phase-00-repository-governance-check.md`](phase-00-repository-governance-check.md) | Phase 0 现状检查记录 | 无冲突，已完成 |
| [`phase-00-governance-revalidation.md`](phase-00-governance-revalidation.md) | Phase 0 takeover 契约复核 | PASS after targeted corrections |
| [`phase-00-03-architecture-convergence.md`](phase-00-03-architecture-convergence.md) | Phase 0–3 实现对齐与架构收敛审计 | automated PASS；manual gates open |
| [`phase-01-technical-spike-report.md`](phase-01-technical-spike-report.md) | Phase 1 重建总报告 | conditional；manual/environment gates open |
| [`phase-01-dependency-baseline.md`](phase-01-dependency-baseline.md) | Phase 1 依赖审计 | refreshed and corrected |
| [`phase-01-performance-baseline.md`](phase-01-performance-baseline.md) | Phase 1 重测性能 | local engineering evidence |
| [`phase-01-windows-api-baseline.md`](phase-01-windows-api-baseline.md) | Phase 1 Win32 边界 | conservative adapter verified |
| [`phase-02-core-document-model.md`](phase-02-core-document-model.md) | Phase 2 重建结果 | automated PASS |
| [`phase-03-dependency-delta.md`](phase-03-dependency-delta.md) | Phase 3 依赖审计 | audited |
| [`phase-03-source-editor-ime.md`](phase-03-source-editor-ime.md) | Phase 3 实现与测量 | automated PASS；manual gate open |
| [`phase-03-manual-ime-checklist.md`](phase-03-manual-ime-checklist.md) | 真实输入法验收表 | NOT TESTED |
| [`phase-04-dependency-delta.md`](phase-04-dependency-delta.md) | Phase 4 依赖增量审计 | audited |
| [`phase-04-portable-persistence.md`](phase-04-portable-persistence.md) | Phase 4 portable persistence 结果与收据 | automated/portable PASS；manual conditions open |
| [`phase-05-dependency-delta.md`](phase-05-dependency-delta.md) | Phase 5 Comrak 依赖增量审计 | audited |
| [`phase-05-markdown-native-preview.md`](phase-05-markdown-native-preview.md) | Phase 5 Markdown/native Preview 结果与收据 | automated PASS；manual conditions open |
| [`phase-06-dependency-delta.md`](phase-06-dependency-delta.md) | Phase 6 RaTeX 依赖增量审计 | exact-pinned and audited |
| [`phase-06-ratex-native-math.md`](phase-06-ratex-native-math.md) | Phase 6 native math 结果与收据 | automated PASS；manual conditions open |
| [`phase-07-dependency-delta.md`](phase-07-dependency-delta.md) | Phase 7 image/clipboard 依赖增量审计 | minimal codec graph audited |
| [`phase-07-windows-clipboard-formats.md`](phase-07-windows-clipboard-formats.md) | Phase 7 Windows clipboard 格式审计 | adapter automated；real sources NOT TESTED |
| [`phase-07-managed-images-export.md`](phase-07-managed-images-export.md) | Phase 7 managed image/export 结果与收据 | automated PASS；manual conditions open |
| [`phase-08-windows-desktop-shell.md`](phase-08-windows-desktop-shell.md) | Phase 8 native Windows shell 结果与收据 | automated PASS；manual conditions open |
| [`phase-09-performance-final.md`](phase-09-performance-final.md) | Phase 9 Release 性能与资源证据 | all measured gates pass except warm startup |
| [`phase-09-release-readiness.md`](phase-09-release-readiness.md) | Phase 9 发布收口结论 | NOT RC READY；warm/manual blockers open |
| [`phase-10-automation-consolidation.md`](phase-10-automation-consolidation.md) | Rust smoke/JSON/CI 收敛 | automated architecture PASS |
| [`phase-10-ux-corrections.md`](phase-10-ux-corrections.md) | Phase 10 十项交互修订 | implementation/automated runtime PASS |
| [`phase-10-startup-requalification.md`](phase-10-startup-requalification.md) | 30+30 启动重新资格化 | cold 400 ms PASS；warm 180 ms FAIL |
| [`phase-10-rc-requalification.md`](phase-10-rc-requalification.md) | Phase 10 本地候选总证据 | NOT RC READY |
| [`phase-14-split-sync-find-replace-scope-question.md`](phase-14-split-sync-find-replace-scope-question.md) | Split 同步滚动与 Source 查找替换的范围/架构分析 | awaiting USER contract decision |
| [`phase-verification-harness-architecture.md`](phase-verification-harness-architecture.md) | 逐阶段 smoke 与矩阵治理决策 | USER approved；落实到 plan 11 |
| [`RISK-ttf-parser-unmaintained.md`](RISK-ttf-parser-unmaintained.md) | 字体解析依赖维护风险 | exact advisory temporarily acknowledged |
