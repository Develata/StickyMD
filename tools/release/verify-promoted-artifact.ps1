[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ArtifactDirectory,
    [Parameter(Mandatory = $true)][string]$SourceSha,
    [Parameter(Mandatory = $true)][string]$ExpectedZipSha256,
    [Parameter(Mandatory = $true)][string]$ExpectedSbomSha256,
    [Parameter(Mandatory = $true)][string]$ReleaseTag
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $PSScriptRoot 'invoke-smoke.ps1')
$artifactRoot = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($ArtifactDirectory)
Invoke-StickyMdReleaseTool -RepoRoot $repoRoot -Arguments @(
    'verify-promoted', '--artifact-directory', $artifactRoot,
    '--source-sha', $SourceSha, '--expected-zip-sha256', $ExpectedZipSha256,
    '--expected-sbom-sha256', $ExpectedSbomSha256, '--release-tag', $ReleaseTag
)
