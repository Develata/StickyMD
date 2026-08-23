# Phase 13 Final Qualification — Source Template

本文件只冻结报告结构与证据规则，不在候选运行后回填数字。

- Runtime results are stored as hash-bound untracked evidence.
- Performance results are stored as hash-bound untracked evidence.
- Resource results and partial progress are stored as hash-bound untracked evidence.
- Manual, remote, downloaded-artifact and readiness receipts remain under `dist/evidence/`.
- 最终动态汇总由 Phase 13 聊天报告给出，并引用 exact source / EXE / ZIP / SBOM hashes。
- 任何 tracked source 变化都会产生新 candidate；旧 receipt 随即失效。

最终状态只能是：

```text
LOCAL QUALIFICATION BLOCKED
LOCAL QUALIFICATION COMPLETE — USER DECISIONS REQUIRED
READY FOR PUSH AUTHORIZATION
REMOTE QUALIFIED — TAG APPROVAL REQUIRED
```

不得在本模板中声明 RC ready、tagged 或 published。
