# Download transport only. Identity, cache selection and atomic publication belong to Rust.
function Get-PinnedSyftFile {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)]$Entry,
        [Parameter(Mandatory = $true)][int]$Attempts
    )

    $cacheDirectory = Split-Path -Parent $Entry.path
    New-Item -ItemType Directory -Path $cacheDirectory -Force | Out-Null
    $lastFailure = 'download was not attempted'
    for ($attempt = 1; $attempt -le $Attempts; $attempt++) {
        $partial = "$($Entry.path).partial-$PID-$attempt-$([guid]::NewGuid().ToString('N'))"
        try {
            Invoke-WebRequest -UseBasicParsing -Uri $Entry.uri -OutFile $partial
            Invoke-StickyMdReleaseTool -RepoRoot $RepoRoot -Arguments @(
                'syft-publish', '--kind', $Entry.kind, '--input', $partial
            ) | Out-Null
            return
        } catch {
            $lastFailure = $_.Exception.Message
        } finally {
            if (Test-Path -LiteralPath $partial) { Remove-Item -LiteralPath $partial -Force }
        }
        if ($attempt -lt $Attempts) { Start-Sleep -Seconds $attempt }
    }
    throw "$($Entry.label) download failed after $Attempts attempts: $lastFailure"
}
