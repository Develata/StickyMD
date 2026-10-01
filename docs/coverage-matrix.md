# coverage-matrix.md - 契约覆盖矩阵

> Plan Contract ↔ Feature Projection ↔ Acceptance Case ↔ Code Area ↔ Current Evidence。
> `Current Evidence` 只描述已验证范围；不得用模块存在代替端到端验收。
>
> 下方主表保留 `v0.1.0` exact source `64690ab8f86f63f3cbfeabbb0961276978c8f26d`
> 的历史 evidence baseline；后续维护按带日期的小节追加，不自动继承该 artifact identity，
> 也不把 ignored 动态 receipt 回填到历史 Phase matrix。旧版处置见
> [`0.1.0 release notes`](release-notes/0.1.0.md)。当前公开版本为 `v0.1.1`，采用 USER
> 发布特例且技术 readiness 仍为 `NOT_READY`，身份和缺口见
> [`0.1.1 release notes`](release-notes/0.1.1.md)；这不把主表的旧证据升级为新版验收。

| Plan | Feature（投影） | Acceptance | Code Area | Current Evidence |
| --- | --- | --- | --- | --- |
| `02_positioning_and_scope.md` | 产品定位、便签模型 | AC-001、AC-026、AC-027 | `stickymd-win/{startup,platform/windows/program_dir.rs,platform/windows/single_instance.rs}` | v0.1.0 copied Release portable bootstrap、same-directory wake 与 different-directory multi-instance exact automation PASS |
| `04_runtime_state_model.md` | 内部 authority、全局内容缩放与 Split 同步配置权威 | AC-009、AC-013、AC-032、AC-037 | `stickymd-win/{config/runtime.rs,config/coordinator.rs,flow/preferences.rs,app/{input,preview_runtime,presentation,window_runtime}.rs}`；`stickymd-render/{scroll.rs,source/projection.rs,preview/pipeline.rs}` | constrained Config authority、source/preview relayout、no-generation/no-reparse、Split sync default/persistence 与 50/100/300% toolbar paint/hit alignment PASS；候选窗主观视觉仍属人工 |
| `05_document_persistence.md` | 自动保存、外部修改冲突、崩溃恢复 | AC-001、AC-005..008、AC-026、AC-027、AC-030 | `stickymd-core/src/persistence.rs`; `stickymd-win/{startup,config,persistence,flow/{persistence,reconciliation,recovery,save}.rs,app/{persistence,reconciliation,recovery}_runtime.rs,platform/windows/{atomic_file,file_identity,file_watch,program_dir,single_instance}.rs}` | automated invariants + Release stage benchmark + occupied recovery/config evidence no-replace、Keep Local force-only receipt/resubmit、missing-canonical recovery receipt/note ack barrier/fixed-temp hard-link regressions PASS；AC-030 deterministic kill-during-publish remains conditional in Phase 4 report |
| `06_markdown_math_rendering.md` | 预览、数学、raw HTML、remote 图片、Split 语义同步、精确 Preview selection | AC-013..017、AC-037、P14-A28 | `stickymd-render/src/{scroll.rs,preview,math,image}/*`; `stickymd-render/tests/{rendering_stress.rs,fixtures/rendering-stress.md}`; `stickymd-win/{preview,flow/preview.rs,app/{input,preview_runtime,preview_input}.rs,platform/windows/shell.rs}` | Phase 5 owned AST/native Preview + Phase 6 RaTeX + Phase 7 bounded local-image projection PASS；viewport cluster map、visual-row locator 与 frame semantic API 已实现，Times/CJK/Emoji/combining/BiDi/多行/atomic tests、5,000-row Release baseline 与 v0.1.0 guided selection observation PASS |
| `07_editor_and_ime.md` | 源码输入、中文输入法、Undo/Redo、剪贴板、内容缩放、数学分隔符转换、纯文本查找替换 | AC-002、AC-003、AC-004、AC-009、AC-022、AC-031、AC-032、AC-038、P11B-A01..A05、P14-A29/A30 | `stickymd-render/src/{source,preview/semantic_conversion.rs}`; `stickymd-win/{source_search.rs,instruction/intent.rs,flow/editor.rs,flow/preferences.rs,interaction/search.rs,app/{search_controller,search_runtime,input,preview_input,presentation,window_interaction}.rs}`；`stickymd-smoke/{window_control/ime_profile.rs,qualification/g4/cases/ime.rs}` | literal search algorithm/transaction、单 session Ctrl+F/Ctrl+H reducer、Find-only guard、字段 paint/hit/caret/IME 共用 layout 与源码 caret 隔离 PASS；Microsoft Pinyin/WeType 客观功能由 G4-06 exact automation PASS，候选窗纯视觉由人工持有 |
| `08_assets_and_export.md` | 图片粘贴、managed GC、导出 | AC-010、AC-011、AC-012、AC-017、AC-018 | `stickymd-core/src/assets.rs`; `stickymd-render/src/{image,preview/export}.rs`; `stickymd-win/{assets,export,app/{assets,export}_runtime.rs,platform/windows/{clipboard,export_dialog,managed_file}.rs}` | handle-bound ownership/scanner/bounded paste OCC/reversed Undo effects/durable safe-boundary GC/live-raster cache/source-preserving export、standard clipboard/native dialog/process-kill recovery/user-asset safety exact automation PASS；主观图片观感由 guided manual PASS |
| `09_windows_shell.md` | dock、托盘、置顶/auto-hide 正交性、透明度、主题、多显示器、紧凑窗口、Tool Window | AC-019..029、AC-033..036、P11B-A06 | `stickymd-win/{flow/window,config,app/{lifecycle,window_runtime,window_interaction,window_geometry_runtime,controls,toolbar_paint}.rs,platform/windows/{tool_window,tray,monitor,native_message,window_opacity,window_topmost}.rs}` | Pin/auto-hide、三边 Dock/timing、tray lifecycle、220×120 geometry、24-DIP nearest/tie、40% alpha、Tool Window 与 guided visual PASS；Clean VM、真实双屏/mixed DPI/拔屏采用 v0.1.0 USER waiver，RDP/物理负坐标为 Tier C NOT TESTED |
| `10_performance_reliability.md` | 质量属性 | AC-022、AC-032、空闲/内存观察、P14-A03/A08/A15/A23/A40 | core/source/preview/math/image/export/window/zoom Release baselines；`stickymd-smoke` copied-runtime metrics/attribution；`docs/report/{phase-14-memory-attribution,phase-14-global-module-review,phase-14-global-module-rereview}.md` | v0.1.0 采用 180 preferred / 400 engineering diagnostic / 550 hard 三层 startup policy；正式 warm-cache 间隔 1000 ms，250 ms rapid-restart 仅诊断；五个独立 Release 进程的 Source/Preview/Split median PWS 为 12.98/15.50/20.89 MiB，idle CPU p95 0–0.0027% PASS；关闭 search 后释放约 2 MiB 上限匹配投影并停止 O(n) 隐式扫描；窗口压力测试须先恢复固定文档再做隐藏采样 |
| `11_testing_and_release.md` | 逐阶段验证、结果驱动等待、可证明并发、GUI child 进程隔离、Source Freeze、remote artifact promotion、module input fingerprint、last-success ledger、exact-artifact JSON evidence、发布形态 | `phase-00.md`..`phase-14.md`、P12-A09/A11/A13/A14/A18、P14-A31..A36、release 验收清单 | `tools/stickymd-smoke`; `tools/smoke/*.ps1`; `.github/workflows/{ci,release,promote-release}.yml`; `dist/evidence/*`; `dist/exact-candidate/*` | Rust task/gate/evidence authority、headless isolated shards 与 targeted module 已落地；Promoted Candidate 继续持有 exact bytes，Runtime/Performance/Resources/G3/G4/G5 按相关输入指纹复用 last-success；成功 evidence 先归档再原子切换 ledger，失败/中止保留本轮诊断 JSON 但不覆盖成功账本；tag/draft/publish 必须复用已验收 artifact 且不得重建 |

---

## 未覆盖声明

- `00_engineering_constitution.md` 与 `01_terminology.md` 是治理基座，
  不直接映射验收案例；其约束通过上述所有章节间接验证。
- `03_system_architecture.md` 的层间规则通过 code review、`plan_ref` 审查
  与各案例的实现结构间接验证。

## 2026-09-30 Preview 现状说明与覆盖边界

plan 06 `markdown-semantics` / `native-preview-layout` / `preview-scheduling`、plan 08
`local-image-read-boundary` → 既有 AC-013..017 与 Phase 05/06/07 →
[Preview 规则详解](report/2026-09-30-preview-rendering-rules.md)。该报告将 parser、RenderTree、
文字/表格/数学/图片布局、worker 与平台 adapter 的现状集中说明，补充导航，不新增产品合同。

在 source `be1322e43624908b7ad04c7c7942d2c3ecb9ed09` 上执行 `phase5_semantics`、
`table_math_pipes`、`phase6_math`、`rendering_stress`，共 21 passed。现有“native layout”
自动化覆盖不能解读为所有视觉条款均符合：报告列出 code 长行换行、表格横向滚动/交替底色、
monospace 字体绑定、复杂列表投影、zoom 布局范围、图片读取边界等差异或风险。
其中 UNC 的实际网络访问未测试；本次不修改 runtime，不提升人工或候选验收状态。

## 2026-10-01 文档复审与诊断边界

沿用上节 plan / AC 映射，详见 [Preview 勘误](report/2026-09-30-preview-rendering-rules.md#review-2026-10-01)。
source `ae2e0aa26cec6265bb2d1e594c5c21b75d97f46f` 的 runtime 与上述 `be1322e` 相同。
本次纠正全局绘制顺序、Source 返回 Preview 的 Build 路由与 file URI 分支范围；五组
headless 合成输入诊断确认了列表内首个表格只剩 marker 的投影/复制结果，并记录本机
generic monospace 的实际字体回退。它们是诊断观察，不是新增五项通过测试或完整符合性证明。
阶段矩阵同步明确 tracked source baseline、ignored 动态收据和各版本发布结论的区别；
原状态列与历史数值不变，人工项未升级。

## 2026-10-01 Preview 实现修复

- plan 08 `local-image-read-boundary` → AC-012/017、Phase 07 A09/A11/A13/A15 →
  `assets/path.rs`、`platform/windows/local_image_file{,/path,/tests}.rs`、Preview worker 与 Export：
  同一个只读 opener 拒绝 UNC/device/远程或未知 drive/reparse；真实本地句柄、junction、
  中文/空格、文件替换及失败保留测试补足仅检查“无网络 client”的覆盖缺口。
- plan 06 `owned-ast-projection` / `native-preview-layout` / `preview-scheduling` →
  AC-013/014、Phase 05 → `preview/render_tree/list.rs`、`layout/list.rs`、`fonts.rs`：
  列表首块保留原语义，表格 cells 与复制内容不丢失；标记与内容命中独立；Consolas 显式
  绑定且缺失时保留本地 fallback。测试包括实际 glyph face 和 headless frame。

本地图片定向 20 passed / 2 ignored；渲染、开发检查和 raster 的本批结果详见
[修复记录](report/2026-09-30-preview-rendering-rules.md#fixes-2026-10-01)。局部自动化与生成
图像检查不升级历史矩阵或人工验收；本地重定向/云占位的兼容性限制、真实网络与桌面矩阵
缺口均保留，未宣称性能提升或完整符合性。

## 维护规则

1. 新增 plan 章节 → 必须补充对应 Feature 段落与 Acceptance 案例（或写明不适用理由）。
2. Acceptance 案例失效 → 标记 Deprecated，编号不复用。
3. Code Area 或验证状态变化 → 同步更新本表；部分实现不得标记完整 AC PASS。
4. 每个 Phase 新建时同步创建 `tools/smoke/phase-XX.ps1` 与
   `docs/acceptance-cases/phase-XX.md`；CI 只自动执行其中可无界面运行的部分。

## 2026-09-29 headless 并行维护投影

plan 11 `phase-verification-harness` / `modular-headless-ci` → P00-A09/A10 与 REL-CLI-05：
两版 PowerShell wrapper 用隔离目录、子进程和只读 Git 查询并发；Linux smoke 从 planner 拆到
独立 job，选择与完整结果聚合继续由 Rust 持有。本地测试、耗时对照及远程未验证边界见
[并行维护报告](report/2026-09-29-headless-test-parallelism.md)。GUI、startup 和资源测量继续独占，
这些工具测试不产生产品或候选资格化证据。

同日后续维护：plan 11 `phase-verification-harness` → P00-A07/A10 →
`tools/stickymd-smoke/tests/headless.rs`。CLI、包路径和发布 wrapper 合并为一个 Cargo
集成测试 target，保持原有用例并集并允许默认 test harness 并发；完整清单对照、子进程隔离、
本机小幅收益以及已撤回的收据并行试验见
[集成测试合并报告](report/2026-09-29-headless-integration-consolidation.md)。

发布声明输出维护：plan 11 `phase-verification-harness` → REL-CLI-05 →
`tools/stickymd-smoke/src/release/notices/`。无效目标在读取依赖前拒绝，最终原子
no-replace 发布仍持有防覆盖责任；无 manifest 的失败路径回归、两版 PowerShell 兼容性与
分项耗时见[输出预检报告](report/2026-09-29-notices-preflight-optimization.md)。

## 2026-09-25 clipboard feedback maintenance coverage

2026-09-25 剪贴板反馈修复：`07_editor_and_ime.md#source-editor` → 输入 Markdown 行为投影 →
[Phase 03 clipboard feedback regression](acceptance-cases/phase-03.md#2026-09-25-clipboard-feedback-regression)
→ `stickymd-win/src/app.rs::apply_effect`。成功复制不再遮挡源码或覆盖既有错误；
剪贴板内容与失败原子性由现有 Windows 模块测试覆盖，真实桌面观察仍按 Phase 03 单列。

## 2026-09-07 maintenance coverage

本次在既有合同下补充回归，不增加产品能力或更新已发布 artifact 的资格化状态。

| Plan / feature mapping | Acceptance projection | Code / evidence |
| --- | --- | --- |
| 07 Undo / AC-009 | Phase 02 maintenance coverage | `stickymd-core/src/undo.rs`: 750 ms and newline deletion boundaries |
| 06 native preview / Markdown rendering | Phase 05 maintenance coverage | semantic image detection, immutable text row locator, visible-row glyph paint; serial Phase 05 Release scroll benchmarks |
| 06 bounded math raster / math error source preservation | Phase 06 maintenance coverage | lease-aware cache admission, allocation preflight and pressure/release tests |
| 06/10 cache lifecycle / Content Zoom | Phase 10 maintenance coverage | Source/Preview discard obsolete glyph scales; Preview purge releases text rasters |

当前维护结果与未验证项见 [2026-09-07 audit](report/2026-09-07-runtime-audit.md)。

2026-09-25 复审补充：Phase 05 覆盖多行组合符号墨迹的可见绘制，Phase 06 覆盖 LRU
计数器溢出时的位图租用与计账。当前验证见同一报告追加的 Resolution；历史发布资格不变。

## 2026-09-30 本地开发检查与耗时观察

| Contract / projection | Implementation and verification |
| --- | --- |
| plan 11 phase-verification-harness / modular-headless-ci → P00-A12 | `development/` 采集工作树输入，复用 `ci/selection` 分类、反向依赖与逐任务原因；`runner/headless/local` 复用原任务图。真实 Git 暂存/未暂存/新增/删除/移动、冲突、未知输入与 registry drift 回归；compiled `dev-check --plan` 只读拒绝路径 |
| plan 11 shared-headless-prerequisite / resource-module-qualification → P00-A13 | `timing_summary/` 复用 JSON parser，读取原 receipts/Cargo/任务/资源日志；本轮、嵌套、历史和预算分开。真实格式、CI ANSI 颜色、LF/CRLF/无末尾换行、缺失/重复/错误数值、Unicode 路径、任意 CWD 与多输入失败不输出部分摘要 |
| P00-A07/A10/A11、REL-CLI-10/16/17 | `tests/support` 统一独占临时目录、失败清理和 PowerShell 5.1/7 调用；原测试保留真实断言，包路径新增 PS7 覆盖。共享预构建 CLI，独立进程/输出目录可并发 |

这些入口属于本地工具诊断；部分检查不生成候选或人工成功收据。耗时汇总不证明历史来源有效，
不合计重叠作用域，也不估算 agent 工时。资源选测/失败优先/续跑继续使用既有入口和身份契约，
完整桌面矩阵仍由 P14-A46 持有。实际检查、数据和未验证项见
[本地开发验证报告](report/2026-09-30-local-development-verification.md)；没有推算整体工时收益。

## 2026-09-08 table formula maintenance coverage

| Plan / feature mapping | Acceptance projection | Code / evidence |
| --- | --- | --- |
| 06 GFM / canonical source ranges / formula copy; 07 math delimiter conversion; 08 source-preserving image export | Phase 05 table formula regression coverage, AC-013/014 | `preview/{parser,source_map}.rs`; `tests/table_math_pipes.rs`: escaped-pipe coordinate restoration, four delimiters, UTF-8, containers, copy, conversion and image rewrite |

本次只修复既有投影合同下的源码坐标；未转义 `|` 的 GFM 分列规则保持不变。自动化结果与未验证项见 [表格公式报告](report/2026-09-08-table-math-source-ranges.md)，不归属于已发布的 `v0.1.0` artifact。

## 2026-09-08 开发工具维护投影

| Plan | Feature（投影） | Acceptance | Code Area | Current Evidence |
| --- | --- | --- | --- | --- |
| `11_testing_and_release.md#phase-verification-harness` | 开发工具；不增加产品功能 | P00-A06 | `stickymd-smoke/src/{headless.rs,runner/headless.rs}` | 显式单/多模块入口、完整并集、共享命令去重、既有 Release 参数保留与未知目标拒绝 |
| `11_testing_and_release.md#modular-headless-ci` | 开发工具；普通 CI 与候选资格化分离 | P00-A08/A09 | `stickymd-smoke/src/ci/*`; `.github/workflows/{ci,scheduled}.yml`; `.github/actions/rust-cache/action.yml` | 已批准日常选测、反向依赖及全量回退；Git/分类/聚合/工作流本地回归；远程执行及耗时尚未验证 |
| `11_testing_and_release.md#release-artifact-authority` | 开发工具；不改变发布资产身份 | P00-A07 | `stickymd-smoke/src/{package_path.rs,repository.rs}`; `tools/release/package-path.ps1` | 包路径判断迁入 Rust；单包、多包、clean/dirty、错误路径和 Windows PowerShell 5.1 中文路径兼容回归 |

这些维护验证不继承 `v0.1.0` exact artifact 身份；新 CI 选测规则的批准记录见
[模块化 CI 影响分析](report/RISK-2026-09-08-modular-ci.md)。

## 2026-09-22 release CLI maintenance coverage

| Plan / existing acceptance mapping | Maintenance projection | Authoritative implementation / verification |
| --- | --- | --- |
| 11 release-artifact-authority / P14-A32/A34 | Phase 14 REL-CLI-01/02 | `stickymd-smoke/src/integrity.rs`、`release/identity.rs`、`release/promoted.rs`；candidate receipts 共用 checksum 实现 |
| 10 ZIP hard gate + 11 portable-windows-runtime / P09-D066..D082、P14-A19 | Phase 14 REL-CLI-03/06 | `release/package*.rs`、现有 PE parser/ChildGuard；ZIP/资源事实仍由 PowerShell 采集 |
| 11 dependency/release contract / P09-D061 | Phase 14 REL-CLI-04/05 | `release/notices/`；锁定 metadata、许可证失败路径、同输入字节比较、PowerShell 双版本测试 |
| 11 phase-verification-harness / package selection and staging | Phase 14 REL-CLI-07 | `release/package_inputs.rs`、`package_path.rs`、`repository.rs`；现有阶段入口和选测继续复用 |

工具修改与验证细节见 [2026-09-22 migration](report/2026-09-22-release-cli-migration.md)。
本次维护不产生 Source Freeze、Promoted Candidate、人工或远端发布证据。

同日 review 补充覆盖：`integrity` 的 ZIP/SBOM 角色重名、空文件、64 位十六进制路径词与 GNU escaped filename；
`repository` 的 workspace 版本作用域/赋值空白/歧义拒绝；`managed_process` 对
`stickymd-verify-*` 遗留测试进程的只读阻断。分别映射 REL-CLI-02/07/06，
Windows 双宿主 wrapper 使用含十六进制词、中文与空格的真实路径。

2026-09-25 复核补充 REL-CLI-05：`release/windows.rs` 为 Windows PowerShell 子进程恢复自身模块搜索环境；
双宿主 wrapper 注入冲突模块，验证 ZIP adapter 可用且父进程 `PSModulePath`、CWD、编码保持不变。
当日重新执行的工具测试和本地包检查见上述维护报告的追加 Resolution；不继承历史验收结论。

2026-09-25 输出收尾继续映射 plan 11 的 release-artifact-authority 与 phase-verification-harness：

| Maintenance projection | Authoritative implementation / verification |
| --- | --- |
| Phase 14 REL-CLI-08 | `release/sbom.rs`：生成与验包复用 SPDX 结构/必需文件覆盖 gate；Rust unit、compiled CLI 与双 PowerShell 宿主失败输出回归 |
| Phase 14 REL-CLI-09 | `release/checksums.rs`、`integrity.rs`、`atomic_evidence.rs`：同一 manifest 规则、路径别名拒绝、单文件原子替换、manifest 最后写入与失败拒绝验证 |
| Phase 14 REL-CLI-10 | `tests/release_outputs.ps1`：实际 ZIP 许可证 bytes、失败 Syft、checksum 格式、输出接口和环境恢复；替代治理中的脚本函数名/次数断言 |

实际验证与多文件非事务边界见 [输出收尾报告](report/2026-09-25-release-output-finalization.md)。

2026-09-30 内部调用与 CI 规则复用继续投影上述 plan 11 契约：

| Maintenance projection | Authoritative implementation / verification |
| --- | --- |
| Phase 14 REL-CLI-11 / REL-CLI-06 | `release/package.rs` 的同一 typed verifier；`runner/package.rs` 管理输出流，`qualification/remote.rs` 保留原收据边界；同包 direct/wrapper 比较、任务计划回归和隔离 runtime 检查 |
| Phase 14 REL-CLI-12 / REL-CLI-07 | `repository::workspace_version` 与 `release/workflow.rs`；工作流读取既有 Rust 版本规则，qualification 共用 run identity 谓词；compiled CLI 和双 PowerShell 实际 step 的离线行为测试 |
| Phase 14 REL-CLI-13 / REL-CLI-03/08/10 | `release/package_content.rs`、README template、`package_staging.rs`；生成、ZIP allowlist 与 SBOM 覆盖共用清单；同输入 ZIP bytes、许可证/身份/占用路径失败和双宿主实际打包 |
| Phase 14 REL-CLI-14 / REL-CLI-05/08 | `release/syft/` 复用 `integrity` 与 `atomic_evidence`；pins/cache/manifest/私有快照测试；`release_syft.ps1` 双宿主离线传输与失败保留；实际缓存 Syft 输出比较 |

维护不改变阶段脚本参数、产品行为、依赖政策、候选身份或发布权限。性能数字仅覆盖本地静态验包
调用路径，远程执行仍未验证；详见 [复用维护报告](report/2026-09-30-release-cli-reuse.md)。
随后完成的包清单/README 与 Syft 缓存迁移见 [后续报告](report/2026-09-30-package-staging-syft-cli.md)，
该部分只主张一致性与可测试性收益，未测量性能。

2026-09-30 后续收尾继续映射 plan 11 release-artifact-authority / phase-verification-harness：

| Maintenance projection | Authoritative implementation / verification |
| --- | --- |
| Phase 14 REL-CLI-15 / REL-CLI-09 | `release/package_publish.rs` 复用 `integrity`、`checksums` 和 `atomic_evidence`；并发赢家、冲突保留、manifest 文件锁、同输入真实 ZIP bytes |
| Phase 00 P00-A11、Phase 12/13 入口维护、Phase 14 REL-CLI-16 | `phase_entry/` 复用 canonical CLI parsers；00–14、11-b 与 all 共用 `invoke-phase.ps1`，保留各入口参数范围；旧入口同输入比较、双宿主实际 Rust 路由、拒绝与 CWD/编码恢复 |
| Phase 14 REL-CLI-19 | `release/package_workflow`、`sbom_workflow` 直接组合 staging/publish/Syft，runner 保留 task identity；ZIP、网络与外部 Syft 由窄 PowerShell adapter 执行；双宿主与失败保留行为测试 |
| Phase 14 REL-CLI-20 | `integrity/windows` 为 CNG 平台适配，`integrity/portable` 保留 sha256sum；同一 hash authority 用于包、缓存和证据；已知向量、分块/失败/锁定/变更与同输入基线 |
| Phase 14 REL-CLI-17 / REL-CLI-12 | `release/remote_state.rs` 验证 HTTP/GraphQL 观察；`release_remote` 执行实际 workflow step + compiled CLI，离线验证拒绝发生在远程写操作之前，双宿主覆盖中文空格路径别名与实际 CWD/编码恢复；工作流保留查询/写操作及授权边界 |
| Phase 14 REL-CLI-18 / REL-CLI-14 | `release/sbom_preparation.rs` 组合已有版本/包选择/Syft 快照能力；每次仍 `cargo run --locked`；实际缓存命中调用计数和新旧入口交替计时、SPDX 语义比较 |

工具验证不写候选或人工收据。ZIP/manifest 不承诺多文件事务，remote state 观察不构成发布授权。
本次本地 SBOM 入口计时单独记录，不能套用到 ZIP、CI 或产品性能；细节见
[收尾报告](report/2026-09-30-release-cli-finalization.md)。

2026-09-25 合并前工具维护映射 plan 11 phase-verification-harness / modular-headless-ci 与
P00-A10：`cli.rs`、`qualification/{mod.rs,repetition.rs}`、`qualification_environment.rs`、
`runner{,/headless}.rs` 按平台编译实际执行路径并保留纯规则测试；Linux 上的 compiled CLI
回归验证 GUI 资格化仍以非零和 `UNSUPPORTED` / `NOT_TESTED` 拒绝。
CI plan job 在 full/smoke 范围运行 Linux 严格 lint 和 smoke tests，避免该维护缺口复发。
实际运行结果见上述报告的合并前 Resolution；人工与发布状态不变。

2026-09-25 垂直滚动条映射 `09_windows_shell.md#vertical-scrollbars`、`07` Source editor 与
`06` Split scroll sync：用户投影见三种视图中的长文滚动，验收见 Phase 03/05 当日维护案例。
实现为 `stickymd-render/src/source/scrollbar.rs` 的惰性首尾定位与
`stickymd-win/src/app/{scrollbar,scroll_runtime}.rs` 的窄槽绘制、手势和既有语义同步；
Preview worker 回执保留完整视口（尺寸、缩放、主题、滚动位置与选区），过期布局不参与新命中或同步。
Source scrollbar baseline 接入 Phase 03、render 模块和完整 CI 性能分片；P00-A06/A09 约束覆盖并集、
任务去重、runner 内串行测量及 CI 日志/计划归档。定向 Rust tests、Release baseline 与 native message probe
的范围及人工缺口见 [维护报告](report/2026-09-25-vertical-scrollbars.md)。
后续审查、修复和复核见 [滚动条与 CI review](report/2026-09-25-scrollbar-ci-review.md)。

2026-09-28 桌面验收工具修复映射 plan 10 的固定测量 fixture、plan 11 的 partial evidence、
失败留证与 module-success-ledger：P14-A37..A39 由 `runner/resource_progress.rs`、
`runtime.rs`、`qualification/exact_desktop/evidence.rs` 和 module fingerprint 回归持有。
资源 padding 与 startup padding 分离；startup fixture SHA-256 保持不变，资源样本变化会使
Resources 成功记录失效。实际桌面失败与诊断范围见
[本轮报告](report/2026-09-28-v0.1.1-desktop-qualification.md)。

2026-09-29 的 P14-A40/A41 继续投影 plan 10 固定测量样本、plan 08 图片解码与 plan 11
验证合同：窗口压力循环后恢复固定文档并校验每轮 bytes；G5 与 render 集成测试共享图片，
使用实际解码器验证彩色像素，样本变化只命中 G5 指纹。诊断与正式资格化的边界见
[独占桌面验收报告](report/2026-09-29-v0.1.1-exclusive-desktop-qualification.md)。

P14-A33 的并发隔离回归以固定时间戳验证临时路径唯一性；指纹流与测试目录共用
进程内序号，流文件 exclusive create 后才取得清理所有权。输入字节序列与指纹算法不变。

P14-A42..A46 投影 plan 11 的资源子模块资格化：`resource_plan` 统一场景身份与完整覆盖，
`runtime/resources` 保留原生测量协议，`runner/resource_session` 负责同命令去重与计时，
`qualification/resource_modules` 和 module ledger 持有五组 last-success。完整入口完成一组
就登记，后续失败保留已完成组；readiness 要求五组兼容成功。旧 aggregate 不迁移，定向
诊断不能写正式收据。共享窗口/进程输入使所有消费者失效，window/zoom 专属 harness
按组失效。19 名称/15 测量和 25 分钟固定等待差是静态与无界面回归结论；
真实桌面资源与提速仍为 P14-A46 NOT TESTED，见
[优化报告](report/2026-09-29-resource-verification-optimization.md)。

P14-A47/A48 继续投影 plan 11 的候选测量与失败证据规则：`runner/candidate_input`
用完整候选校验替代未使用的本地 EXE 构建；`runtime/resources/cohort` 保留完整单次样本，
在既有 any/max 硬门确定失败时终止后续轮次；`runner/resource_session` 把失败观测送入
统一 JSON 收尾路径，五类 last-success 不受影响。成功次数、预热/CPU 窗口和 startup
p95 算法不变；首轮 CPU 失败减少 360 秒固定等待是静态预算，桌面实测仍为 P14-A46。

P14-A49..A51 投影 plan 11 的 `shared-headless-prerequisite`：
`qualification/workspace_tests` 持有完整 Runtime/Performance workspace tests 的 source-bound
成功与执行身份，`module_ledger/fingerprint` 和 `resource_modules` 共享单轮规划结果，
`runner/timing` 保留任务失败用时。执行与复用前后重新核对输入；未知设置绕过共享，
失败/partial/CI 不登记；历史运行时间明确标为 origin。真实 Git/freeze fixture、受控执行器、
指纹与失败观测回归验证工具规则；资源产品依赖仍覆盖 ALL_PRODUCT，原生验收保持 P14-A46。

上述优化的独立 review 修复继续映射 P14-A42/A43/A49：`resource_plan/observations`
核对实际样本、统计、来源和既有硬门；共享 cohort 缓存保留完整观测。`module_ledger`
按文件系统身份统一输出别名，`smoke_scope` 在任务前保护内部路径；`workspace_tests/identity`
按 Cargo 子进程工作目录解析相对配置。隔离旧行为复现与修复回归见优化报告追加 Resolution。

P14-A50/A51 的批量规划回归由 `module_ledger/fingerprint/stream` 持有：共享输入单次流式
读取，逐组保持旧摘要协议；有界缓冲、真实读取/flush 失败、临时文件冲突及 unwind 清理
验证本次临时文件所有权。显式 `resource_planning_profile` 比较旧实现与批量实现的指纹、
源码读取量和规划用时，默认测试跳过该本机计时；其结果不等于 P14-A46 的原生测量。

本轮独立 review 补漏仍映射 P14-A42/A43/A50：`resource_modules/tests` 用真实 Git/freeze、
合成且不执行的 PE 和完整资源收据，验证规划后 ignored 账本/归档删除、损坏及来源更新；
`module_ledger`、`exact_desktop/evidence` 与 CLI 回归验证整个成功存储目录、目录别名和
Windows 未创建目标的尾随点/空格保护。验证结果见
[`2026-09-29-optimization-independent-review.md`](report/2026-09-29-optimization-independent-review.md)。

P14-A52 映射 plan 11 的资源交互预检：`runtime/resources/probe` 使用独立 fixture，先验证
真实 Source/Preview/Split 切换再开始资源等待；`window_control/routing` 在路由错误发生时
查询 HWND/PID、可执行文件名和窗口类，不记录窗口标题或完整路径。设计与验证记录见
[资源诊断设计](report/2026-09-30-resource-diagnostics-design.md)。

P14-A53 的场景完成回调由 `resource_plan/progress` 定义，执行层只上报阶段和观测；
`runner/resource_observer` 持有进度 sidecar 与 INCOMPLETE 检查点的原子写入。
`runner/resource_progress` 继续持有组间检查点，正式子组成功仍只由既有 Campaign 登记。

P14-A60 映射 plan 11 的诊断失败优先：`resource_diagnostics/priority` 持有有界、校验和与
有效期约束的提示；`runner/resource_priority` 保持前置任务原位，`resource_resume` 强制
选中单元新测量、记录实际失败并在完整新成功后清除仍匹配的旧提示。只读计划和正式拒绝
由 CLI/PowerShell 集成回归覆盖，提示始终不持有任何成功证据权威。

P14-A59 映射 plan 11 的缓存批次：`resource_diagnostics/batch` 保留前后鲜活检查、只返回
完整批次；`runtime/resources/batch` 限定单组尚未共享的单元，不跨新测量保留批次。

P14-A58 映射 plan 11 的跨命令等价场景复用：`resource_plan/diagnostic` 定义注册等价和标签
投影，`resource_diagnostics/lookup` 独立验证原记录，`record` 保留原场景/身份/创建时间。

P14-A57 映射 plan 11 的身份开销与鲜活性边界：`resource_diagnostics/identity` 分项计时，
Store 保留读取前后/保存前后的鲜活检查；同命令共享经 observer 单独检查身份。

P14-A56 映射 plan 11 的诊断计划：`resource_diagnostics/plan` 计算逐单元预算与同命令共享，
`lookup` 解释记录/身份失效，最新指针无复用权威；CLI 只看计划输出 NOT_RUN，保留证据。

P14-A55 映射 plan 11 的完整 Window/Zoom 诊断复用：`resource_plan/diagnostic` 共享
正式组结构校验，`runtime/resources/whole_group` 在完整测量及清理后保存；历史原始观测
仍拒绝正式登记。完整五组现场资格化继续由 P14-A46 持有。

P14-A54 映射 plan 11 的显式诊断续跑：`runner/resource_resume` 在单次命令内持有
`qualification/resource_diagnostics`，严格核对完整场景和前后身份；公共路径保护隔离
ignored 缓存与 canonical 账本。原始观测校验复用 `resource_plan/observations`，历史
`shared_from` 标记在同命令别名共享时保留，不得投射成正式资格。
