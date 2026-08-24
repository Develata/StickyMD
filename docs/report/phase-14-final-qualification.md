# Phase 14 Final Qualification — Source Template

本文件冻结 Phase 14 最终报告的结构和解释规则，不把 candidate 运行后的动态数字回填进 tracked
source。exact source / EXE / ZIP / SBOM hash、Runtime、Performance、Resources、startup attribution、
manual 和 readiness 收据均保存在 ignored `dist/evidence/`；最终动态汇总由 Phase 14 聊天报告给出。

## Required Evidence Channels

1. qualification environment；
2. Release/package；
3. headless CI；
4. Runtime；
5. Performance；
6. startup attribution；
7. Resources；
8. USER-driven Manual；
9. Readiness；
10. remote/downloaded artifact（仅在 USER 授权 push 后）。

普通 channel failure 不删除或跳过独立、安全的后续证据；environment/identity/P0/security/data
safety/schema corruption 仍是 global stop。任何 tracked source 变化都会产生新 candidate，并使旧
dynamic receipt 失效。

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
sleep 不是 ready 证据。任何 tracked harness correction 都使旧 candidate 失效，并触发完整 exact
candidate requalification。
