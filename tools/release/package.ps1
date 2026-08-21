[CmdletBinding()]
param(
    [string]$ExePath,
    [string]$OutputDirectory,
    [string]$Version,
    [string]$CommitSha,
    [string]$ReleaseTag,
    [switch]$AllowDirtyValidation
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
if (-not $ExePath) { $ExePath = Join-Path $repoRoot 'target\release\stickymd-win.exe' }
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $repoRoot 'dist' }
$ExePath = [IO.Path]::GetFullPath($ExePath)
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)

if (-not (Test-Path -LiteralPath $ExePath -PathType Leaf)) {
    throw "Release executable does not exist: $ExePath"
}
if (-not $Version) {
    $workspaceManifest = Get-Content -LiteralPath (Join-Path $repoRoot 'Cargo.toml') -Raw
    $match = [regex]::Match($workspaceManifest, '(?m)^version\s*=\s*"([^"]+)"\s*$')
    if (-not $match.Success) { throw 'Cannot read workspace version from Cargo.toml' }
    $Version = $match.Groups[1].Value
}
if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?$') {
    throw "Invalid workspace version: $Version"
}
if (-not $CommitSha) {
    $CommitSha = (& git -C $repoRoot rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve current Git commit' }
}
if ($CommitSha -notmatch '^[0-9a-fA-F]{40}$') { throw "Invalid full commit SHA: $CommitSha" }
$shortSha = $CommitSha.Substring(0, 12).ToLowerInvariant()
$dirty = [bool](& git -C $repoRoot status --porcelain)
if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect Git working tree state' }
if ($dirty -and -not $AllowDirtyValidation) {
    throw 'Refusing to label a dirty working tree as a local RC; commit first or use -AllowDirtyValidation for non-RC script validation'
}
if ($ReleaseTag) {
    if ($ReleaseTag -ne "v$Version") { throw "Release tag $ReleaseTag does not match workspace version v$Version" }
    if ($dirty) { throw 'A tagged release package cannot be built from a dirty working tree' }
    $archiveName = "StickyMD-$ReleaseTag-windows-x64-portable.zip"
} else {
    $qualifier = if ($dirty) { "local-validation-$shortSha-dirty" } else { "local-rc-$shortSha" }
    $archiveName = "StickyMD-$Version-$qualifier-windows-x64-portable.zip"
}
$archivePath = Join-Path $OutputDirectory $archiveName

New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) ("stickymd-package-" + [guid]::NewGuid().ToString('N'))
$packageRoot = Join-Path $temporaryRoot 'StickyMD'
New-Item -ItemType Directory -Path (Join-Path $packageRoot 'licenses') -Force | Out-Null

try {
    Copy-Item -LiteralPath $ExePath -Destination (Join-Path $packageRoot 'StickyMD.exe')
    Copy-Item -LiteralPath (Join-Path $repoRoot 'LICENSE') -Destination (Join-Path $packageRoot 'LICENSE.txt')
    & (Join-Path $repoRoot 'tools\release\generate-third-party-notices.ps1') -DestinationPath (Join-Path $packageRoot 'THIRD_PARTY_NOTICES.txt')
    if ($LASTEXITCODE -ne 0) { throw 'Runtime dependency notice generation failed' }
    Copy-Item -LiteralPath (Join-Path $repoRoot 'assets\licenses\SIL-OFL-1.1.txt') -Destination (Join-Path $packageRoot 'licenses\SIL-OFL-1.1.txt')
    Copy-Item -LiteralPath (Join-Path $repoRoot 'assets\licenses\KaTeX-fonts-NOTICE.txt') -Destination (Join-Path $packageRoot 'licenses\KaTeX-fonts-NOTICE.txt')

    $readme = @(
        'StickyMD portable release candidate for Windows 11 x64'
        "Version: $Version"
        "Source commit: $($CommitSha.ToLowerInvariant())"
        ''
        'Run StickyMD.exe from a writable directory. The program creates its only working note under .\note\note.md.'
        'Do not place the executable under Program Files or another directory that requires administrator rights.'
        'Closing the paper hides StickyMD to the notification area; use the tray menu Exit command to quit.'
        'Markdown Preview is native and supports the documented CommonMark/GFM profile plus RaTeX math.'
        'Remote images are never downloaded; their alt text and link remain available.'
        ''
        'This build is unsigned. Windows reputation warnings may appear; verify the SHA-256 checksum before running it.'
        'License: MIT. Complete Rust dependency notices and the KaTeX font license are included in this package.'
        'Project: https://github.com/Develata/StickyMD'
    ) -join "`r`n"
    [IO.File]::WriteAllText((Join-Path $packageRoot 'README.txt'), $readme + "`r`n", [Text.UTF8Encoding]::new($false))

    if (Test-Path -LiteralPath $archivePath) { Remove-Item -LiteralPath $archivePath -Force }
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $stream = [IO.File]::Open($archivePath, [IO.FileMode]::CreateNew, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
    try {
        $archive = [IO.Compression.ZipArchive]::new($stream, [IO.Compression.ZipArchiveMode]::Create, $false)
        try {
            $files = Get-ChildItem -LiteralPath $packageRoot -File -Recurse | Sort-Object { $_.FullName.Substring($temporaryRoot.Length) }
            foreach ($file in $files) {
                $relative = [IO.Path]::GetRelativePath($temporaryRoot, $file.FullName).Replace('\', '/')
                $entry = $archive.CreateEntry($relative, [IO.Compression.CompressionLevel]::Optimal)
                $entry.LastWriteTime = [DateTimeOffset]::new(1980, 1, 1, 0, 0, 0, [TimeSpan]::Zero)
                $input = [IO.File]::OpenRead($file.FullName)
                $output = $entry.Open()
                try { $input.CopyTo($output) } finally { $output.Dispose(); $input.Dispose() }
            }
        } finally { $archive.Dispose() }
    } finally { $stream.Dispose() }

    $zipHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    $checksumPath = Join-Path $OutputDirectory 'SHA256SUMS.txt'
    [IO.File]::WriteAllText($checksumPath, "$zipHash *$archiveName`n", [Text.UTF8Encoding]::new($false))
    Write-Output "PACKAGE_PATH=$archivePath"
    Write-Output "PACKAGE_SHA256=$zipHash"
    Write-Output "SOURCE_TREE_STATE=$(if ($dirty) { 'DIRTY_VALIDATION' } else { 'CLEAN_RC' })"
} finally {
    $resolvedTemp = [IO.Path]::GetFullPath($temporaryRoot)
    $systemTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if (-not $resolvedTemp.StartsWith($systemTemp, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove unexpected temporary path: $resolvedTemp"
    }
    if (Test-Path -LiteralPath $resolvedTemp) { Remove-Item -LiteralPath $resolvedTemp -Recurse -Force }
}
