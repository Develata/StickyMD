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
    [ValidateSet('collapse', 'tray', 'controls', 'view-mode', 'collapse-tray', 'combined')]
    [string]$WindowStressScenario = 'combined',
    [ValidateRange(1, 1000)]
    [int]$WindowStressRuns = 10,
    [ValidateRange(0, 10000)]
    [int]$CollapseCycles = 1000,
    [ValidateRange(0, 10000)]
    [int]$TrayCycles = 100,
    [ValidateRange(0, 10000)]
    [int]$ControlCycles = 100,
    [ValidateRange(0, 10000)]
    [int]$ViewModeCycles = 100,
    [ValidateRange(0, 10000)]
    [int]$PersistenceCycles = 100,
    [string]$DecisionKey,
    [string]$DecisionStatus,
    [string]$DecisionEvidence,
    [switch]$Manual,
    [ValidateSet('M1', 'M2', 'M3', 'M4', 'M5')]
    [string]$ManualSession,
    [switch]$Guided,
    [ValidateSet('G1', 'G2')]
    [string]$GuidedSession,
    [switch]$G3,
    [string]$G3Zip,
    [ValidateSet('G3-01', 'G3-02', 'G3-03', 'G3-04', 'G3-05')]
    [string]$G3Case,
    [switch]$G4,
    [string]$G4Zip,
    [ValidateSet('G4-01', 'G4-02', 'G4-03', 'G4-04', 'G4-05', 'G4-06')]
    [string]$G4Case,
    [switch]$G5,
    [string]$G5Zip,
    [ValidateSet('G5-01', 'G5-02', 'G5-03', 'G5-04')]
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
if ($G3Zip -and -not $G3) { throw 'G3Zip requires -G3' }
if ($G3Case -and -not $G3) { throw 'G3Case requires -G3' }
if ($G4Zip -and -not $G4) { throw 'G4Zip requires -G4' }
if ($G4Case -and -not $G4) { throw 'G4Case requires -G4' }
if ($G5Zip -and -not $G5) { throw 'G5Zip requires -G5' }
if ($G5Case -and -not $G5) { throw 'G5Case requires -G5' }
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
    $G3,
    $G4,
    $G5,
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
        "--view-mode-cycles=$ViewModeCycles",
        "--persistence-cycles=$PersistenceCycles"
    )
} elseif ($Manual -or $ManualSession) {
    $arguments += @('acceptance', 'manual', 'run')
    if ($ManualSession) { $arguments += "--session=$ManualSession" }
} elseif ($Guided -or $GuidedSession) {
    $arguments += @('acceptance', 'manual', 'guided')
    if ($GuidedSession) { $arguments += "--session=$GuidedSession" }
} elseif ($G3) {
    $arguments += @('qualification', 'g3')
    if ($G3Zip) { $arguments += "--zip=$G3Zip" }
    if ($EvidenceFile) { $arguments += "--evidence-file=$EvidenceFile" }
    if ($G3Case) { $arguments += "--case=$G3Case" }
} elseif ($G4) {
    $arguments += @('qualification', 'g4')
    if ($G4Zip) { $arguments += "--zip=$G4Zip" }
    if ($EvidenceFile) { $arguments += "--evidence-file=$EvidenceFile" }
    if ($G4Case) { $arguments += "--case=$G4Case" }
} elseif ($G5) {
    $arguments += @('qualification', 'g5')
    if ($G5Zip) { $arguments += "--zip=$G5Zip" }
    if ($EvidenceFile) { $arguments += "--evidence-file=$EvidenceFile" }
    if ($G5Case) { $arguments += "--case=$G5Case" }
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
