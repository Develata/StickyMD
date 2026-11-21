function Invoke-StickyMdPackageTool {
    # The packaging crate is the only producer of release bytes (plan 11 #platform-artifact-reuse);
    # scripts that build bytes call it, never the verification tool.
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )
    $previousEncoding = [Console]::OutputEncoding
    Push-Location -LiteralPath $RepoRoot
    try {
        [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
        & cargo run --quiet -p stickymd-package --locked -- @Arguments
        if ($LASTEXITCODE -ne 0) { throw 'Rust packaging tool failed' }
    } finally {
        try { [Console]::OutputEncoding = $previousEncoding } finally { Pop-Location }
    }
}
