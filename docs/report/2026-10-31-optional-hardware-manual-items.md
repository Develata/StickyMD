# 2026-10-31 — 需要额外硬件的人工项改为可选

- 状态：USER 已批准（2026-10-31），已写入 [plan 11](../plan/11_testing_and_release.md) 与
  [phase-12 矩阵](../acceptance-cases/phase-12.md)。本文只记录决定的原因与影响，不替代工程合同。
- 基线：`main` `1c6d7214e44b9120d503948c32145ff8d7289902`。

## 决定

需要另一台机器、虚拟机或第二块显示器才能执行的人工观察改为可选人工项：`NOT TESTED` 不阻断发布，
不需要 waiver；已观察到的 `MANUAL_FAIL` 与无效证据仍阻断；readiness 汇总逐项逐角色列出未测的可选项，
并写明 READY 不表示这些环境已经测过。

| 范围 | 之前 | 之后 |
| --- | --- | --- |
| `previous` 角色下的全部项 | 按各项 tier；缺失时可用 `WAIVER-PROFILE-PREVIOUS-v{version}` | 可选；档案 key 退役 |
| M34 Clean VM、M35 同 DPI 双屏、M36 混合 DPI 双屏、M37 运行中拔屏 | Tier B（组 waiver） | Tier O（可选） |
| M42 RDP、M43 物理负坐标布局 | Tier C | Tier O（可选） |
| M11/M12 左/右 dock | Tier A，含"混合 DPI 下感应条仍可用" | Tier A，单屏；混合 DPI 下左/右感应条移入 M36 的 `left-dock-sensor`、`right-dock-sensor` 场景 |
| M38–M40、M47 单屏 DPI，M41 睡眠唤醒 | Tier B / Tier C | 不变（本机可完成） |

## 原因

USER 只有一台机器、一块显示器；这些条件每次发版都难以满足，此前 v0.1.2、v0.1.3 都以整版 USER 例外发布。
改为可选后，单机单屏可以按合同到达 READY，不再需要逐版签署环境 waiver。

## 代价

- 多屏、混合 DPI 与 Windows 版本差异的回归只有在 USER 实际测试时才会被发现；风险最大的是跨屏与混合 DPI 下的
  dock 与感应条，其次是托盘与输入法在不同功能版本上的差异。
- READY 不再表示测过前一个 Windows 版本或干净机器。干净机器的主要风险（缺运行库）已由 P12-A15 自动检查覆盖；
  M34 剩下的是信誉提示与 README 描述是否一致。
- 目标环境收据仍声明 `current`、`previous` 两个角色（schema 不变），以便测试 `previous` 时角色匹配照常工作。

## 实现与验证

- `registry::Tier::Optional`（显示为 `O`）；readiness 对 Tier O 与 `previous` 角色的未测项记为
  `NOT TESTED`（`optional; ...`）且不加 blocker，在其他规则（waiver、Tier C）之前判定；FAIL 与无效证据的处理不变。
- `waiver::validate` 不再接受 `WAIVER-PROFILE-PREVIOUS-v{version}`；Tier B 组 key 与逐项 key 不变。
- 覆盖表中 P08-M11 `mixed-dpi-dock` 的 left/right 义务改由 M36 的新场景持有；`PHASE12_SHA256` 在核对全部人工行后更新。
- 本机：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、
  `cargo test -p stickymd-smoke --locked`（596 + 26 passed）、`all --ci --ci-shard=tests`（5 项 PASSED）、`phase-entry 00` 均通过。
- USER 明确表示本项不另走跨模型审查（2026-10-31）。
