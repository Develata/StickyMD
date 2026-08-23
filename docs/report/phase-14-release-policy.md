# Phase 14 Release Policy

## Executive Decision

Phase 14 是 release-policy / verification-plane phase。产品功能和 runtime 依赖保持冻结。

| Decision | Status | Binding scope |
| --- | --- | --- |
| Release version `0.1.0`; future tag `v0.1.0` | USER APPROVED | 不授权创建或推送 tag |
| Unsigned Authenticode portable ZIP | USER APPROVED | v0.1.0；必须公开信誉提示与验证方法 |
| Cold/warm p95 `≤550 ms` release boundary | USER APPROVED | v0.1.0 exact candidate |
| `≤180 ms` preferred / `≤400 ms` engineering target | ACTIVE DIAGNOSTIC | 未达到不阻断 v0.1.0，但必须报告 |
| Tier A/B/C manual policy | USER APPROVED | version + source + case/group binding |
| Independent evidence channels | USER APPROVED | 普通失败不抹除独立证据 |
| Push/tag/draft/publish | PENDING | 不得执行 |

## Startup Threshold Semantics

三层门必须同时展示：preferred target `180 ms`、engineering target `400 ms`、v0.1.0 hard
release boundary `550 ms`。只有 cold 或 warm p95 超过 550 ms 才产生 startup release failure。
400–550 ms 是需要归因和后续改善的性能债，不是 Phase 14 产品改动授权。

## Manual Risk Policy

- Tier A：真实 IME、ToolWindow/taskbar/Alt+Tab、tray/basic docking、基本 Preview/math/image、native
  export、hard-kill recovery 等 release-critical human facts。PASS 或 explicit exact-bound waiver。
- Tier B：Clean VM、真实双屏/mixed DPI/断屏与真实 125/150/200% DPI。PASS 或 USER 对明确
  case/group 的 version/source-bound waiver。
- Tier C：sleep/resume、RDP、negative coordinates、真实 junction/symlink extended cases。
  对应 automated contract PASS 时允许 `NOT TESTED`；已观察的 FAIL 仍阻断。

任何 waiver 都不跨版本继承。`v0.1.0` waiver 不适用于 `v0.1.1`。

## Independent Evidence Policy

Environment invalid、candidate identity mismatch、P0/security/data-safety failure、receipt corruption
是全局停止条件。普通 Runtime、Performance 或 Resources failure 只失败当前通道；后续独立且
安全的通道继续执行并保留各自 receipt。Readiness 最终聚合全部事实，不以顺序短路掩盖问题。

## Signing Policy

v0.1.0 package 明确为 unsigned Authenticode。自动化不得生成自签证书、伪造 signed 字段或因
unsigned 本身失败。README/release notes 必须建议核对 `SHA256SUMS.txt` 和 GitHub attestation，
不得建议关闭 Defender 或 SmartScreen。

## Remaining Authority

PUSH、TAG、DRAFT-RELEASE、PUBLISH 均保持 `PENDING`。Phase 14 不执行任何 remote mutation。
