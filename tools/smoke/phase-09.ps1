[CmdletBinding()]
param(
    [switch]$Performance,
    [switch]$Release,
    [switch]$Package
)

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$smokeArguments = @('run', '-p', 'stickymd-smoke', '--locked', '--', 'phase', '09')
if ($Performance) { $smokeArguments += '--performance' }
if ($Release) { $smokeArguments += '--release' }
if ($Package) { $smokeArguments += '--package' }
$smokeExitCode = 1
Push-Location -LiteralPath $repoRoot
try {
    & cargo @smokeArguments
    $smokeExitCode = $LASTEXITCODE
} finally {
    Pop-Location
}
exit $smokeExitCode
