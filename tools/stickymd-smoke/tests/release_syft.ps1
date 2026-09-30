# Offline transport failures against the real pinned Rust validator, in a private cache.
[Console]::OutputEncoding = [Text.Encoding]::GetEncoding(936)
$syftRepo = Join-Path $outputDirectory 'Syft 中文 space'
[void][IO.Directory]::CreateDirectory($syftRepo)
foreach ($marker in @('Cargo.toml', 'AGENTS.md')) { [IO.File]::WriteAllText((Join-Path $syftRepo $marker), '') }
. (Join-Path $repo 'tools/release/invoke-smoke.ps1')
. (Join-Path $repo 'tools/release/syft-download.ps1')
$plan = (Invoke-StickyMdReleaseTool -RepoRoot $syftRepo -Arguments @('syft-plan')) | ConvertFrom-Json
Assert-State
if ($plan.external -or $plan.downloads.Count -ne 2 -or $plan.download_attempts -ne 3) { throw 'Missing Syft cache must require both pinned files' }
$entry = $plan.downloads[0]
[void][IO.Directory]::CreateDirectory((Split-Path -Parent $entry.path))
[IO.File]::WriteAllText($entry.path, 'previous-cache')
function Invoke-WebRequest {
    param([switch]$UseBasicParsing, [string]$Uri, [string]$OutFile)
    if ($Uri -cne $entry.uri) { throw 'Unexpected download URI' }
    $script:syftDownloadCount++
    [IO.File]::WriteAllText($OutFile, 'incomplete or corrupt download')
    if ($script:syftTransportFailure) { throw 'simulated interrupted transport' }
}
function Start-Sleep { param([int]$Seconds) $script:syftBackoffs += $Seconds }
try {
    foreach ($transportFailure in @($false, $true)) {
        $script:syftTransportFailure = $transportFailure
        $script:syftDownloadCount = 0
        $script:syftBackoffs = @()
        $failed = $false
        try { Get-PinnedSyftFile -RepoRoot $syftRepo -Entry $entry -Attempts $plan.download_attempts } catch {
            if ($_.Exception.Message -notlike '*download failed after 3 attempts:*') { throw }
            $failed = $true
        }
        if (-not $failed -or $syftDownloadCount -ne 3 -or ($syftBackoffs -join ',') -cne '1,2') { throw 'Syft retry boundary changed' }
        if ([IO.File]::ReadAllText($entry.path) -cne 'previous-cache') { throw 'Rejected download replaced previous cache' }
        if (@(Get-ChildItem -LiteralPath (Split-Path -Parent $entry.path) -Filter '*.partial-*').Count -ne 0) { throw 'Syft partial download was not cleaned' }
        Assert-State
    }
} finally {
    Remove-Item Function:Invoke-WebRequest
    Remove-Item Function:Start-Sleep
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
