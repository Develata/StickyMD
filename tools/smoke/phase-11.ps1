[CmdletBinding()]
param(
    [switch]$Performance,
    [switch]$Runtime,
    [switch]$Resources,
    [switch]$Release,
    [switch]$Package,
    [switch]$Json,
    [string]$EvidenceFile
)

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$smokeArguments = @('run', '-p', 'stickymd-smoke', '--locked', '--', 'phase', '11')
if ($Performance) { $smokeArguments += '--performance' }
if ($Runtime) { $smokeArguments += '--runtime' }
if ($Resources) { $smokeArguments += '--resources' }
if ($Release) { $smokeArguments += '--release' }
if ($Package) { $smokeArguments += '--package' }
if ($Json) { $smokeArguments += '--json' }
if ($EvidenceFile) { $smokeArguments += "--evidence-file=$EvidenceFile" }
$smokeExitCode = 1
Push-Location -LiteralPath $repoRoot
try {
    & cargo @smokeArguments
    $smokeExitCode = $LASTEXITCODE
} finally {
    Pop-Location
}
exit $smokeExitCode
