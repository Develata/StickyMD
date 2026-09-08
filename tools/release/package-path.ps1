function Resolve-StickyMdPackagePath {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$PackageDirectory
    )

    $packageRoot = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($PackageDirectory)
    $previousEncoding = [Console]::OutputEncoding
    Push-Location -LiteralPath $RepoRoot
    try {
        # Rust writes UTF-8; Windows PowerShell otherwise decodes using the console code page.
        [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
        $packagePath = & cargo run --quiet -p stickymd-smoke --locked -- package-path --directory $packageRoot
        if ($LASTEXITCODE -ne 0) { throw 'Rust portable package selection failed' }
        return $packagePath
    } finally {
        try {
            [Console]::OutputEncoding = $previousEncoding
        } finally {
            Pop-Location
        }
    }
}
