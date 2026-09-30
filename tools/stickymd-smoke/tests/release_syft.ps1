# Offline transport failures against the real pinned Rust validator, in a private cache.
[Console]::OutputEncoding = [Text.Encoding]::GetEncoding(936)
$syftRepo = Join-Path $outputDirectory 'Syft 中文 space'
[void][IO.Directory]::CreateDirectory($syftRepo)
foreach ($marker in @('Cargo.toml', 'AGENTS.md')) { [IO.File]::WriteAllText((Join-Path $syftRepo $marker), '') }
. (Join-Path $repo 'tools/release/invoke-smoke.ps1')
$plan = (Invoke-StickyMdReleaseTool -RepoRoot $syftRepo -Arguments @('syft-plan')) | ConvertFrom-Json
Assert-State
if ($plan.external -or $plan.downloads.Count -ne 2 -or $plan.download_attempts -ne 3) { throw 'Missing Syft cache must require both pinned files' }
$entry = $plan.downloads[0]
[void][IO.Directory]::CreateDirectory((Split-Path -Parent $entry.path))
[IO.File]::WriteAllText($entry.path, 'previous-cache')
function Invoke-WebRequest {
    param([switch]$UseBasicParsing, [string]$Uri, [string]$OutFile)
    if ($Uri -cne $entry.uri) { throw 'Unexpected download URI' }
    $global:syftDownloadCount++
    [IO.File]::WriteAllText($OutFile, 'incomplete or corrupt download')
    if ($global:syftTransportFailure) { throw 'simulated interrupted transport' }
}
try {
    foreach ($transportFailure in @($false, $true)) {
        $global:syftTransportFailure = $transportFailure
        $global:syftDownloadCount = 0
        $failed = $false
        $partial = Join-Path $syftRepo 'private-download.partial'
        try {
            & (Join-Path $repo 'tools/release/syft-fetch.ps1') -Uri $entry.uri -OutputPath $partial
            Invoke-StickyMdReleaseTool -RepoRoot $syftRepo -Arguments @('syft-publish', '--kind', $entry.kind, '--input', $partial)
        } catch {
            $failed = $true
        } finally {
            if (Test-Path -LiteralPath $partial) { Remove-Item -LiteralPath $partial }
        }
        # Retry/backoff/partial cleanup now run in sbom_workflow Rust behavior tests.
        if (-not $failed -or $syftDownloadCount -ne 1) { throw 'Syft transport/validation boundary changed' }
        if ([IO.File]::ReadAllText($entry.path) -cne 'previous-cache') { throw 'Rejected download replaced previous cache' }
        if (@(Get-ChildItem -LiteralPath (Split-Path -Parent $entry.path) -Filter '*.partial-*').Count -ne 0) { throw 'Syft partial download was not cleaned' }
        [Console]::OutputEncoding = [Text.Encoding]::GetEncoding(936)
        Assert-State
    }
} finally {
    Remove-Item Function:Invoke-WebRequest
}
$planAfter = (Invoke-StickyMdReleaseTool -RepoRoot $syftRepo -Arguments @('syft-plan')) | ConvertFrom-Json
if ($planAfter.downloads.Count -ne 2) { throw 'Corrupt cache was accepted as a hit' }
$external = (Invoke-StickyMdReleaseTool -RepoRoot $syftRepo -Arguments @('syft-plan', '--syft-path', $fakeSyft)) | ConvertFrom-Json
if (-not $external.external -or $external.downloads.Count -ne 0) { throw 'Provided Syft path lost its existing bypass behavior' }
Assert-Fails 'missing provided Syft' { Invoke-StickyMdReleaseTool -RepoRoot $syftRepo -Arguments @('syft-plan', '--syft-path', (Join-Path $syftRepo 'missing.exe')) }
$verifiedDirectory = Join-Path $syftRepo 'unverified snapshot'
Assert-Fails 'corrupt Syft snapshot' { Invoke-StickyMdReleaseTool -RepoRoot $syftRepo -Arguments @('syft-verify', '--archive', $entry.path, '--checksums', $entry.path, '--staging-directory', $verifiedDirectory) }
if (Test-Path -LiteralPath $verifiedDirectory) { throw 'Failed Syft verification left a usable snapshot' }
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
'RELEASE_SYFT=PASS'
