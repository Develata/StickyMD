# StickyMD Phase Smoke CLI

`stickymd-smoke` is a development-only, std-only Rust CLI. It is the reusable
execution engine behind the stable PowerShell entry points in `tools/smoke/`.
It is not linked into `StickyMD.exe` and is not included in the portable
release package.

## Stable entry points

```powershell
./tools/smoke/phase-00.ps1
./tools/smoke/phase-01.ps1 -Performance
./tools/smoke/phase-02.ps1 -Performance
./tools/smoke/phase-03.ps1 -Performance -Runtime
./tools/smoke/phase-04.ps1 -Performance -Runtime
./tools/smoke/phase-05.ps1 -Performance -Runtime
./tools/smoke/phase-05.ps1 -Resources
./tools/smoke/phase-06.ps1 -Performance -Runtime
./tools/smoke/phase-06.ps1 -Resources
./tools/smoke/phase-07.ps1 -Performance -Runtime
./tools/smoke/phase-07.ps1 -Resources
./tools/smoke/all.ps1 -Ci
```

For a local diagnosis of one opt-in resource case, set
`STICKYMD_SMOKE_RESOURCE_CASE` to the exact case label before invoking the owning phase script.
This development filter is never set by CI or by the durable full-matrix receipts.

`-Ci` runs every headless check, including the Release performance entry
points. Stable hard thresholds may fail CI; machine-specific measurements are
diagnostic only. `-Performance` reruns the same measurements explicitly on a
local machine. `-Runtime` creates native windows and remains local-only. The
CLI rejects combining either explicit local mode with `-Ci`.

`-Resources` is the long-running Windows resource measurement. It launches copied standalone
Release executables, waits 30 seconds, records private working set/private bytes over five runs,
and measures 60-second idle CPU for Source, Preview and Split. It is never part of headless CI.

## Acceptance status

The persistent result for each phase lives in
`docs/acceptance-cases/phase-XX.md`. Automated checks may be marked
`AUTOMATED PASS` only when their checked-in runner passes. Manual checks stay
`NOT TESTED` until a durable receipt is checked in; terminal output from a
one-off run is not such a receipt.
