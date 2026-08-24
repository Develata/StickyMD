[CmdletBinding()]
param(
    [switch]$Ci,
    [switch]$Performance,
    [switch]$Runtime,
    [switch]$Resources,
    [ValidateSet('source-preview', 'math', 'images', 'window', 'zoom')]
    [string]$ResourceModule,
    [switch]$Release,
    [switch]$Package,
    [switch]$Json,
    [string]$EvidenceFile,
    [switch]$Environment,
    [switch]$Campaign,
    [switch]$Candidate,
    [switch]$Attribution,
    [switch]$WindowStress,
    [ValidateSet('collapse', 'tray', 'controls', 'collapse-tray', 'combined')]
    [string]$WindowStressScenario = 'combined',
    [ValidateRange(1, 100)]
    [int]$WindowStressRuns = 10,
    [ValidateRange(0, 10000)]
    [int]$CollapseCycles = 1000,
    [ValidateRange(0, 10000)]
    [int]$TrayCycles = 100,
    [ValidateRange(0, 10000)]
    [int]$ControlCycles = 100,
    [ValidateRange(0, 10000)]
    [int]$PersistenceCycles = 100,
    [string]$DecisionKey,
    [string]$DecisionStatus,
    [string]$DecisionEvidence,
    [switch]$Manual,
    [ValidateSet('M1', 'M2', 'M3', 'M4', 'M5')]
    [string]$ManualSession,
    [switch]$Guided,
    [ValidateSet('G1', 'G2', 'G3')]
    [string]$GuidedSession,
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
    $Attribution,
    $WindowStress,
    [bool]$DecisionKey,
    $Manual,
    [bool]$ManualSession,
    $Guided,
    [bool]$GuidedSession,
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
} elseif ($Attribution) {
    $arguments += @('qualification', 'attribution')
} elseif ($WindowStress) {
    $arguments += @(
        'qualification',
        'window-stress',
        "--scenario=$WindowStressScenario",
        "--runs=$WindowStressRuns",
        "--collapse-cycles=$CollapseCycles",
        "--tray-cycles=$TrayCycles",
        "--control-cycles=$ControlCycles",
        "--persistence-cycles=$PersistenceCycles"
    )
} elseif ($Manual -or $ManualSession) {
    $arguments += @('acceptance', 'manual', 'run')
    if ($ManualSession) { $arguments += "--session=$ManualSession" }
} elseif ($Guided -or $GuidedSession) {
    $arguments += @('acceptance', 'manual', 'guided')
    if ($GuidedSession) { $arguments += "--session=$GuidedSession" }
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
    $arguments += @('phase', '14')
    if ($Ci) { $arguments += '--ci' }
    if ($Performance) { $arguments += '--performance' }
    if ($Runtime) { $arguments += '--runtime' }
    if ($Resources) { $arguments += '--resources' }
    if ($ResourceModule) { $arguments += "--resource-module=$ResourceModule" }
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
