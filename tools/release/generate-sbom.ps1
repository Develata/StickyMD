[CmdletBinding()]
param(
    [string]$PackageDirectory,
    [string]$ZipPath,
    [string]$OutputPath,
    [string]$SyftPath
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $PSScriptRoot 'invoke-smoke.ps1')

if (-not $PackageDirectory) { $PackageDirectory = Join-Path $repoRoot 'dist' }
$PackageDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($PackageDirectory)
if ($ZipPath) { $ZipPath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($ZipPath) }
if (-not $OutputPath) { $OutputPath = Join-Path $PackageDirectory 'SBOM.spdx.json' }
$OutputPath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputPath)
if ($SyftPath) { $SyftPath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($SyftPath) }

$shellName = if ($PSVersionTable.PSEdition -eq 'Desktop') { 'powershell.exe' } else { 'pwsh.exe' }
$arguments = @('generate-sbom', '--package-directory', $PackageDirectory, '--output', $OutputPath, '--powershell', (Join-Path $PSHOME $shellName))
if ($ZipPath) { $arguments += @('--zip', $ZipPath) }
if ($SyftPath) { $arguments += @('--syft-path', $SyftPath) }
Invoke-StickyMdReleaseTool -RepoRoot $repoRoot -Arguments $arguments
