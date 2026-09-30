# Failed task runs retain diagnostic JSON when -Json/-EvidenceFile is requested;
# only complete PASS results can update the qualification last-success ledger.
# Resource evidence files include INCOMPLETE checkpoints between major stages;
# G3/G4/G5 also retain failed case results without updating the success ledger.
# Window resources restore the fixed note after leak stress and verify every hidden sample.
# G5 uses shared image fixtures validated by the render crate's actual decoder tests.
# Module fingerprints isolate concurrent temporary streams even when clock values coincide.
# Full canonical Resources saves five complete groups independently and reuses compatible success.
# ResourceModule/case filters remain diagnostics; equivalent cohorts share samples in one command.
# Candidate measurements validate staged artifacts without building an unused local Release EXE.
# Resource any/max hard failures stop after a complete sample and retain partial failure evidence.
# Canonical Runtime/Performance share complete source-bound workspace tests with fresh identity checks.
# Rust reports task, identity and resource-planning timings; historical reuse time remains distinct.
# Fingerprint planning streams shared inputs once; reuse/promotion still read fresh bytes.
# Reserved output aliases are rejected before execution; shared resource cohorts retain raw samples/gates.
# Each measured resource group starts with a disposable physical desktop probe; blocked input records bounded window identity.
# Resource cases checkpoint INCOMPLETE samples; .progress.json reports rounds, stages and remaining fixed wait outside sampling windows.
# ResourceResume is diagnostic-only: complete five-sample cases or whole Window/Zoom groups, with strict identity in ignored target storage.
# ResourcePlan emits advisory NOT_RUN JSON without running resource tasks; identity timings remain outside sample windows.
# ResourceFailureFirst requires diagnostic resume; it keeps prerequisites and scope, and measures the selected failed unit fresh.
# Diagnostic resume can use registered equivalent cases across commands without renewing historical observations.
# Fully cached remaining cases in one group use fresh before/after batch validation; partial hits fall back to individual execution.
# Package inventory/README and Syft cache validation are owned by Rust; ZIP/network adapters stay in PowerShell.
[CmdletBinding()]
param(
    [switch]$Ci,
    [switch]$Performance,
    [switch]$Runtime,
    [switch]$Resources,
    [string]$ResourceModule,
    [switch]$ResourceResume,
    [switch]$ResourcePlan,
    [switch]$ResourceFailureFirst,
    [switch]$Release,
    [switch]$Package,
    [switch]$Json,
    [string]$EvidenceFile,
    [switch]$Environment,
    [switch]$Campaign,
    [switch]$SourceFreeze,
    [switch]$Attribution,
    [switch]$WindowStress,
    [string]$WindowStressScenario,
    [int]$WindowStressRuns,
    [int]$CollapseCycles,
    [int]$TrayCycles,
    [int]$ControlCycles,
    [int]$ViewModeCycles,
    [int]$PersistenceCycles,
    [string]$DecisionKey,
    [string]$DecisionStatus,
    [string]$DecisionEvidence,
    [switch]$Manual,
    [string]$ManualSession,
    [switch]$Guided,
    [string]$GuidedSession,
    [switch]$G3,
    [string]$G3Zip,
    [string]$G3Case,
    [switch]$G4,
    [string]$G4Zip,
    [string]$G4Case,
    [switch]$G5,
    [string]$G5Zip,
    [string]$G5Case,
    [switch]$ManualList,
    [switch]$ManualStatus,
    [switch]$Readiness,
    [switch]$Explain,
    [UInt64]$RemoteRunId,
    [UInt64]$RemoteAttempt,
    [string]$DownloadedZip
)

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $PSScriptRoot 'invoke-phase.ps1')
Invoke-StickyMdPhase -RepoRoot $repoRoot -Phase '14' -Parameters $PSBoundParameters
exit $LASTEXITCODE
