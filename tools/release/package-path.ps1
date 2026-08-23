function Resolve-StickyMdPackagePath {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$PackageDirectory
    )

    $packages = @(Get-ChildItem -LiteralPath $PackageDirectory -Filter 'StickyMD-*-windows-x64-portable.zip' -File)
    if ($packages.Count -eq 1) { return $packages[0].FullName }

    $workspaceManifest = Get-Content -LiteralPath (Join-Path $RepoRoot 'Cargo.toml') -Raw
    $versionMatch = [regex]::Match($workspaceManifest, '(?m)^version\s*=\s*"([^"]+)"\s*$')
    if (-not $versionMatch.Success) { throw 'Cannot read workspace version from Cargo.toml' }
    $commitSha = (& git -C $RepoRoot rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $commitSha -notmatch '^[0-9a-fA-F]{40}$') {
        throw 'Cannot resolve current Git commit while selecting the portable package'
    }
    $dirty = [bool](& git -C $RepoRoot status --porcelain)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect Git state while selecting the portable package' }
    $qualifier = if ($dirty) {
        "local-validation-$($commitSha.Substring(0, 12).ToLowerInvariant())-dirty"
    } else {
        "local-rc-$($commitSha.Substring(0, 12).ToLowerInvariant())"
    }
    $expectedName = "StickyMD-$($versionMatch.Groups[1].Value)-$qualifier-windows-x64-portable.zip"
    $matches = @($packages | Where-Object { $_.Name -ceq $expectedName })
    if ($matches.Count -eq 1) { return $matches[0].FullName }
    throw "Cannot select current portable ZIP in $PackageDirectory; expected $expectedName among $($packages.Count) package(s)"
}
