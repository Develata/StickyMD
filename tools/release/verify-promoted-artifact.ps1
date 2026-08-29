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
$artifactRoot = [IO.Path]::GetFullPath($ArtifactDirectory)
$SourceSha = $SourceSha.ToLowerInvariant()
$ExpectedZipSha256 = $ExpectedZipSha256.ToLowerInvariant()
$ExpectedSbomSha256 = $ExpectedSbomSha256.ToLowerInvariant()

if ($SourceSha -notmatch '^[0-9a-f]{40}$') { throw 'SourceSha must be a full lowercase commit SHA' }
foreach ($item in @($ExpectedZipSha256, $ExpectedSbomSha256)) {
    if ($item -notmatch '^[0-9a-f]{64}$') { throw 'Expected hashes must be lowercase SHA-256 values' }
}
$head = (& git -C $repoRoot rev-parse HEAD).Trim().ToLowerInvariant()
if ($LASTEXITCODE -ne 0 -or $head -ne $SourceSha) {
    throw "Checked-out source $head does not match approved source $SourceSha"
}
$manifest = Get-Content -LiteralPath (Join-Path $repoRoot 'Cargo.toml') -Raw
$versionMatch = [regex]::Match($manifest, '(?m)^version\s*=\s*"([^"]+)"\s*$')
if (-not $versionMatch.Success) { throw 'Workspace version is missing' }
$version = $versionMatch.Groups[1].Value
if ($ReleaseTag -ne "v$version") { throw "Release tag $ReleaseTag does not match v$version" }

$expectedZipName = "StickyMD-$version-windows-x64-portable.zip"
$zip = Join-Path $artifactRoot $expectedZipName
$sbom = Join-Path $artifactRoot 'SBOM.spdx.json'
$checksums = Join-Path $artifactRoot 'SHA256SUMS.txt'
foreach ($path in @($zip, $sbom, $checksums)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Exact promotion input is missing: $path"
    }
}
$unexpectedZip = @(Get-ChildItem -LiteralPath $artifactRoot -File -Filter '*.zip' | Where-Object Name -ne $expectedZipName)
if ($unexpectedZip.Count -ne 0) { throw 'Exact promotion input contains an unexpected ZIP' }

$zipHash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
$sbomHash = (Get-FileHash -LiteralPath $sbom -Algorithm SHA256).Hash.ToLowerInvariant()
if ($zipHash -ne $ExpectedZipSha256) { throw "ZIP hash $zipHash differs from approved $ExpectedZipSha256" }
if ($sbomHash -ne $ExpectedSbomSha256) { throw "SBOM hash $sbomHash differs from approved $ExpectedSbomSha256" }
$checksumLines = @(Get-Content -LiteralPath $checksums)
if ($checksumLines -notcontains "$zipHash *$expectedZipName") {
    throw 'SHA256SUMS.txt does not bind the approved ZIP'
}
if ($checksumLines -notcontains "$sbomHash *SBOM.spdx.json") {
    throw 'SHA256SUMS.txt does not bind the approved SBOM'
}

Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$stream = [IO.File]::OpenRead($zip)
try {
    $archive = [IO.Compression.ZipArchive]::new($stream, [IO.Compression.ZipArchiveMode]::Read, $false)
    try {
        $readmeEntries = @($archive.Entries | Where-Object FullName -eq 'StickyMD/README.txt')
        if ($readmeEntries.Count -ne 1) { throw 'Promoted ZIP must contain exactly one StickyMD/README.txt' }
        $reader = [IO.StreamReader]::new($readmeEntries[0].Open(), [Text.Encoding]::UTF8, $true)
        try { $readmeText = $reader.ReadToEnd() } finally { $reader.Dispose() }
    } finally { $archive.Dispose() }
} finally { $stream.Dispose() }
if (-not ($readmeText -split "`r?`n" -contains "Source commit: $SourceSha")) {
    throw "Promoted ZIP README does not bind approved source $SourceSha"
}

Write-Output "PROMOTION_INPUT=PASS"
Write-Output "PROMOTION_ZIP=$zip"
Write-Output "PROMOTION_ZIP_SHA256=$zipHash"
Write-Output "PROMOTION_SBOM_SHA256=$sbomHash"
