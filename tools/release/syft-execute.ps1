[CmdletBinding()]
param([string]$Context, [string]$ZipPath, [string]$SyftPath, [string]$SyftArchive,
      [string]$WorkspaceVersion, [string]$SyftVersion, [string]$OutputPath)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
Add-Type -AssemblyName System.IO.Compression.FileSystem
# Match the existing archive-facts adapter: fresh destinations, no overwrite,
# framework ZIP path checks, and no Archive module startup for every subprocess.
[IO.Compression.ZipFile]::ExtractToDirectory($ZipPath, (Join-Path $Context 'package'))
if ($SyftArchive) {
    [IO.Compression.ZipFile]::ExtractToDirectory($SyftArchive, (Split-Path -Parent $SyftPath))
}
if (-not (Test-Path -LiteralPath $SyftPath -PathType Leaf)) { throw "Syft executable does not exist: $SyftPath" }
$previousFileSelection = $env:SYFT_FILE_METADATA_SELECTION
$previousUpdateCheck = $env:SYFT_CHECK_FOR_APP_UPDATE
try {
    $env:SYFT_FILE_METADATA_SELECTION = 'all'
    $env:SYFT_CHECK_FOR_APP_UPDATE = 'false'
    & $SyftPath "dir:$Context" '--source-name' 'StickyMD' '--source-version' $WorkspaceVersion '--output' "spdx-json=$OutputPath"
    if ($LASTEXITCODE -ne 0) { throw "Syft $SyftVersion failed with exit code $LASTEXITCODE" }
} finally {
    $env:SYFT_FILE_METADATA_SELECTION = $previousFileSelection
    $env:SYFT_CHECK_FOR_APP_UPDATE = $previousUpdateCheck
}
