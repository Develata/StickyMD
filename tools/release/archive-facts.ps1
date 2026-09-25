[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][ValidateSet('List', 'Read', 'Extract')][string]$Operation,
    [Parameter(Mandatory = $true)][string]$ZipPath,
    [int]$EntryIndex,
    [string]$DestinationPath
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
if ($Operation -eq 'Extract') {
    [IO.Compression.ZipFile]::ExtractToDirectory($ZipPath, $DestinationPath)
    Write-Output 'null'
    return
}
$stream = [IO.File]::OpenRead($ZipPath)
try {
    $archive = [IO.Compression.ZipArchive]::new($stream, [IO.Compression.ZipArchiveMode]::Read, $false)
    try {
        if ($Operation -eq 'List') {
            ConvertTo-Json -InputObject @($archive.Entries | ForEach-Object { $_.FullName }) -Compress
        } else {
            $reader = [IO.StreamReader]::new($archive.Entries[$EntryIndex].Open(), [Text.UTF8Encoding]::new($false, $true), $true)
            try { ConvertTo-Json -InputObject $reader.ReadToEnd() -Compress } finally { $reader.Dispose() }
        }
    } finally { $archive.Dispose() }
} finally { $stream.Dispose() }
