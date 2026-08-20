# Phase 00 Acceptance Matrix

> Verification projection for repository governance. Product behavior remains defined by
> [`00_v1_acceptance.md`](00_v1_acceptance.md); status rules come from
> [`plan 11`](../plan/11_testing_and_release.md#phase-verification-harness).

| ID | Plan / AC mapping | Mode | Checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P00-A01 | plan authority tree and required governance files | Automated | [`phase-00.ps1`](../../tools/smoke/phase-00.ps1): required-file check | AUTOMATED PASS |
| P00-A02 | AC-001..AC-030 stable verification case structure | Automated | [`phase-00.ps1`](../../tools/smoke/phase-00.ps1): AC sequence validator | AUTOMATED PASS |
| P00-A03 | plan_ref targets and stable anchors | Automated | [`phase-00.ps1`](../../tools/smoke/phase-00.ps1): production plan-ref validator | AUTOMATED PASS |
| P00-A04 | local Markdown links and forbidden root dependencies | Automated | [`phase-00.ps1`](../../tools/smoke/phase-00.ps1): governance validators | AUTOMATED PASS |
| P00-A05 | one smoke entry and one matrix for every retained Phase | Automated | [`phase-00.ps1`](../../tools/smoke/phase-00.ps1): phase-artifact validator | AUTOMATED PASS |
| P00-M01 | USER constitution semantic fidelity review | Manual | Current-commit section-by-section review receipt required | NOT TESTED |
| P00-M02 | architecture contract judgment review | Manual | Current-commit architecture checklist receipt required | NOT TESTED |

The automated rows are re-evaluated by the checked-in runner; manual judgment remains separate.
