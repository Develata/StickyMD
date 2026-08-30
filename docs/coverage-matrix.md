# coverage-matrix.md - 契约覆盖矩阵

> Plan Contract ↔ Feature Projection ↔ Acceptance Case ↔ Code Area ↔ Current Evidence。
> `Current Evidence` 只描述已验证范围；不得用模块存在代替端到端验收。
>
> 当前快照以已发布 `v0.1.0` exact source
> `64690ab8f86f63f3cbfeabbb0961276978c8f26d` 为 evidence boundary。后续文档提交不自动
> 继承该 artifact identity，也不要求把 ignored 动态 receipt 回填到历史 Phase matrix；发布身份、
> USER disposition 与剩余环境缺口见 [`release-notes/0.1.0.md`](release-notes/0.1.0.md)。

| Plan | Feature（投影） | Acceptance | Code Area | Current Evidence |
| --- | --- | --- | --- | --- |
| `02_positioning_and_scope.md` | 产品定位、便签模型 | AC-001、AC-026、AC-027 | `stickymd-win/{startup,platform/windows/program_dir.rs,platform/windows/single_instance.rs}` | v0.1.0 copied Release portable bootstrap、same-directory wake 与 different-directory multi-instance exact automation PASS |
| `04_runtime_state_model.md` | 内部 authority、全局内容缩放与 Split 同步配置权威 | AC-009、AC-013、AC-032、AC-037 | `stickymd-win/{config/runtime.rs,config/coordinator.rs,flow/preferences.rs,app/{input,preview_runtime,presentation,window_runtime}.rs}`；`stickymd-render/{scroll.rs,source/projection.rs,preview/pipeline.rs}` | constrained Config authority、source/preview relayout、no-generation/no-reparse、Split sync default/persistence 与 50/100/300% toolbar paint/hit alignment PASS；候选窗主观视觉仍属人工 |
| `05_document_persistence.md` | 自动保存、外部修改冲突、崩溃恢复 | AC-001、AC-005..008、AC-026、AC-027、AC-030 | `stickymd-core/src/persistence.rs`; `stickymd-win/{startup,config,persistence,flow/{persistence,reconciliation,recovery,save}.rs,app/{persistence,reconciliation,recovery}_runtime.rs,platform/windows/{atomic_file,file_identity,file_watch,program_dir,single_instance}.rs}` | automated invariants + Release stage benchmark + occupied recovery/config evidence no-replace、Keep Local force-only receipt/resubmit、missing-canonical recovery receipt/note ack barrier/fixed-temp hard-link regressions PASS；AC-030 deterministic kill-during-publish remains conditional in Phase 4 report |
| `06_markdown_math_rendering.md` | 预览、数学、raw HTML、remote 图片、Split 语义同步、精确 Preview selection | AC-013..017、AC-037、P14-A28 | `stickymd-render/src/{scroll.rs,preview,math,image}/*`; `stickymd-render/tests/{rendering_stress.rs,fixtures/rendering-stress.md}`; `stickymd-win/{preview,flow/preview.rs,app/{input,preview_runtime,preview_input}.rs,platform/windows/shell.rs}` | Phase 5 owned AST/native Preview + Phase 6 RaTeX + Phase 7 bounded local-image projection PASS；viewport cluster map、visual-row locator 与 frame semantic API 已实现，Times/CJK/Emoji/combining/BiDi/多行/atomic tests、5,000-row Release baseline 与 v0.1.0 guided selection observation PASS |
| `07_editor_and_ime.md` | 源码输入、中文输入法、Undo/Redo、剪贴板、内容缩放、数学分隔符转换、纯文本查找替换 | AC-002、AC-003、AC-004、AC-009、AC-022、AC-031、AC-032、AC-038、P11B-A01..A05、P14-A29/A30 | `stickymd-render/src/{source,preview/semantic_conversion.rs}`; `stickymd-win/{source_search.rs,instruction/intent.rs,flow/editor.rs,flow/preferences.rs,interaction/search.rs,app/{search_controller,search_runtime,input,preview_input,presentation,window_interaction}.rs}`；`stickymd-smoke/{window_control/ime_profile.rs,qualification/g4/cases/ime.rs}` | literal search algorithm/transaction、单 session Ctrl+F/Ctrl+H reducer、Find-only guard、字段 paint/hit/caret/IME 共用 layout 与源码 caret 隔离 PASS；Microsoft Pinyin/WeType 客观功能由 G4-06 exact automation PASS，候选窗纯视觉由人工持有 |
| `08_assets_and_export.md` | 图片粘贴、managed GC、导出 | AC-010、AC-011、AC-012、AC-017、AC-018 | `stickymd-core/src/assets.rs`; `stickymd-render/src/{image,preview/export}.rs`; `stickymd-win/{assets,export,app/{assets,export}_runtime.rs,platform/windows/{clipboard,export_dialog,managed_file}.rs}` | handle-bound ownership/scanner/bounded paste OCC/reversed Undo effects/durable safe-boundary GC/live-raster cache/source-preserving export、standard clipboard/native dialog/process-kill recovery/user-asset safety exact automation PASS；主观图片观感由 guided manual PASS |
| `09_windows_shell.md` | dock、托盘、置顶/auto-hide 正交性、透明度、主题、多显示器、紧凑窗口、Tool Window | AC-019..029、AC-033..036、P11B-A06 | `stickymd-win/{flow/window,config,app/{lifecycle,window_runtime,window_interaction,window_geometry_runtime,controls,toolbar_paint}.rs,platform/windows/{tool_window,tray,monitor,native_message,window_opacity,window_topmost}.rs}` | Pin/auto-hide、三边 Dock/timing、tray lifecycle、220×120 geometry、24-DIP nearest/tie、40% alpha、Tool Window 与 guided visual PASS；Clean VM、真实双屏/mixed DPI/拔屏采用 v0.1.0 USER waiver，RDP/物理负坐标为 Tier C NOT TESTED |
| `10_performance_reliability.md` | 质量属性 | AC-022、AC-032、空闲/内存观察、P14-A03/A08/A15/A23 | core/source/preview/math/image/export/window/zoom Release baselines；`stickymd-smoke` copied-runtime metrics/attribution；`docs/report/{phase-14-memory-attribution,phase-14-global-module-review,phase-14-global-module-rereview}.md` | v0.1.0 采用 180 preferred / 400 engineering diagnostic / 550 hard 三层 startup policy；正式 warm-cache 间隔 1000 ms，250 ms rapid-restart 仅诊断；五个独立 Release 进程的 Source/Preview/Split median PWS 为 12.98/15.50/20.89 MiB，idle CPU p95 0–0.0027% PASS；关闭 search 后释放约 2 MiB 上限匹配投影并停止 O(n) 隐式扫描 |
| `11_testing_and_release.md` | 逐阶段验证、结果驱动等待、可证明并发、GUI child 进程隔离、Source Freeze、remote artifact promotion、module input fingerprint、last-success ledger、exact-artifact JSON evidence、发布形态 | `phase-00.md`..`phase-14.md`、P12-A09/A11/A13/A14/A18、P14-A31..A35、release 验收清单 | `tools/stickymd-smoke`; `tools/smoke/*.ps1`; `.github/workflows/{ci,release,promote-release}.yml`; `dist/evidence/*`; `dist/exact-candidate/*` | Rust task/gate/evidence authority、headless isolated shards 与 targeted module 已落地；Promoted Candidate 继续持有 exact bytes，Runtime/Performance/Resources/G3/G4/G5 按相关输入指纹复用 last-success；成功 evidence 先归档再原子切换 ledger，失败/中止不覆盖；tag/draft/publish 必须复用已验收 artifact 且不得重建 |

---

## 未覆盖声明

- `00_engineering_constitution.md` 与 `01_terminology.md` 是治理基座，
  不直接映射验收案例；其约束通过上述所有章节间接验证。
- `03_system_architecture.md` 的层间规则通过 code review、`plan_ref` 审查
  与各案例的实现结构间接验证。

## 维护规则

1. 新增 plan 章节 → 必须补充对应 Feature 段落与 Acceptance 案例（或写明不适用理由）。
2. Acceptance 案例失效 → 标记 Deprecated，编号不复用。
3. Code Area 或验证状态变化 → 同步更新本表；部分实现不得标记完整 AC PASS。
4. 每个 Phase 新建时同步创建 `tools/smoke/phase-XX.ps1` 与
   `docs/acceptance-cases/phase-XX.md`；CI 只自动执行其中可无界面运行的部分。
