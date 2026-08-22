# coverage-matrix.md - 契约覆盖矩阵

> Plan Contract ↔ Feature Projection ↔ Acceptance Case ↔ Code Area ↔ Current Evidence。
> `Current Evidence` 只描述已验证范围；不得用模块存在代替端到端验收。

| Plan | Feature（投影） | Acceptance | Code Area | Current Evidence |
| --- | --- | --- | --- | --- |
| `02_positioning_and_scope.md` | 产品定位、便签模型 | AC-001、AC-026、AC-027 | `stickymd-win/{startup,platform/windows/program_dir.rs,platform/windows/single_instance.rs}` | copied Release EXE portable bootstrap + same/different-directory instance smoke PASS；full manual matrix pending |
| `04_runtime_state_model.md` | 内部 authority、全局内容缩放配置权威 | AC-009、AC-013、AC-032 | planned: `stickymd-win/{config,flow/preferences,app/*}` + `stickymd-render/{source,preview,math,image}` | Phase 10 contract amended；implementation/evidence pending |
| `05_document_persistence.md` | 自动保存、外部修改冲突、崩溃恢复 | AC-001、AC-005..008、AC-026、AC-027、AC-030 | `stickymd-core/src/persistence.rs`; `stickymd-win/{startup,config,persistence,flow/{persistence,reconciliation,recovery,save}.rs,app/{persistence,reconciliation,recovery}_runtime.rs,platform/windows/{atomic_file,file_identity,file_watch,program_dir,single_instance}.rs}` | automated invariants + Release stage benchmark + copied portable smoke PASS；AC-030 deterministic kill-during-publish remains conditional in Phase 4 report |
| `06_markdown_math_rendering.md` | 预览、数学、raw HTML、remote 图片 | AC-013..017 | `stickymd-render/src/{preview,math,image}/*`; `stickymd-win/{preview,flow/preview.rs,app/{preview_runtime,preview_input}.rs,platform/windows/shell.rs}` | Phase 5 owned AST/native Preview + Phase 6 RaTeX + Phase 7 bounded local-image projection automated；formula/image visual and same-process first-use memory manual NOT TESTED |
| `07_editor_and_ime.md` | 源码输入、中文输入法、Undo/Redo、传统剪贴板快捷键、内容缩放输入 | AC-002、AC-003、AC-004、AC-009、AC-022、AC-031、AC-032 | planned: `stickymd-render/src/source/*`; `stickymd-win/{instruction,flow,interaction,app/input.rs}` | Phase 10 aliases/zoom pending；AC-003/004 real IME NOT TESTED |
| `08_assets_and_export.md` | 图片粘贴、managed GC、导出 | AC-010、AC-011、AC-012、AC-017、AC-018 | `stickymd-core/src/assets.rs`; `stickymd-render/src/{image,preview/export}.rs`; `stickymd-win/{assets,export,app/{assets,export}_runtime.rs,platform/windows/{clipboard,export_dialog,managed_file}.rs}` | handle-bound ownership/scanner/bounded paste OCC/reversed Undo effects/durable safe-boundary GC/live-raster cache/source-preserving export automated PASS；real clipboard, visual, dialog and crash rows NOT TESTED |
| `09_windows_shell.md` | dock、托盘、置顶、透明度、主题、多显示器、紧凑窗口、Tool Window | AC-019..029、AC-033..036 | planned: `stickymd-win/{flow/window,config,app/{lifecycle,window_runtime,window_interaction,window_geometry_runtime,controls}.rs,platform/windows/{tray,monitor,native_message,window_identity,window_opacity,window_topmost}.rs}` | Phase 10 geometry/platform corrections pending；real shell/IME/visual rows remain NOT TESTED |
| `10_performance_reliability.md` | 质量属性 | AC-022、空闲/内存观察 | core/source/preview/math/image/export/window Release baselines | Phase 9 current-code latency and copied-Release resource reports complete；cold p95 PASS；warm p95 BLOCKED；machine-specific values are evidence, not portable promises |
| `11_testing_and_release.md` | 逐阶段验证、JSON evidence、发布形态 | `phase-00.md`..`phase-10.md`、release 验收清单 | planned: `tools/stickymd-smoke`; `tools/smoke/*.ps1`; Windows CI | Phase 10 Rust authority/JSON consolidation pending；manual modes remain excluded/NOT TESTED |

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
