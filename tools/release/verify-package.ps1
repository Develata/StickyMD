[CmdletBinding()]
param(
    [string]$PackageDirectory,
    [string]$ZipPath,
    [string]$ChecksumPath,
    [switch]$Runtime
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $PSScriptRoot 'invoke-smoke.ps1')
$arguments = @('verify-package')
foreach ($entry in @(
    @('--package-directory', $PackageDirectory),
    @('--zip', $ZipPath),
    @('--checksums', $ChecksumPath)
)) {
    if ($entry[1]) {
        $arguments += $entry[0]
        $arguments += $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($entry[1])
    }
}
if ($Runtime) { $arguments += '--runtime' }
Invoke-StickyMdReleaseTool -RepoRoot $repoRoot -Arguments $arguments
