function Invoke-StickyMdReleaseTool {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )
    $previousEncoding = [Console]::OutputEncoding
    Push-Location -LiteralPath $RepoRoot
    try {
        [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
        & cargo run --quiet -p stickymd-smoke --locked -- release @Arguments
        if ($LASTEXITCODE -ne 0) { throw 'Rust release tool failed' }
    } finally {
        try { [Console]::OutputEncoding = $previousEncoding } finally { Pop-Location }
    }
}
