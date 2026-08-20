[CmdletBinding()]
param(
    [switch]$Performance,
    [switch]$Runtime
)

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$smokeArguments = @('run', '-p', 'stickymd-smoke', '--locked', '--', 'phase', '03')
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
