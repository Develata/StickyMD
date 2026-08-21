[CmdletBinding()]
param(
    [switch]$Performance,
    [switch]$Runtime,
    [switch]$Resources
)

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$smokeArguments = @('run', '-p', 'stickymd-smoke', '--locked', '--', 'phase', '07')
if ($Performance) { $smokeArguments += '--performance' }
if ($Runtime) { $smokeArguments += '--runtime' }
if ($Resources) { $smokeArguments += '--resources' }
$smokeExitCode = 1
Push-Location -LiteralPath $repoRoot
try {
    & cargo @smokeArguments
    $smokeExitCode = $LASTEXITCODE
} finally {
    Pop-Location
}
exit $smokeExitCode
