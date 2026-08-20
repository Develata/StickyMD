# Phase 01 Acceptance Matrix

> Verification projection for Technical Foundation & Risk Spikes. It does not approve production
> Preview, persistence, or real IME behavior.

| ID | Plan / AC mapping | Mode | Checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P01-A01 | workspace boundaries and platform-independent core/render | Automated | [`phase-01.ps1`](../../tools/smoke/phase-01.ps1): workspace check | AUTOMATED PASS |
| P01-A02 | AC-013..AC-016 semantic/math spike subset | Automated | `markdown-math` tests through [`phase-01.ps1`](../../tools/smoke/phase-01.ps1) | AUTOMATED PASS |
| P01-A03 | AC-026/AC-030 persistence primitive spike subset | Automated | `persistence` tests through [`phase-01.ps1`](../../tools/smoke/phase-01.ps1) | AUTOMATED PASS |
| P01-A04 | forbidden architecture and dependency boundary | Automated | Rust governance validator + workspace metadata check | AUTOMATED PASS |
| P01-A05 | Phase 1 Release measurements | Automated | [`phase-01.ps1 -Performance`](../../tools/smoke/phase-01.ps1) | AUTOMATED PASS |
| P01-M01 | native window at 100/150/200% DPI, opacity, resize and idle redraw | Manual | Windows environment matrix required | NOT TESTED |
| P01-M02 | AC-003 Microsoft Pinyin | Manual | [`phase-03 manual IME checklist`](../report/phase-03-manual-ime-checklist.md) | NOT TESTED |
| P01-M03 | AC-004 WeChat Input Method | Manual | [`phase-03 manual IME checklist`](../report/phase-03-manual-ime-checklist.md) | NOT TESTED |
| P01-M04 | junction/non-ASCII identity, ACL, kill-mid-save, hardware-loss boundaries | Manual | Dedicated environment/fault receipts required | NOT TESTED |
