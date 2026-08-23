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
    [ValidateSet('M1', 'M2', 'M3', 'M4', 'M5')]
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
$qualificationActions = @(
    $Environment,
    $Campaign,
    $Candidate,
    [bool]$DecisionKey,
    $Manual,
    [bool]$ManualSession,
    $ManualList,
    $ManualStatus,
    $Readiness,
    [bool]$RemoteRunId,
    [bool]$DownloadedZip
) | Where-Object { $_ }
if ($qualificationActions.Count -gt 1) { throw 'Select at most one qualification action' }

$arguments = @('run', '-p', 'stickymd-smoke', '--locked', '--')
if ($Environment) {
    $arguments += @('qualification', 'environment')
    if ($EvidenceFile) { $arguments += "--evidence-file=$EvidenceFile" }
} elseif ($Campaign) {
    $arguments += @('qualification', 'local')
} elseif ($Candidate) {
    $arguments += @('qualification', 'candidate')
} elseif ($Manual -or $ManualSession) {
    $arguments += @('acceptance', 'manual', 'run')
    if ($ManualSession) { $arguments += "--session=$ManualSession" }
} elseif ($ManualList) {
    $arguments += @('acceptance', 'manual', 'list')
} elseif ($ManualStatus) {
    $arguments += @('acceptance', 'manual', 'status')
} elseif ($DecisionKey) {
    if (-not $DecisionStatus -or -not $DecisionEvidence) {
        throw 'DecisionStatus and DecisionEvidence are required with DecisionKey'
    }
    $arguments += @(
        'qualification',
        'decision',
        "--key=$DecisionKey",
        "--status=$DecisionStatus",
        "--evidence=$DecisionEvidence"
    )
} elseif ($Readiness) {
    $arguments += @('qualification', 'readiness')
    if ($Explain) { $arguments += '--explain' }
} elseif ($RemoteRunId) {
    if (-not $RemoteAttempt) { throw 'RemoteAttempt is required with RemoteRunId' }
    $arguments += @('qualification', 'remote', "--run-id=$RemoteRunId", "--attempt=$RemoteAttempt")
} elseif ($DownloadedZip) {
    $arguments += @('qualification', 'downloaded', "--zip=$DownloadedZip")
} else {
    $arguments += @('phase', '13')
    if ($Ci) { $arguments += '--ci' }
    if ($Performance) { $arguments += '--performance' }
    if ($Runtime) { $arguments += '--runtime' }
    if ($Resources) { $arguments += '--resources' }
    if ($Release) { $arguments += '--release' }
    if ($Package) { $arguments += '--package' }
    if ($Json) { $arguments += '--json' }
    if ($EvidenceFile) { $arguments += "--evidence-file=$EvidenceFile" }
}

$exitCode = 1
Push-Location -LiteralPath $repoRoot
try {
    & cargo @arguments
    $exitCode = $LASTEXITCODE
} finally {
    Pop-Location
}
exit $exitCode
