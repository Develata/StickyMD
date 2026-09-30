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
. (Join-Path $PSScriptRoot 'package-path.ps1')
. (Join-Path $PSScriptRoot 'invoke-smoke.ps1')
. (Join-Path $PSScriptRoot 'syft-download.ps1')
$workspaceVersion = Invoke-StickyMdReleaseTool -RepoRoot $repoRoot -Arguments @('workspace-version')
if (-not $PackageDirectory) { $PackageDirectory = Join-Path $repoRoot 'dist' }
$PackageDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($PackageDirectory)
if (-not $ZipPath) {
    $ZipPath = Resolve-StickyMdPackagePath -RepoRoot $repoRoot -PackageDirectory $PackageDirectory
}
$ZipPath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($ZipPath)
if (-not $OutputPath) { $OutputPath = Join-Path $PackageDirectory 'SBOM.spdx.json' }
$OutputPath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputPath)
if ($SyftPath) { $SyftPath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($SyftPath) }

$syftArguments = @('syft-plan')
if ($SyftPath) { $syftArguments += @('--syft-path', $SyftPath) }
$syftPlan = (Invoke-StickyMdReleaseTool -RepoRoot $repoRoot -Arguments $syftArguments) | ConvertFrom-Json
$SyftVersion = $syftPlan.version

$temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) ("stickymd-sbom-" + [guid]::NewGuid().ToString('N'))
$context = Join-Path $temporaryRoot 'context'
$stagedSbom = Join-Path $temporaryRoot 'generated.spdx.json'
New-Item -ItemType Directory -Path $context -Force | Out-Null
try {
    Copy-Item -LiteralPath (Join-Path $repoRoot 'Cargo.lock') -Destination (Join-Path $context 'Cargo.lock')
    Copy-Item -LiteralPath (Join-Path $repoRoot 'Cargo.toml') -Destination (Join-Path $context 'Cargo.toml')
    Expand-Archive -LiteralPath $ZipPath -DestinationPath (Join-Path $context 'package')

    if (-not $SyftPath) {
        $toolRoot = Join-Path $temporaryRoot 'syft'
        New-Item -ItemType Directory -Path $toolRoot | Out-Null
        foreach ($download in $syftPlan.downloads) {
            Get-PinnedSyftFile -RepoRoot $repoRoot -Entry $download -Attempts $syftPlan.download_attempts
        }
        $verified = (Invoke-StickyMdReleaseTool -RepoRoot $repoRoot -Arguments @(
            'syft-verify', '--archive', $syftPlan.archive_path, '--checksums', $syftPlan.checksums_path,
            '--staging-directory', (Join-Path $temporaryRoot 'verified-syft')
        )) | ConvertFrom-Json
        Expand-Archive -LiteralPath $verified.archive_path -DestinationPath $toolRoot
        $SyftPath = Join-Path $toolRoot 'syft.exe'
    }
    if (-not (Test-Path -LiteralPath $SyftPath -PathType Leaf)) { throw "Syft executable does not exist: $SyftPath" }

    $previousFileSelection = $env:SYFT_FILE_METADATA_SELECTION
    $previousUpdateCheck = $env:SYFT_CHECK_FOR_APP_UPDATE
    try {
        $env:SYFT_FILE_METADATA_SELECTION = 'all'
        $env:SYFT_CHECK_FOR_APP_UPDATE = 'false'
        & $SyftPath "dir:$context" '--source-name' 'StickyMD' '--source-version' $workspaceVersion '--output' "spdx-json=$stagedSbom"
        if ($LASTEXITCODE -ne 0) { throw "Syft $SyftVersion failed with exit code $LASTEXITCODE" }
    } finally {
        $env:SYFT_FILE_METADATA_SELECTION = $previousFileSelection
        $env:SYFT_CHECK_FOR_APP_UPDATE = $previousUpdateCheck
    }
    $checksumPath = Join-Path $PackageDirectory 'SHA256SUMS.txt'
    Invoke-StickyMdReleaseTool -RepoRoot $repoRoot -Arguments @(
        'publish-sbom', '--input', $stagedSbom, '--output', $OutputPath,
        '--zip', $ZipPath, '--checksums', $checksumPath
    )
    Write-Output "SYFT_VERSION=$SyftVersion"
} finally {
    $resolvedTemp = [IO.Path]::GetFullPath($temporaryRoot)
    $systemTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if (-not $resolvedTemp.StartsWith($systemTemp, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove unexpected temporary path: $resolvedTemp"
    }
    if (Test-Path -LiteralPath $resolvedTemp) { Remove-Item -LiteralPath $resolvedTemp -Recurse -Force }
}
