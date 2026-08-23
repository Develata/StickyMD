# Phase 12 Release Decision Ledger

本账本只记录 USER authority。自动化不得修改、推断或替代这些状态。合法状态仅为
`PENDING`、`USER APPROVED`、`USER REJECTED`、`NOT APPLICABLE`。

| ID | Decision | Status | Evidence / scope |
| --- | --- | --- | --- |
| DEC-01 | STARTUP-RELEASE-BOUNDARY | USER APPROVED | 2026-08-23 USER 批准 v0.1.0 cold/warm hard boundary `p95 <= 550 ms`；`<=180 ms` 为 preferred、`<=400 ms` 为 diagnostic engineering target |
| DEC-02 | RELEASE-VERSION | USER APPROVED | 2026-08-23 USER 批准 workspace version `0.1.0`，未来 tag 名为 `v0.1.0`；本决定不授权创建或推送 tag |
| DEC-03 | MANUAL-RISK-POLICY | USER APPROVED | 2026-08-23 USER 批准 Tier A/B/C 风险分层、exact-bound waiver 与 G1..G3 guided sessions |
| DEC-04 | UNSIGNED-POLICY | USER APPROVED | 2026-08-23 USER 批准 v0.1.0 以 unsigned Authenticode portable ZIP 分发；必须明确信誉提示和校验方法 |
| DEC-05 | INDEPENDENT-EVIDENCE-COLLECTION | USER APPROVED | 2026-08-23 USER 批准普通 channel failure 后继续收集独立、安全的后续证据 |
| DEC-06 | PUSH | PENDING | 未授权 push exact candidate |
| DEC-07 | TAG | PENDING | 未授权创建或推送 tag |
| DEC-08 | DRAFT-RELEASE | PENDING | 未授权生成 remote draft release |
| DEC-09 | PUBLISH | PENDING | 未授权 publish |

## Interpretation

- `USER APPROVED` startup calibration 改变 v0.1.0 release hard boundary；它不把未执行的人工
  验收变成 PASS，也不表示达到 180/400 ms target。
- 人工 waiver 只有在 USER 明确列出具体 case/group、版本与 exact source 后才成立；不得用一个
  泛化状态吞掉 Tier A/B/C 明细，也不得跨版本继承。
- action authorization 只授权对应动作，不自动批准后续动作。
