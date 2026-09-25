[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$DestinationPath
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $PSScriptRoot 'invoke-smoke.ps1')
$destination = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($DestinationPath)
Invoke-StickyMdReleaseTool -RepoRoot $repoRoot -Arguments @('notices', '--destination', $destination)
