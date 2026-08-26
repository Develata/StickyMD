# Phase 04 Acceptance Matrix

> Verification projection for Portable Persistence, Autosave, Recovery and External Reconciliation.
> Rows map only the Phase 4 slice; they do not upgrade a broader v1 AC whose later UI/assets remain
> unimplemented.

| ID | Plan / AC mapping | Mode | Checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P04-A01 | AC-001 portable note/config bootstrap subset | Automated | startup tests + [`phase-04.ps1 -Runtime`](../../tools/smoke/phase-04.ps1) | AUTOMATED PASS |
| P04-A02 | AC-005 650 ms autosave and bounded coalescing | Automated | deterministic scheduler/worker tests | AUTOMATED PASS |
| P04-A03 | AC-006 manual-save intent and durable acknowledgement | Automated | coordinator/storage tests | AUTOMATED PASS |
| P04-A04 | AC-007 clean external reconciliation subset | Automated | watcher/reconciliation tests | AUTOMATED PASS |
| P04-A05 | AC-008 dirty conflict, Load External and Keep Local subset | Automated | OCC/coordinator/storage tests, including an old guarded completion superseding a queued Keep Local request and force-receipt-only resolution | AUTOMATED PASS |
| P04-A06 | AC-026/AC-027 same/different directory instance behavior | Automated | Windows adapter tests + [`phase-04.ps1 -Runtime`](../../tools/smoke/phase-04.ps1) | AUTOMATED PASS |
| P04-A07 | AC-030 recovery classification/choice subset | Automated | bootstrap/recovery/failure-injection tests plus occupied recovery/quarantine-name no-overwrite regressions | AUTOMATED PASS |
| P04-A08 | 20 KiB/100 KiB/1 MiB persistence stages | Automated | [`phase-04.ps1 -Performance`](../../tools/smoke/phase-04.ps1) | AUTOMATED PASS |
| P04-M01 | real Notepad clean reload and dirty conflict UI | Manual | Current-commit Notepad matrix required | NOT TESTED |
| P04-M02 | read-only note.md and permission error UI | Manual | Current-commit ACL/UI receipt required | NOT TESTED |
| P04-M03 | deterministic kill between temp flush and publish | Manual | Controlled kill-window receipt required | NOT TESTED |
| P04-M04 | greater-than-260-character portable Program Directory | Manual | Long-path-enabled Windows receipt required | NOT TESTED |
| P04-M05 | live ReplaceFileW 1175/1176/1177 filesystem states | Manual | Live Windows fault receipt required | NOT TESTED |
| P04-M06 | inherited Microsoft Pinyin/WeChat/candidate-position gate | Manual | [`phase-03 manual IME checklist`](../report/phase-03-manual-ime-checklist.md) | NOT TESTED |
