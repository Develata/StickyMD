[CmdletBinding()]
param(
    [switch]$Ci,
    [ValidateSet('tests', 'performance')]
    [string]$CiShard,
    [switch]$Performance,
    [switch]$Runtime,
    [switch]$Resources,
    [ValidateSet('source-preview', 'math', 'images', 'window', 'zoom')]
    [string]$ResourceModule,
    [switch]$Release,
    [switch]$Package,
    [switch]$Json
)

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$smokeArguments = @('run', '-p', 'stickymd-smoke', '--locked', '--', 'all')
if ($Ci) { $smokeArguments += '--ci' }
if ($CiShard) { $smokeArguments += "--ci-shard=$CiShard" }
if ($Performance) { $smokeArguments += '--performance' }
if ($Runtime) { $smokeArguments += '--runtime' }
if ($Resources) { $smokeArguments += '--resources' }
if ($ResourceModule) { $smokeArguments += "--resource-module=$ResourceModule" }
if ($Release) { $smokeArguments += '--release' }
if ($Package) { $smokeArguments += '--package' }
if ($Json) { $smokeArguments += '--json' }
$smokeExitCode = 1
Push-Location -LiteralPath $repoRoot
try {
    & cargo @smokeArguments
    $smokeExitCode = $LASTEXITCODE
} finally {
    Pop-Location
}
exit $smokeExitCode
