[CmdletBinding()]
param(
    [string]$PackageDirectory,
    [string]$ZipPath,
    [string]$ChecksumPath,
    [switch]$Runtime
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
if (-not $PackageDirectory) { $PackageDirectory = Join-Path $repoRoot 'dist' }
$PackageDirectory = [IO.Path]::GetFullPath($PackageDirectory)
if (-not $ZipPath) {
    $packages = @(Get-ChildItem -LiteralPath $PackageDirectory -Filter 'StickyMD-*-windows-x64-portable.zip' -File)
    if ($packages.Count -ne 1) { throw "Expected exactly one portable ZIP in $PackageDirectory; found $($packages.Count)" }
    $ZipPath = $packages[0].FullName
}
$ZipPath = [IO.Path]::GetFullPath($ZipPath)
if (-not $ChecksumPath) { $ChecksumPath = Join-Path $PackageDirectory 'SHA256SUMS.txt' }

$allowed = @(
    'StickyMD/StickyMD.exe',
    'StickyMD/README.txt',
    'StickyMD/LICENSE.txt',
    'StickyMD/THIRD_PARTY_NOTICES.txt',
    'StickyMD/licenses/SIL-OFL-1.1.txt',
    'StickyMD/licenses/KaTeX-fonts-NOTICE.txt'
)
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$stream = [IO.File]::OpenRead($ZipPath)
try {
    $archive = [IO.Compression.ZipArchive]::new($stream, [IO.Compression.ZipArchiveMode]::Read, $false)
    try {
        $names = @($archive.Entries | ForEach-Object { $_.FullName })
        if ($names.Count -ne (@($names | Sort-Object -Unique)).Count) { throw 'Portable ZIP contains duplicate paths' }
        foreach ($name in $names) {
            if ($name.Contains('\') -or $name.StartsWith('/') -or $name.Contains(':')) { throw "Unsafe ZIP path: $name" }
            if (@($name.Split('/') | Where-Object { $_ -eq '.' -or $_ -eq '..' }).Count -ne 0) { throw "Unsafe ZIP path: $name" }
        }
        $unexpected = @($names | Where-Object { $_ -notin $allowed })
        $missing = @($allowed | Where-Object { $_ -notin $names })
        if ($unexpected.Count -ne 0 -or $missing.Count -ne 0) {
            throw "Portable allowlist mismatch; unexpected=[$($unexpected -join ', ')], missing=[$($missing -join ', ')]"
        }
    } finally { $archive.Dispose() }
} finally { $stream.Dispose() }

if ((Get-Item -LiteralPath $ZipPath).Length -gt 30MB) { throw 'Portable ZIP exceeds the 30 MiB hard gate' }
if (-not (Test-Path -LiteralPath $ChecksumPath -PathType Leaf)) { throw "Checksum manifest missing: $ChecksumPath" }
foreach ($line in Get-Content -LiteralPath $ChecksumPath) {
    if (-not $line.Trim()) { continue }
    if ($line -notmatch '^([0-9a-fA-F]{64}) \*(.+)$') { throw "Invalid checksum line: $line" }
    $artifact = Join-Path $PackageDirectory $Matches[2]
    if (-not (Test-Path -LiteralPath $artifact -PathType Leaf)) { throw "Checksummed artifact missing: $artifact" }
    $actual = (Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash
    if ($actual -ne $Matches[1]) { throw "Checksum mismatch for $artifact" }
}

$temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) ("stickymd-verify-" + [guid]::NewGuid().ToString('N'))
$runtimeProcesses = [Collections.Generic.List[Diagnostics.Process]]::new()
try {
    Expand-Archive -LiteralPath $ZipPath -DestinationPath $temporaryRoot
    $exe = Join-Path $temporaryRoot 'StickyMD\StickyMD.exe'
    $bytes = [IO.File]::ReadAllBytes($exe)
    if ($bytes.Length -lt 0x40 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) { throw 'StickyMD.exe is not a PE image' }
    $pe = [BitConverter]::ToInt32($bytes, 0x3C)
    if ($pe -lt 0 -or $pe + 26 -ge $bytes.Length) { throw 'StickyMD.exe has an invalid PE header offset' }
    if ([Text.Encoding]::ASCII.GetString($bytes, $pe, 4) -ne "PE`0`0") { throw 'StickyMD.exe has no PE signature' }
    if ([BitConverter]::ToUInt16($bytes, $pe + 4) -ne 0x8664) { throw 'StickyMD.exe is not x86_64' }
    if ([BitConverter]::ToUInt16($bytes, $pe + 24) -ne 0x020B) { throw 'StickyMD.exe is not PE32+' }
    $ascii = [Text.Encoding]::ASCII.GetString($bytes)
    if (-not $ascii.Contains('PerMonitorV2')) { throw 'Embedded manifest lacks PerMonitorV2' }
    if (-not $ascii.Contains('asInvoker')) { throw 'Embedded manifest lacks asInvoker' }
    $version = [Diagnostics.FileVersionInfo]::GetVersionInfo($exe)
    if ($version.ProductName -ne 'StickyMD' -or -not $version.FileVersion) { throw 'StickyMD.exe lacks the required product/version resource' }

    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class StickyMdIconProbe {
    [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
    public static extern uint ExtractIconEx(string file, int index, IntPtr[] large, IntPtr[] small, uint count);
    [DllImport("user32.dll")]
    public static extern bool DestroyIcon(IntPtr icon);
}
'@
    $large = [IntPtr[]]::new(1)
    $small = [IntPtr[]]::new(1)
    if ([StickyMdIconProbe]::ExtractIconEx($exe, 0, $large, $small, 1) -eq 0) { throw 'StickyMD.exe lacks an application icon resource' }
    foreach ($handle in @($large[0], $small[0])) { if ($handle -ne [IntPtr]::Zero) { [void][StickyMdIconProbe]::DestroyIcon($handle) } }

    if ($Runtime) {
        $runtimeRoot = Join-Path $temporaryRoot 'runtime'
        $programDirectories = @(
            (Join-Path $runtimeRoot 'ascii'),
            (Join-Path $runtimeRoot 'with space'),
            (Join-Path $runtimeRoot '中文便签')
        )
        foreach ($programDirectory in $programDirectories) {
            New-Item -ItemType Directory -Path $programDirectory -Force | Out-Null
            $runtimeExe = Join-Path $programDirectory 'StickyMD.exe'
            Copy-Item -LiteralPath $exe -Destination $runtimeExe
            $process = Start-Process -FilePath $runtimeExe -WorkingDirectory $programDirectory -PassThru
            $runtimeProcesses.Add($process)
            $deadline = [DateTime]::UtcNow.AddSeconds(10)
            $note = Join-Path $programDirectory 'note\note.md'
            $config = Join-Path $programDirectory 'note\config.toml'
            while ((-not (Test-Path -LiteralPath $note -PathType Leaf) -or -not (Test-Path -LiteralPath $config -PathType Leaf)) -and [DateTime]::UtcNow -lt $deadline) {
                if ($process.HasExited) { throw "Packaged runtime exited early in $programDirectory" }
                Start-Sleep -Milliseconds 50
            }
            if (-not (Test-Path -LiteralPath $note -PathType Leaf) -or -not (Test-Path -LiteralPath $config -PathType Leaf)) {
                throw "Packaged runtime did not bootstrap portable files in $programDirectory"
            }
        }

        $primaryDirectory = $programDirectories[0]
        $primaryExe = Join-Path $primaryDirectory 'StickyMD.exe'
        $primaryNote = Join-Path $primaryDirectory 'note\note.md'
        $primaryConfig = Join-Path $primaryDirectory 'note\config.toml'
        $before = @(
            (Get-FileHash -LiteralPath $primaryNote -Algorithm SHA256).Hash,
            (Get-Item -LiteralPath $primaryNote).LastWriteTimeUtc.Ticks,
            (Get-FileHash -LiteralPath $primaryConfig -Algorithm SHA256).Hash,
            (Get-Item -LiteralPath $primaryConfig).LastWriteTimeUtc.Ticks
        )
        $secondary = Start-Process -FilePath $primaryExe -WorkingDirectory $primaryDirectory -PassThru
        if (-not $secondary.WaitForExit(5000)) {
            Stop-Process -Id $secondary.Id -Force -ErrorAction SilentlyContinue
            throw 'Same-directory packaged secondary instance did not exit'
        }
        if ($secondary.ExitCode -ne 0) { throw "Same-directory packaged secondary exited with $($secondary.ExitCode)" }
        $after = @(
            (Get-FileHash -LiteralPath $primaryNote -Algorithm SHA256).Hash,
            (Get-Item -LiteralPath $primaryNote).LastWriteTimeUtc.Ticks,
            (Get-FileHash -LiteralPath $primaryConfig -Algorithm SHA256).Hash,
            (Get-Item -LiteralPath $primaryConfig).LastWriteTimeUtc.Ticks
        )
        if (($before -join '|') -ne ($after -join '|')) { throw 'Same-directory packaged secondary modified durable files' }
        foreach ($process in $runtimeProcesses) {
            if ($process.HasExited) { throw "Different-directory packaged instance exited early: $($process.Id)" }
        }
        Write-Output 'PACKAGE_RUNTIME=PASS (ASCII, space, Chinese, same-directory and different-directory)'
    }
} finally {
    foreach ($process in $runtimeProcesses) {
        if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
        $process.Dispose()
    }
    $resolvedTemp = [IO.Path]::GetFullPath($temporaryRoot)
    $systemTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if (-not $resolvedTemp.StartsWith($systemTemp, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove unexpected temporary path: $resolvedTemp"
    }
    if (Test-Path -LiteralPath $resolvedTemp) { Remove-Item -LiteralPath $resolvedTemp -Recurse -Force }
}

Write-Output "PACKAGE_VERIFY=PASS"
Write-Output "PACKAGE_PATH=$ZipPath"
