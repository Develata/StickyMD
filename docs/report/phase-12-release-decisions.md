# Phase 12 Release Decision Ledger

本账本只记录 USER authority。自动化不得修改、推断或替代这些状态。合法状态仅为
`PENDING`、`USER APPROVED`、`USER REJECTED`、`NOT APPLICABLE`。

| ID | Decision | Status | Evidence / scope |
| --- | --- | --- | --- |
| DEC-01 | WARM-STARTUP-GATE | USER APPROVED | 2026-08-23 USER 批准 v0.1.0 warm startup hard boundary `p95 <= 400 ms`；`<=180 ms` 保持 preferred target；这是 engineering gate recalibration，不是 acceptance waiver |
| DEC-02 | RELEASE-VERSION | PENDING | Workspace 当前为 `0.1.0`，但 USER 尚未批准把它作为发布版本 |
| DEC-03 | MANUAL-WAIVERS | PENDING | 没有任何 blanket waiver；必须逐项/明确分组批准 |
| DEC-04 | UNSIGNED-POLICY | PENDING | 当前 package 明示 unsigned；尚未收到 v0.1.0 unsigned release 批准 |
| DEC-05 | PUSH | PENDING | 未授权 push exact candidate |
| DEC-06 | TAG | PENDING | 未授权创建或推送 tag |
| DEC-07 | DRAFT-RELEASE | PENDING | 未授权生成 remote draft release |
| DEC-08 | PUBLISH | PENDING | 未授权 publish |

## Interpretation

- `USER APPROVED` gate calibration 改变 v0.1.0 release hard boundary；它不把未执行的人工
  验收变成 PASS。
- `MANUAL-WAIVERS` 只有在 USER 明确列出具体 case/group 后才能批准；不得用一个泛化状态
  吞掉 Tier A/B/C 明细。
- action authorization 只授权对应动作，不自动批准后续动作。
