# Phase 14 Startup Attribution Plan

## Goal

解释 cold/warm startup 的主要成本来源，不为了追逐 180/400 ms target 修改 frozen product。

## Evidence Sources

1. copied standalone Release EXE 的 30 cold + 50 warm `EDITOR_READY` samples；
2. product 已有 startup milestones，按每个 sample 计算阶段 interval 后再报告 p50/p95；
3. Windows Performance Recorder/Analyzer 仅在环境能够安全、可复现地产生并解释 trace 时作为
   辅助证据。只有 `wpr.exe` 而缺少可用 WPA 分析链时，必须写 `ETW attribution NOT AVAILABLE`，
   不得用未分析 ETL 冒充归因。

## Categories

- process overhead：外部进程创建到 product `EDITOR_READY` 之外的剩余时长；
- bootstrap：`main_enter → event_loop_ready`；
- window/surface：`event_loop_ready → font_system_begin`；
- font discovery：`font_system_begin → font_system_end`；
- source layout：`font_system_end → source_projection_ready`；
- shell setup：`source_projection_ready → window_visible`；
- focus/guards：`window_visible → editor_ready`。

每个 category 使用 per-sample interval；禁止用不同 milestone 的独立 p95 直接相减伪造联合分布。

## Decision Rule

结论必须严格为以下之一：

- `NO PRODUCT OPTIMIZATION NEEDED`
- `ONE SIMPLE LOCAL OPTIMIZATION JUSTIFIED`
- `STARTUP CORRECTNESS ISSUE FOUND`

默认结论是第一项。只有发现 correctness defect，或一个局部、低复杂度、无架构扩张且可证明
去除至少约 50 ms 重复工作时，才允许提出 product change；本 Phase 不自动实施。

## Output

ignored `dist/evidence/startup-attribution.json` 绑定 exact source / EXE，并包含 method、ETW
availability、dominant cold/warm category、三层 threshold classification 与上述唯一 decision。
