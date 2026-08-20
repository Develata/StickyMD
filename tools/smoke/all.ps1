[CmdletBinding()]
param(
    [switch]$Ci,
    [switch]$Performance,
    [switch]$Runtime
)

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$smokeArguments = @('run', '-p', 'stickymd-smoke', '--locked', '--', 'all')
if ($Ci) { $smokeArguments += '--ci' }
if ($Performance) { $smokeArguments += '--performance' }
if ($Runtime) { $smokeArguments += '--runtime' }
$smokeExitCode = 1
Push-Location -LiteralPath $repoRoot
try {
    & cargo @smokeArguments
    $smokeExitCode = $LASTEXITCODE
} finally {
    Pop-Location
}
exit $smokeExitCode
