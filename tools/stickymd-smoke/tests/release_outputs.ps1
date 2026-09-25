# Runs after release_wrappers.ps1 in the same isolated host and fixture.
[Console]::OutputEncoding = [Text.Encoding]::GetEncoding(936)
$outputDirectory = $env:STICKYMD_TEST_OUTPUT_DIRECTORY
$outputRelative = Join-Path '..' ([IO.Path]::GetFileName($outputDirectory))
$fakeExe = Join-Path $fixture 'fixture.exe'
[IO.File]::WriteAllBytes($fakeExe, [byte[]](77,90,0,1))
$packaged = @(& (Join-Path $repo 'tools/release/package.ps1') -ExePath 'fixture.exe' -OutputDirectory $outputRelative -AllowDirtyValidation)
Assert-State
$outputZip = ($packaged | Where-Object { $_.StartsWith('PACKAGE_PATH=') }).Substring('PACKAGE_PATH='.Length)
$archive = [IO.Compression.ZipFile]::OpenRead($outputZip)
try {
    # Check the actual archive, not PowerShell function names or source-code tokens.
    foreach ($member in @('StickyMD/LICENSE.txt', 'StickyMD/licenses/SIL-OFL-1.1.txt', 'StickyMD/licenses/KaTeX-fonts-NOTICE.txt')) {
        $entry = @($archive.Entries | Where-Object FullName -ceq $member)
        if ($entry.Count -ne 1) { throw "Missing/duplicate packaged license: $member" }
        $stream = $entry[0].Open()
        $buffer = [IO.MemoryStream]::new()
        try { $stream.CopyTo($buffer); $bytes = $buffer.ToArray() } finally { $buffer.Dispose(); $stream.Dispose() }
        if ($bytes.Length -eq 0 -or $bytes[0] -eq 0xef -or $bytes -contains 13) { throw "License encoding drift: $member" }
        [void][Text.UTF8Encoding]::new($false, $true).GetString($bytes)
    }
} finally { $archive.Dispose() }
$outputSbom = Join-Path $outputDirectory 'SBOM.spdx.json'
$outputChecksums = Join-Path $outputDirectory 'SHA256SUMS.txt'
$fakeSyft = Join-Path $fixture 'fake-syft.ps1'
[IO.File]::WriteAllText($fakeSyft, @'
$destination = $args[-1].Substring('spdx-json='.Length)
[IO.File]::WriteAllText($destination, $env:STICKYMD_TEST_SBOM_TEXT, [Text.UTF8Encoding]::new($false))
if ($env:STICKYMD_TEST_SYFT_FAIL -eq 'yes') { exit 23 }
exit 0
'@)
function Invoke-TestSbom {
    & (Join-Path $repo 'tools/release/generate-sbom.ps1') -PackageDirectory $outputRelative -ZipPath (Join-Path $outputRelative ([IO.Path]::GetFileName($outputZip))) -OutputPath (Join-Path $outputRelative 'SBOM.spdx.json') -SyftPath 'fake-syft.ps1'
}
function Assert-OldOutputs {
    if ([IO.File]::ReadAllText($outputSbom) -cne 'previous-sbom' -or [IO.File]::ReadAllText($outputChecksums) -cne 'previous-checksums') {
        throw 'Failed SBOM generation modified previous outputs'
    }
    if ($env:SYFT_FILE_METADATA_SELECTION -cne 'fixture-selection' -or $env:SYFT_CHECK_FOR_APP_UPDATE -cne 'fixture-update') {
        throw 'SBOM wrapper changed caller Syft settings'
    }
}
$previousSelection = $env:SYFT_FILE_METADATA_SELECTION
$previousUpdate = $env:SYFT_CHECK_FOR_APP_UPDATE
try {
    $env:SYFT_FILE_METADATA_SELECTION = 'fixture-selection'
    $env:SYFT_CHECK_FOR_APP_UPDATE = 'fixture-update'
    [IO.File]::WriteAllText($outputSbom, 'previous-sbom')
    [IO.File]::WriteAllText($outputChecksums, 'previous-checksums')
    $env:STICKYMD_TEST_SBOM_TEXT = 'truncated output'
    $env:STICKYMD_TEST_SYFT_FAIL = 'yes'
    Assert-Fails 'Syft failed after writing output' { Invoke-TestSbom }
    Assert-OldOutputs
    $env:STICKYMD_TEST_SYFT_FAIL = 'no'
    foreach ($invalid in @('{}', '{"spdxVersion":"SPDX-3.0","packages":[{}],"files":[]}', '{"spdxVersion":"SPDX-2.3","packages":[],"files":[]}')) {
        $env:STICKYMD_TEST_SBOM_TEXT = $invalid
        Assert-Fails 'invalid generated SBOM' { Invoke-TestSbom }
        Assert-OldOutputs
    }
    $env:STICKYMD_TEST_SBOM_TEXT = $validSbomText
    $generated = @(Invoke-TestSbom)
    Assert-State
    if ($generated -notcontains "SBOM_PATH=$outputSbom" -or $generated -notcontains 'SYFT_VERSION=1.50.0') { throw 'SBOM output interface changed' }
    if ([IO.File]::ReadAllText($outputSbom) -cne $env:STICKYMD_TEST_SBOM_TEXT) { throw 'SBOM bytes changed during publication' }
    $expectedManifest = ((Get-FileHash -LiteralPath $outputZip).Hash.ToLowerInvariant() + ' *' + [IO.Path]::GetFileName($outputZip) + "`n" + (Get-FileHash -LiteralPath $outputSbom).Hash.ToLowerInvariant() + " *SBOM.spdx.json`n")
    if ([IO.File]::ReadAllText($outputChecksums) -cne $expectedManifest) { throw 'Checksum output drift' }
} finally {
    $env:SYFT_FILE_METADATA_SELECTION = $previousSelection
    $env:SYFT_CHECK_FOR_APP_UPDATE = $previousUpdate
    Remove-Item Env:STICKYMD_TEST_SBOM_TEXT -ErrorAction SilentlyContinue
    Remove-Item Env:STICKYMD_TEST_SYFT_FAIL -ErrorAction SilentlyContinue
}
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
'RELEASE_OUTPUTS=PASS'
