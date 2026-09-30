[CmdletBinding()]
param(
    [string]$ExePath,
    [string]$OutputDirectory,
    [string]$Version,
    [string]$CommitSha,
    [string]$ReleaseTag,
    [switch]$ExactCandidate,
    [switch]$AllowDirtyValidation
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
if (-not $ExePath) { $ExePath = Join-Path $repoRoot 'target\release\stickymd-win.exe' }
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $repoRoot 'dist' }
$ExePath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($ExePath)
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)

. (Join-Path $PSScriptRoot 'invoke-smoke.ps1')
$shellName = if ($PSVersionTable.PSEdition -eq 'Desktop') { 'powershell.exe' } else { 'pwsh.exe' }
$arguments = @('build-package', '--exe', $ExePath, '--output-directory', $OutputDirectory, '--powershell', (Join-Path $PSHOME $shellName))
if ($Version) { $arguments += @('--version', $Version) }
if ($CommitSha) { $arguments += @('--commit-sha', $CommitSha) }
if ($ReleaseTag) { $arguments += @('--release-tag', $ReleaseTag) }
if ($ExactCandidate) { $arguments += '--exact-candidate' }
if ($AllowDirtyValidation) { $arguments += '--allow-dirty-validation' }
Invoke-StickyMdReleaseTool -RepoRoot $repoRoot -Arguments $arguments
