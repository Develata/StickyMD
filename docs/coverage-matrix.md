# coverage-matrix.md - 契约覆盖矩阵

> Plan Contract ↔ Feature Projection ↔ Acceptance Case ↔ Code Area ↔ Current Evidence。
> `Current Evidence` 只描述已验证范围；不得用模块存在代替端到端验收。

| Plan | Feature（投影） | Acceptance | Code Area | Current Evidence |
| --- | --- | --- | --- | --- |
| `02_positioning_and_scope.md` | 产品定位、便签模型 | AC-001、AC-026、AC-027 | `stickymd-win/{startup,platform/windows/program_dir.rs,platform/windows/single_instance.rs}` | copied Release EXE portable bootstrap + same/different-directory instance smoke PASS；full manual matrix pending |
| `04_runtime_state_model.md` | 内部 authority、全局内容缩放与 Split 同步配置权威 | AC-009、AC-013、AC-032、AC-037 | `stickymd-win/{config/runtime.rs,config/coordinator.rs,flow/preferences.rs,app/{input,preview_runtime,presentation,window_runtime}.rs}`；`stickymd-render/{scroll.rs,source/projection.rs,preview/pipeline.rs}` | constrained Config authority、source/preview relayout、no-generation/no-reparse、Split sync default/persistence 与 50/100/300% toolbar paint/hit alignment targeted tests PASS；real zoom/IME visual NOT TESTED |
| `05_document_persistence.md` | 自动保存、外部修改冲突、崩溃恢复 | AC-001、AC-005..008、AC-026、AC-027、AC-030 | `stickymd-core/src/persistence.rs`; `stickymd-win/{startup,config,persistence,flow/{persistence,reconciliation,recovery,save}.rs,app/{persistence,reconciliation,recovery}_runtime.rs,platform/windows/{atomic_file,file_identity,file_watch,program_dir,single_instance}.rs}` | automated invariants + Release stage benchmark + occupied recovery/config evidence no-replace、Keep Local force-only receipt/resubmit、missing-canonical recovery receipt/note ack barrier/fixed-temp hard-link regressions PASS；AC-030 deterministic kill-during-publish remains conditional in Phase 4 report |
| `06_markdown_math_rendering.md` | 预览、数学、raw HTML、remote 图片、Split 语义同步 | AC-013..017、AC-037 | `stickymd-render/src/{scroll.rs,preview,math,image}/*`; `stickymd-render/tests/{rendering_stress.rs,fixtures/rendering-stress.md}`; `stickymd-win/{preview,flow/preview.rs,app/{input,preview_runtime,preview_input}.rs,platform/windows/shell.rs}` | Phase 5 owned AST/native Preview + Phase 6 RaTeX + Phase 7 bounded local-image projection；多行/Unicode grapheme 选择隔离、borrowed foreground、formula-source sharing、generation-local 双向锚点回归 PASS；完整 exact-candidate visual pending |
| `07_editor_and_ime.md` | 源码输入、中文输入法、Undo/Redo、剪贴板、内容缩放、数学分隔符转换、纯文本查找替换 | AC-002、AC-003、AC-004、AC-009、AC-022、AC-031、AC-032、AC-038、P11B-A01..A05 | `stickymd-render/src/{source,preview/semantic_conversion.rs}`; `stickymd-win/{source_search.rs,instruction/intent.rs,flow/editor.rs,flow/preferences.rs,interaction/search.rs,app/{search_controller,search_runtime,input,preview_input,presentation,window_interaction}.rs}` | existing editor/conversion regressions PASS；literal search streaming KMP、Unicode boundary、wrap/stale/replace/one-Undo、closed-session allocation release、external-resync refresh and global save/export routing tests PASS；AC-003/004 real IME and current-candidate visual remain NOT TESTED |
| `08_assets_and_export.md` | 图片粘贴、managed GC、导出 | AC-010、AC-011、AC-012、AC-017、AC-018 | `stickymd-core/src/assets.rs`; `stickymd-render/src/{image,preview/export}.rs`; `stickymd-win/{assets,export,app/{assets,export}_runtime.rs,platform/windows/{clipboard,export_dialog,managed_file}.rs}` | handle-bound ownership/scanner/bounded paste OCC/reversed Undo effects/durable safe-boundary GC/live-raster cache/source-preserving export、no-replace staging 与 observed-handle cleanup automated PASS；real clipboard, visual, dialog and crash rows NOT TESTED |
| `09_windows_shell.md` | dock、托盘、置顶/auto-hide 正交性、透明度、主题、多显示器、紧凑窗口、Tool Window | AC-019..029、AC-033..036、P11B-A06 | `stickymd-win/{flow/window,config,app/{lifecycle,window_runtime,window_interaction,window_geometry_runtime,controls,toolbar_paint}.rs,platform/windows/{tool_window,tray,monitor,native_message,window_opacity,window_topmost}.rs}` | Pin/auto-hide parameterized transition-equivalence、floating exclusion、220×120 geometry、24-DIP nearest/tie reducer、40% alpha and copied HWND Tool/App/NOACTIVATE facts PASS；real shell/IME/visual rows NOT TESTED |
| `10_performance_reliability.md` | 质量属性 | AC-022、AC-032、空闲/内存观察、P14-A03/A08/A15/A23 | core/source/preview/math/image/export/window/zoom Release baselines；`stickymd-smoke` copied-runtime metrics/attribution；`docs/report/{phase-14-memory-attribution,phase-14-global-module-review,phase-14-global-module-rereview}.md` | v0.1.0 采用 180 preferred / 400 engineering diagnostic / 550 hard 三层 startup policy；当前 dirty-worktree focused Source/Preview/Split PWS 约 13.0/15.4/21.0 MiB 且 idle CPU p95 ≤0.0053% PASS；关闭 search 后释放约 2 MiB 上限的匹配投影且停止 O(n) 隐式扫描；新 exact candidate 仍需完整资源重跑 |
| `11_testing_and_release.md` | 逐阶段验证、exact-artifact JSON evidence、发布形态 | `phase-00.md`..`phase-14.md`、release 验收清单 | `tools/stickymd-smoke`; `tools/smoke/*.ps1`; `.github/workflows/ci.yml`; `dist/evidence/*` | Rust task/gate/evidence authority、independent channel collection、typed hidden-window/view-mode/lifecycle reducer、source-projection gate、至少 100-run 的 desktop-jitter `≥98%` PASS / `<98%` FAIL disposition、Tier A/B/C readiness、G1..G3 guided manual、unsigned receipt contract；旧 candidate 已失效，dynamic/manual evidence pending new exact Phase 14 candidate |

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
