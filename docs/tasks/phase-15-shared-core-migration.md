# Phase 15 共享实现迁移

## Completion State

In Progress。A 的代码迁移与本机验证已完成；B 的笔记、预览、资产/导出/配置链已迁入 `stickymd-app`，隐藏已按请求、回执、提交执行；跨模型审查待做；C、D 未开始。

## Goal

把可共享的应用逻辑与界面逻辑集中到 `stickymd-app`、`stickymd-desktop`，Windows 只保留宿主与原生 adapter，为后续
Debian、macOS 宿主留出真实可用的共享层。Windows 产品行为与数据格式不变。

## Inputs

- [批准的提案](../report/2026-10-23-shared-core-thin-platform-shell-architecture.md)（USER 批准 2026-10-31）。
- [`03` 共享实现节](../plan/03_system_architecture.md#shared-crates)、
  [`11` Phase 15 节](../plan/11_testing_and_release.md#phase-15-shared-core-migration)、
  [`10` 迁移性能判定](../plan/10_performance_reliability.md#phase-15-migration-gates)。

## Scope

| 里程碑 | 状态 | 内容 |
| --- | --- | --- |
| A | 本机验证完成 | `stickymd-app`；`instruction/`、`flow/`（含 `flow/window/`）、配置模型、`interaction/`、`source_search.rs` |
| B | 本机验证完成：笔记、预览、资产/导出/配置链与隐藏回执；显示、置顶、定位的回执未做（见 Open Items） | 执行接口与应用运行时；窗口动作的请求、回执与提交；入口测试见 plan 11 |
| C | 未开始 | `stickymd-desktop`；通用控件、绘制、命中、事件翻译；各 target 的 backend features |
| D | 未开始 | 迁移剩余业务分支，删除仅为迁移保留的转发，最终审查 |

A 中 `config/storage.rs` 留在 `apps/stickymd-win`：它依赖 Windows 原子发布与文件系统，存储能力在 B 进入 Capability Port。
`apps/stickymd-win/src/config/mod.rs` 暂时无状态转发 `stickymd_app::config`，D 删除。B 后写入经 `ConfigStorage` 端口，读取与
迁移告警仍在启动路径。

## Deliverables

- `crates/stickymd-app`（`forbid(unsafe_code)`，不依赖 winit、softbuffer 或平台 API）。
- CLI 模块 `app`、Linux portable 选择、资格化指纹归属、`phase 15` 入口、[验收矩阵](../acceptance-cases/phase-15.md)。
- 治理：共享应用的平台类型、平台条件与平台依赖禁令；未归入产品根的源码拒绝；按名字过滤的测试任务静态核对。
- 迁移前基线：配置字节 golden、无操作启动与保存、热路径计数（基线提交 `b846e83`）。

## Verification

见 [phase-15 验收矩阵](../acceptance-cases/phase-15.md)。每批出口按 plan 11：治理随代码、测试选择与指纹、数据字节兼容与
回退、性能。

### 性能对比

基线提交 `b846e83` 与迁移后在同一台机器上、无其他编译任务时运行（Release，p95，单位 ms；数值受机器波动影响，只用于确认
没有退步）。硬门的计数项见 [P15-A07](../acceptance-cases/phase-15.md)，计数与基线相同。

| 基线项 | 基线 | A 后 | B 后 |
| --- | --- | --- | --- |
| 1 MiB 查找 | 14.94 | 12.16 | 15.6–21.0 |
| 窗口 reducer 100k | 4.87 | 3.81 | 3.77 |
| Source 1 MiB 末尾输入 | 1.73 | 1.73 | 1.31 |
| Source 1 MiB full resync | 87.0 | 58.8 | 90.4 |

B 后的查找偏高不是回退：`source_search.rs` 自 A 起未改；同一时段与迁移前提交 `03ba733` 交替运行 4 轮，迁移前 p95 为
18.4–27.0，迁移后为 15.6–21.0。硬门计数未变（`phase 15 --performance` PASS）。

## Open Items

- 显示、置顶、定位的请求与回执（[plan 04](../plan/04_runtime_state_model.md#window-action-receipts)）未做：这三类动作仍是单向
  效果，winit 的显示与定位没有失败回执，定位又按动画帧发出，需要先决定回执粒度。
- `AppSession` 未建立：`StickyApp` 仍持有各 coordinator，`NoteIo`、`AssetIo`、`PreviewIo` 按步借用；属于 D。
- 进入 C 之前要先有逐帧拷贝计数（[plan 10](../plan/10_performance_reliability.md#phase-15-migration-gates)）。
- 同一副本回退重开（P15-B13）、真实桌面回归（P15-A10）与远程 CI 未执行；`hide_native` 的真实窗口回执只有桌面运行能证明。

## Out of Scope

- 非 Windows 宿主与其产品决定（`02` 平台节）。
- 任何产品行为变化；A 只移动代码、调整可见性与导入路径。
- 把 Phase 15 加入 Phase 14 的正式资格化门；它不是资格化 suite。
