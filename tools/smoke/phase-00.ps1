[CmdletBinding()]
param()

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$smokeArguments = @('run', '-p', 'stickymd-smoke', '--locked', '--', 'phase', '00')
$smokeExitCode = 1
Push-Location -LiteralPath $repoRoot
try {
    & cargo @smokeArguments
    $smokeExitCode = $LASTEXITCODE
} finally {
    Pop-Location
}
exit $smokeExitCode
