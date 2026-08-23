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
    [switch]$Candidate,
    [string]$DecisionKey,
    [string]$DecisionStatus,
    [string]$DecisionEvidence,
    [switch]$Manual,
    [switch]$Readiness,
    [switch]$Explain,
    [UInt64]$RemoteRunId,
    [UInt64]$RemoteAttempt,
    [string]$DownloadedZip
)

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$selectedQualificationActions = @($Candidate, [bool]$DecisionKey, $Manual, $Readiness, [bool]$RemoteRunId, [bool]$DownloadedZip) | Where-Object { $_ }
if ($selectedQualificationActions.Count -gt 1) { throw 'Select at most one qualification action' }

$smokeArguments = @('run', '-p', 'stickymd-smoke', '--locked', '--')
if ($Candidate) {
    $smokeArguments += @('qualification', 'candidate')
} elseif ($Manual) {
    $smokeArguments += @('acceptance', 'manual')
} elseif ($DecisionKey) {
    if (-not $DecisionStatus -or -not $DecisionEvidence) { throw 'DecisionStatus and DecisionEvidence are required with DecisionKey' }
    $smokeArguments += @('qualification', 'decision', "--key=$DecisionKey", "--status=$DecisionStatus", "--evidence=$DecisionEvidence")
} elseif ($Readiness) {
    $smokeArguments += @('qualification', 'readiness')
    if ($Explain) { $smokeArguments += '--explain' }
} elseif ($RemoteRunId) {
    if (-not $RemoteAttempt) { throw 'RemoteAttempt is required with RemoteRunId' }
    $smokeArguments += @('qualification', 'remote', "--run-id=$RemoteRunId", "--attempt=$RemoteAttempt")
} elseif ($DownloadedZip) {
    $smokeArguments += @('qualification', 'downloaded', "--zip=$DownloadedZip")
} else {
    $smokeArguments += @('phase', '12')
    if ($Ci) { $smokeArguments += '--ci' }
    if ($Performance) { $smokeArguments += '--performance' }
    if ($Runtime) { $smokeArguments += '--runtime' }
    if ($Resources) { $smokeArguments += '--resources' }
    if ($Release) { $smokeArguments += '--release' }
    if ($Package) { $smokeArguments += '--package' }
    if ($Json) { $smokeArguments += '--json' }
    if ($EvidenceFile) { $smokeArguments += "--evidence-file=$EvidenceFile" }
}

$smokeExitCode = 1
Push-Location -LiteralPath $repoRoot
try {
    & cargo @smokeArguments
    $smokeExitCode = $LASTEXITCODE
} finally {
    Pop-Location
}
exit $smokeExitCode
