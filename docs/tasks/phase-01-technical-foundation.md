# phase-01-technical-foundation.md - Phase 1 任务记录（技术地基验证）

## Goal

在写入任何生产运行时代码之前，用**可删除的独立实验 crate** 验证冻结技术栈的关键
技术路径：呈现链路、文本/IME、Markdown/数学、可移植持久化，并产出依赖/Windows API/
性能基线与决策报告，为 Phase 2（核心文档模型）建立前置门。

## Scope

技术 spike + 基线测量 + 决策报告。生产 workspace 仅建立最小骨架（Phase 1A），
**不实现任何生产运行时功能**。所有 spike 位于 `experiments/phase-01/*`，各自声明空
`[workspace]`，可随时删除而不影响生产构建。

## Inputs

- USER Phase 1 任务 prompt（冻结技术栈与验证目标）。
- `docs/plan/` 已批准契约（尤其 03 / 06 / 07 / 09 / 10）。
- 仓库现状：Phase 0 已完成治理骨架，工作树 clean。

## Deliverables

- Phase 1A：生产 workspace 骨架（`crates/stickymd-core` / `crates/stickymd-render` /
  `apps/stickymd-win`，最小依赖）+ `rust-toolchain.toml`（锁定 1.97.1）。
- Phase 1B：`experiments/phase-01/window`（winit+softbuffer+tiny-skia + Win32 薄适配）
  + `RESULTS.md`。
- Phase 1C：`experiments/phase-01/text`（cosmic-text 投影模型 + IME 事件 + 剪贴板 + undo）
  + `RESULTS.md`。
- Phase 1D：`experiments/phase-01/markdown`（Comrak 投影 + RaTeX 渲染）
  + `RESULTS.md` + `COMRAK_NOTES.md`。
- Phase 1E：`experiments/phase-01/persistence`（身份/单实例/原子保存/恢复/冲突）
  + `RESULTS.md`（18 测试通过）。
- 基线与决策报告（`docs/report/`）：
  - `phase-01-dependency-baseline.md`
  - `phase-01-windows-api-baseline.md`
  - `phase-01-performance-baseline.md`
  - `phase-01-technical-spike-report.md`（Executive Decision + Recommendation A/B/C）

## Verification

- 每个 spike：`cargo build --release` 通过；`cargo run` 输出与 `RESULTS.md` 一致。
- 1E：`cargo test --release` 18 通过；`cargo clippy --all-targets` 0 warning。
- 禁用项审计：`cargo tree --target x86_64-pc-windows-msvc` 无 WebView/JS/浏览器运行时
  crate（`wasm-bindgen` 仅为 wasm32 平台条件项，见依赖基线 §1.1/§2）。
- 可删除性：`cargo build --workspace`（生产）不含任何实验依赖，独立成功。
- 交互式 IME、真实 DPI/多显示器、ACL/junction、断电持久性等人工/环境项**如实标注
  NOT TESTED**，不冒充已验证。

## Out of Scope

- 任何生产运行时代码、生产 crate 的实质实现。
- Phase 2 的 DocumentState / TextDelta / Undo 实现（属下一阶段）。
- 真实发布、CI/CD、安装包。

## Completion State

`Completed`（2026-08-19）

说明：五条技术路径验证完成，**无 FAIL**；呈现链路与 RaTeX 数学为 PASS，文本/IME、
Markdown、持久化为 CONDITIONAL（条件项已在 spike 报告 §2 明确，均非阻塞）。
**生产运行时能力仍未实现**；Phase 2 前置门待 USER 对 Recommendation A 批准后开启。
