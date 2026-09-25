$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
function cargo {
    if (($args[0..4] -join ' ') -cne 'run --quiet -p stickymd-smoke --locked') {
        throw ('Unexpected Cargo invocation: ' + ($args -join '/'))
    }
    & $env:STICKYMD_TEST_EXE @($args[5..($args.Length - 1)])
}
$repo = $env:STICKYMD_TEST_ROOT
$fixture = $env:STICKYMD_TEST_DIRECTORY
Set-Location -LiteralPath $fixture
$location = (Get-Location).Path
[Console]::OutputEncoding = [Text.Encoding]::GetEncoding(936)
function Assert-State {
    if ([Console]::OutputEncoding.CodePage -ne 936 -or (Get-Location).Path -cne $location) {
        throw 'Wrapper changed the caller location or output encoding'
    }
}
function Assert-Fails([string]$label, [scriptblock]$action) {
    $failed = $false
    try { & $action | Out-Null } catch { $failed = $true }
    if (-not $failed) { throw "Expected rejection: $label" }
    if ($LASTEXITCODE -eq 0) { throw "Failure lost the native exit code: $label" }
    Assert-State
}
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$version = [regex]::Match([IO.File]::ReadAllText((Join-Path $repo 'Cargo.toml')), '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
$source = (& git -C $repo rev-parse HEAD).Trim()
$name = "StickyMD-$version-windows-x64-portable.zip"
$zip = Join-Path $fixture $name
$sbom = Join-Path $fixture 'SBOM.spdx.json'
$manifest = Join-Path $fixture 'SHA256SUMS.txt'
$validSbomText = '{"spdxVersion":"SPDX-2.3","packages":[{}],"files":[{"fileName":"\\package\\StickyMD\\StickyMD.exe"},{"fileName":"\\package\\StickyMD\\THIRD_PARTY_NOTICES.txt"},{"fileName":"\\package\\StickyMD\\licenses\\SIL-OFL-1.1.txt"},{"fileName":"\\package\\StickyMD\\licenses\\KaTeX-fonts-NOTICE.txt"}]}'
[IO.File]::WriteAllText($sbom, $validSbomText, [Text.UTF8Encoding]::new($false))
function New-Archive([string[]]$names, [string]$sourceText) {
    if ([IO.File]::Exists($zip)) { [IO.File]::Delete($zip) }
    $stream = [IO.File]::Open($zip, [IO.FileMode]::CreateNew)
    try {
        $archive = [IO.Compression.ZipArchive]::new($stream, [IO.Compression.ZipArchiveMode]::Create, $false)
        try {
            foreach ($entryName in $names) {
                $entry = $archive.CreateEntry($entryName)
                $writer = [IO.StreamWriter]::new($entry.Open(), [Text.UTF8Encoding]::new($false))
                try { $writer.Write($sourceText) } finally { $writer.Dispose() }
            }
        } finally { $archive.Dispose() }
    } finally { $stream.Dispose() }
    $script:zipHash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    $script:sbomHash = (Get-FileHash -LiteralPath $sbom -Algorithm SHA256).Hash.ToLowerInvariant()
    $script:checksumText = "$zipHash *$name`n$sbomHash *SBOM.spdx.json`n"
    [IO.File]::WriteAllText($manifest, $checksumText, [Text.UTF8Encoding]::new($false))
}
function Invoke-Promoted {
    & (Join-Path $repo 'tools/release/verify-promoted-artifact.ps1') -ArtifactDirectory '.' -SourceSha $source -ExpectedZipSha256 $zipHash -ExpectedSbomSha256 $sbomHash -ReleaseTag "v$version"
}
function Invoke-Package {
    & (Join-Path $repo 'tools/release/verify-package.ps1') -PackageDirectory '.' -ZipPath $name -ChecksumPath 'SHA256SUMS.txt'
}
New-Archive @('StickyMD/README.txt') "Source commit: $source`r`n"
$output = @(Invoke-Promoted)
if ($output -notcontains 'PROMOTION_INPUT=PASS' -or $output -notcontains "PROMOTION_ZIP=$zip" -or $output -notcontains "PROMOTION_ZIP_SHA256=$zipHash") {
    throw 'Promotion output did not preserve names and Unicode paths'
}
Assert-State
# A native intermediary does not get PowerShell's cross-edition environment cleanup.
# Shadow a built-in module to reproduce a foreign PSModulePath on any Windows host.
$foreignModules = Join-Path ([IO.Path]::GetTempPath()) ('stickymd-foreign-modules-' + [guid]::NewGuid().ToString('N'))
$foreignUtility = Join-Path $foreignModules 'Microsoft.PowerShell.Utility'
[void][IO.Directory]::CreateDirectory($foreignUtility)
[IO.File]::WriteAllText((Join-Path $foreignUtility 'Microsoft.PowerShell.Utility.psd1'), "@{ ModuleVersion = '99.0'; RootModule = 'foreign.psm1'; FunctionsToExport = @('Add-Type', 'ConvertTo-Json') }")
[IO.File]::WriteAllText((Join-Path $foreignUtility 'foreign.psm1'), "throw 'Foreign module path reached the Windows release adapter'")
$previousModulePath = $env:PSModulePath
try {
    $env:PSModulePath = $foreignModules
    $output = @(Invoke-Promoted)
    if ($output -notcontains 'PROMOTION_INPUT=PASS') { throw 'Foreign module path broke promotion verification' }
    if ($env:PSModulePath -cne $foreignModules) { throw 'Wrapper changed caller module path' }
    Assert-State
} finally {
    $env:PSModulePath = $previousModulePath
    $temporaryPrefix = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    $resolvedModules = [IO.Path]::GetFullPath($foreignModules)
    if (-not $resolvedModules.StartsWith($temporaryPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Unexpected foreign-module fixture directory'
    }
    Remove-Item -LiteralPath $resolvedModules -Recurse -Force
}
Assert-Fails 'promotion success does not imply valid package' { Invoke-Package }
$validSource = $source
$source = '0' * 40
Assert-Fails 'wrong source' { Invoke-Promoted }
$source = 'short'
Assert-Fails 'short source' { Invoke-Promoted }
$source = $validSource
$validHash = $zipHash
$zipHash = '0' * 64
Assert-Fails 'wrong expected hash' { Invoke-Promoted }
$zipHash = $validHash
[IO.File]::WriteAllText($manifest, $checksumText + "$sbomHash *sbom.spdx.json`n")
Assert-Fails 'duplicate checksum' { Invoke-Promoted }
[IO.File]::WriteAllText($manifest, "$zipHash *../$name`n$sbomHash *SBOM.spdx.json`n")
Assert-Fails 'unsafe checksum' { Invoke-Promoted }
[IO.File]::Delete($manifest)
Assert-Fails 'missing checksum' { Invoke-Promoted }
New-Archive @('StickyMD/README.txt') 'No source identity'
Assert-Fails 'missing README source' { Invoke-Promoted }
New-Archive @('StickyMD/README.txt', 'stickymd/readme.txt') "Source commit: $source`n"
Assert-Fails 'duplicate README' { Invoke-Promoted }
foreach ($bad in @('../escape', '/absolute', 'StickyMD\README.txt', 'StickyMD/note/note.md')) {
    New-Archive @($bad) "Source commit: $source`n"
    Assert-Fails 'unsafe or forbidden ZIP member' { Invoke-Package }
}
$destination = Join-Path $fixture '声明 with spaces.txt'
$output = @(& (Join-Path $repo 'tools/release/generate-third-party-notices.ps1') -DestinationPath '声明 with spaces.txt')
if ($output -notcontains "THIRD_PARTY_NOTICES=$destination") { throw 'Notice destination did not round trip' }
Assert-State
$before = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
Assert-Fails 'existing notice output' { & (Join-Path $repo 'tools/release/generate-third-party-notices.ps1') -DestinationPath '声明 with spaces.txt' }
if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $before) { throw 'Existing notice output was modified' }
Assert-Fails 'missing notice parent' { & (Join-Path $repo 'tools/release/generate-third-party-notices.ps1') -DestinationPath 'missing/output.txt' }
$bytes = [IO.File]::ReadAllBytes($destination)
if ($bytes[0] -eq 0xef -or $bytes -contains 13) { throw 'Notices are not UTF-8 without BOM and LF' }
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
'RELEASE_WRAPPERS=PASS'
