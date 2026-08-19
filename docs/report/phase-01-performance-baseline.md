# phase-01-performance-baseline.md - Phase 1 性能基线

- `Date`: 2026-08-19
- `Type`: 阶段基线（performance baseline）
- `Status`: Completed（数据来自 1B / 1D spike release 构建实测；环境：Windows 11 x64，20 逻辑核，Rust 1.97.1 MSVC）

## 背景

plan_ref: docs/plan/10_performance_reliability.md。本报告汇总 Phase 1 可量化的性能数据，
作为后续阶段的预算参照。**未测项如实标注**，不以推断代替测量。

## 1. 空闲行为（1B window）

| 指标 | 值 | 判定 |
| --- | --- | --- |
| 空闲 CPU（占单核，中位数） | **0%** | PASS（无持续重绘循环） |
| 空闲 CPU（最大，含启动余波） | 3.9% | 首帧/启动一次性开销 |
| Working Set（稳定） | 18.31 MB | 恒定，无泄漏迹象 |
| Private Memory（稳定） | 5.96 MB | 恒定 |
| 8.06 s 内重绘次数 | 4（1 初始 + 3 启动期 resize） | 之后约 8 s 零重绘 |

满足宪法 §5（idle behavior）：`ControlFlow::Wait` + dirty-only 重绘。

## 2. Markdown 解析（1D，release，24 次取中位 / p95 / max）

| 文档规模 | median | p95 | max | 节点数 | 峰值分配 |
| --- | --- | --- | --- | --- | --- |
| 20 KiB | 2.45 ms | 2.54 ms | 2.62 ms | 3321 | ≈1988 KiB |
| 100 KiB | 11.45 ms | 12.30 ms | 12.70 ms | 16481 | ≈13037 KiB |
| 1024 KiB | 126.47 ms | 189.97 ms | 190.25 ms | 167961 | ≈114265 KiB |

- fixture（865 B）解析 + arena→owned 投影：约 0.22 ms / 64 节点。
- 解析耗时与节点数近似线性。峰值分配与节点数同阶（arena + owned 投影双份，投影后
  arena 释放）。

## 3. 数学渲染（1D，RaTeX parse→layout→display→PNG）

| 指标 | 值 |
| --- | --- |
| 单式渲染 median | 0.75 ms |
| p95 / max | 1.23 ms / 1.25 ms |
| 5 个 fixture 公式 | 全部成功（PNG 4.3–11.2 KB） |

## 4. 持久化（1E，功能性为主，未做吞吐基准）

- 原子保存（tmp→FlushFileBuffers→ReplaceFileW/MoveFileExW）为功能性验证，未测吞吐；
  对单文件便签（note.md）的规模而言远低于帧预算，暂不构成瓶颈。
- 目录身份哈希（SHA-256）为一次性启动开销，可忽略。

## 5. 性能结论与预算建议

1. **空闲零开销成立**：呈现链路满足 idle 合同（CPU≈0、无持续重绘）。
2. **≤100 KiB 全量重解析可接受**（~11 ms），可在编辑停顿/视口变化时整段重算。
3. **>~100 KiB（尤其 1024 KiB ~126 ms）超出单帧预算**：生产必须采用增量解析 /
   视口裁剪 / 后台线程（plan 10），不得每次按键全量重解析巨型文档。**这是 Phase 2+
   的设计约束，非阻塞。**
4. **数学按需渲染 + 位图缓存**：单式 <1.3 ms，配合缓存策略可满足滚动流畅度。
5. 峰值内存：1024 KiB 文档解析峰值约 112 MiB（含 arena+投影双份）；生产应以增量/
   流式策略压低常驻，避免一次性全量投影巨型文档。

## 未测项（如实记录）

- 真实 DPI / 多显示器缩放下的呈现性能（本机 scale=1.0）。
- 大文档滚动帧率（依赖 Phase 2+ 视口布局实现，Phase 1 未构建滚动渲染）。
- 持久化写吞吐 / 断电持久性（功能性验证，非性能基准）。
