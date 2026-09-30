[CmdletBinding()]
param(
    [switch]$Ci,
    [switch]$Performance,
    [switch]$Runtime,
    [switch]$Resources,
    [switch]$Release,
    [switch]$Package,
    [switch]$Json,
    [string]$EvidenceFile,
    [switch]$Environment,
    [switch]$Campaign,
    [switch]$Candidate,
    [string]$DecisionKey,
    [string]$DecisionStatus,
    [string]$DecisionEvidence,
    [switch]$Manual,
    [string]$ManualSession,
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
Invoke-StickyMdPhase -RepoRoot $repoRoot -Phase '13' -Parameters $PSBoundParameters
exit $LASTEXITCODE
