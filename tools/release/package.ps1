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

if (-not (Test-Path -LiteralPath $ExePath -PathType Leaf)) {
    throw "Release executable does not exist: $ExePath"
}
. (Join-Path $PSScriptRoot 'invoke-smoke.ps1')
$temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) ("stickymd-package-" + [guid]::NewGuid().ToString('N'))
$stagingRoot = Join-Path $temporaryRoot 'contents'
$inputArguments = @('prepare-package', '--exe', $ExePath, '--staging-directory', $stagingRoot)
if ($Version) { $inputArguments += @('--version', $Version) }
if ($CommitSha) { $inputArguments += @('--commit-sha', $CommitSha) }
if ($ReleaseTag) { $inputArguments += @('--release-tag', $ReleaseTag) }
if ($ExactCandidate) { $inputArguments += '--exact-candidate' }
if ($AllowDirtyValidation) { $inputArguments += '--allow-dirty-validation' }
$archiveTemporaryPath = $null
New-Item -ItemType Directory -Path $temporaryRoot | Out-Null

try {
    $prepared = (Invoke-StickyMdReleaseTool -RepoRoot $repoRoot -Arguments $inputArguments) | ConvertFrom-Json
    $inputs = $prepared.inputs
    $prepared.diagnostics | Write-Output
    $archiveName = $inputs.archive_name
    $archivePath = Join-Path $OutputDirectory $archiveName
    $archiveTemporaryPath = Join-Path $OutputDirectory (".$archiveName." + [guid]::NewGuid().ToString('N') + '.tmp')
    New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null

    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $stream = [IO.File]::Open($archiveTemporaryPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
    try {
        $archive = [IO.Compression.ZipArchive]::new($stream, [IO.Compression.ZipArchiveMode]::Create, $false)
        try {
            foreach ($relative in $prepared.members) {
                $entry = $archive.CreateEntry($relative, [IO.Compression.CompressionLevel]::Optimal)
                $entry.LastWriteTime = [DateTimeOffset]::new(1980, 1, 1, 0, 0, 0, [TimeSpan]::Zero)
                $input = [IO.File]::OpenRead((Join-Path $stagingRoot $relative))
                $output = $entry.Open()
                try { $input.CopyTo($output) } finally { $output.Dispose(); $input.Dispose() }
            }
        } finally { $archive.Dispose() }
    } finally { $stream.Dispose() }

    if (Test-Path -LiteralPath $archivePath) {
        $existingHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash
        $temporaryHash = (Get-FileHash -LiteralPath $archiveTemporaryPath -Algorithm SHA256).Hash
        if (-not $existingHash.Equals($temporaryHash, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to overwrite a different existing package: $archivePath"
        }
    } else {
        [IO.File]::Move($archiveTemporaryPath, $archivePath)
    }

    $checksumPath = Join-Path $OutputDirectory 'SHA256SUMS.txt'
    $hashes = (Invoke-StickyMdReleaseTool -RepoRoot $repoRoot -Arguments @(
        'checksums', '--zip', $archivePath, '--output', $checksumPath
    )) | ConvertFrom-Json
    Write-Output "PACKAGE_PATH=$archivePath"
    Write-Output "PACKAGE_SHA256=$($hashes.zip_sha256)"
    $sourceTreeState = $inputs.source_tree_state
    Write-Output "SOURCE_TREE_STATE=$sourceTreeState"
} finally {
    $resolvedTemp = [IO.Path]::GetFullPath($temporaryRoot)
    $systemTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if (-not $resolvedTemp.StartsWith($systemTemp, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove unexpected temporary path: $resolvedTemp"
    }
    if (Test-Path -LiteralPath $resolvedTemp) { Remove-Item -LiteralPath $resolvedTemp -Recurse -Force }
    if ($archiveTemporaryPath) {
        $resolvedArchiveTemporaryPath = [IO.Path]::GetFullPath($archiveTemporaryPath)
        $resolvedOutputDirectory = [IO.Path]::GetFullPath($OutputDirectory).TrimEnd('\') + '\'
        if (-not $resolvedArchiveTemporaryPath.StartsWith($resolvedOutputDirectory, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove unexpected package temporary path: $resolvedArchiveTemporaryPath"
        }
        if (Test-Path -LiteralPath $resolvedArchiveTemporaryPath) {
            Remove-Item -LiteralPath $resolvedArchiveTemporaryPath -Force
        }
    }
}
