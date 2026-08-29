# Phase 14 Final Qualification — Source Template

本文件冻结 Phase 14 最终报告的结构和解释规则，不把 candidate 运行后的动态数字回填进 tracked
source。Source Freeze / workflow run/artifact id / EXE / ZIP / SBOM hash、Runtime、Performance、Resources、startup attribution、
manual 和 readiness 收据均保存在 ignored `dist/evidence/`；最终动态汇总由 Phase 14 聊天报告给出。

## Required Evidence Channels

1. Source Freeze 与 Local Preflight；
2. source-only headless CI；
3. remote release workflow run/attempt/artifact identity（仅在 USER 授权 push 后）；
4. downloaded artifact verification 与 Promoted Candidate；
5. qualification environment；
6. Runtime；
7. Performance 与 startup attribution；
8. Resources；
9. G3/G4/G5 exact desktop；
10. USER-driven Manual；
11. Readiness。

普通 channel failure 不删除或跳过独立、安全的后续证据；environment/identity/P0/security/data
safety/schema corruption 仍是 global stop。任何 tracked source 变化都会使 Source Freeze 及所有
dynamic receipt 失效；仅 Promoted Candidate 字节变化时，source-only evidence 可复用，只重跑
artifact-bound 的最小 exact 验证。

## Startup Interpretation

- `p95 <= 180 ms`：preferred target；
- `180 < p95 <= 400 ms`：未达到 preferred，但达到 engineering target；
- `400 < p95 <= 550 ms`：性能债，仍满足 v0.1.0 release boundary；
- `p95 > 550 ms`：v0.1.0 startup release failure。

## Manual Interpretation

- Tier A/B `NOT TESTED` 在没有 exact-bound USER waiver 时阻断；
- Tier C `NOT TESTED` 仅在相应 automated contract PASS 时 nonblocking；
- 任意已观察 `MANUAL_FAIL` 阻断；
- G1..G3 只组织操作，P12-M01..M44 仍是逐项 authority。

最终状态只能是：

```text
LOCAL QUALIFICATION BLOCKED
LOCAL QUALIFICATION COMPLETE — USER MANUAL DISPOSITION REQUIRED
READY FOR PUSH AUTHORIZATION
REMOTE QUALIFIED — TAG APPROVAL REQUIRED
```

不得在本模板中声明 RC ready、tagged 或 published。

## Resources Triage Rule

hidden-window resource failure 必须先拆分 collapse/expand、tray hide/show、controls、
collapse+tray 与原组合路径，并以独立运行和降阶计数区分 product reducer、platform projection、
qualification harness 与 environment。等待 editor 可写时必须观察真实 shell/source projection；固定
sleep 不是 ready 证据。任何 tracked harness correction 都使 Source Freeze 与旧 candidate 失效；
只有 Source Freeze 不变、单纯 Promote 不同 candidate bytes 时才允许最小 artifact-bound exact 重验。
