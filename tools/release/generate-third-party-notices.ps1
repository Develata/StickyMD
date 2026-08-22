[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$DestinationPath
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$DestinationPath = [IO.Path]::GetFullPath($DestinationPath)
$metadataJson = & cargo metadata --format-version 1 --locked --filter-platform x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed while generating runtime notices' }
$convertFromJson = Get-Command ConvertFrom-Json -ErrorAction Stop
if ($convertFromJson.Parameters.ContainsKey('Depth')) {
    $metadata = $metadataJson | ConvertFrom-Json -Depth 100
} else {
    # Windows PowerShell 5.1 has no -Depth parameter. Its parser still handles
    # Cargo's metadata graph, while PowerShell 7 receives the explicit bound.
    $metadata = $metadataJson | ConvertFrom-Json
}
$rootPackage = @($metadata.packages | Where-Object { $_.name -eq 'stickymd-win' })
if ($rootPackage.Count -ne 1) { throw 'Cannot identify the stickymd-win package in cargo metadata' }

$nodes = @{}
foreach ($node in $metadata.resolve.nodes) { $nodes[$node.id] = $node }
$visited = [Collections.Generic.HashSet[string]]::new()
$pending = [Collections.Generic.Queue[string]]::new()
$pending.Enqueue($rootPackage[0].id)
while ($pending.Count -ne 0) {
    $packageId = $pending.Dequeue()
    if (-not $visited.Add($packageId)) { continue }
    foreach ($dependency in $nodes[$packageId].deps) {
        $isRuntime = @($dependency.dep_kinds | Where-Object { $null -eq $_.kind }).Count -ne 0
        if ($isRuntime) { $pending.Enqueue($dependency.pkg) }
    }
}

$thirdPartyPackages = [Collections.Generic.List[object]]::new()
foreach ($package in $metadata.packages) {
    if ($visited.Contains($package.id) -and $null -ne $package.source) {
        $thirdPartyPackages.Add($package)
    }
}
$packageComparison = [Comparison[object]] {
    param($left, $right)
    $nameOrder = [StringComparer]::Ordinal.Compare([string]$left.name, [string]$right.name)
    if ($nameOrder -ne 0) { return $nameOrder }
    return [StringComparer]::Ordinal.Compare([string]$left.version, [string]$right.version)
}
$thirdPartyPackages.Sort($packageComparison)
$unsupportedSources = @(
    $thirdPartyPackages |
        Where-Object { -not $_.source.StartsWith('registry+', [StringComparison]::Ordinal) }
)
if ($unsupportedSources.Count -ne 0) {
    $summary = @($unsupportedSources | ForEach-Object { "$($_.name) $($_.version): $($_.source)" }) -join '; '
    throw "Runtime dependency notice generation does not support non-registry sources: $summary"
}
$packages = $thirdPartyPackages
if ($packages.Count -eq 0) { throw 'The Windows runtime dependency graph is empty' }

$fallbackFiles = @{
    'clipboard-win' = 'assets\licenses\Boost-1.0.txt'
    'harfrust' = 'assets\licenses\HarfRust-MIT.txt'
    'ratex-font' = 'assets\licenses\RaTeX-MIT.txt'
    'ratex-font-loader' = 'assets\licenses\RaTeX-MIT.txt'
    'ratex-katex-fonts' = 'assets\licenses\RaTeX-MIT.txt'
    'ratex-layout' = 'assets\licenses\RaTeX-MIT.txt'
    'ratex-lexer' = 'assets\licenses\RaTeX-MIT.txt'
    'ratex-parser' = 'assets\licenses\RaTeX-MIT.txt'
    'ratex-types' = 'assets\licenses\RaTeX-MIT.txt'
    'ratex-unicode-font' = 'assets\licenses\RaTeX-MIT.txt'
}

$utf8 = [Text.UTF8Encoding]::new($false, $true)
$builder = [Text.StringBuilder]::new()
[void]$builder.AppendLine(([IO.File]::ReadAllText((Join-Path $repoRoot 'THIRD_PARTY_NOTICES.md'), $utf8)).TrimEnd())
[void]$builder.AppendLine()
[void]$builder.AppendLine('## Generated Rust Runtime Dependency Notices')
[void]$builder.AppendLine()
[void]$builder.AppendLine('This section is generated from the Cargo.lock-resolved normal dependency graph for')
[void]$builder.AppendLine('stickymd-win on x86_64-pc-windows-msvc. Build-only and development-only packages are excluded.')
[void]$builder.AppendLine("Cargo.lock SHA-256: $((Get-FileHash -LiteralPath (Join-Path $repoRoot 'Cargo.lock') -Algorithm SHA256).Hash.ToLowerInvariant())")
[void]$builder.AppendLine("Runtime registry packages: $($packages.Count)")

foreach ($package in $packages) {
    $repository = if ($package.repository) {
        [string]$package.repository
    } elseif ($package.homepage) {
        [string]$package.homepage
    } else {
        "https://crates.io/crates/$($package.name)/$($package.version)"
    }
    [void]$builder.AppendLine()
    [void]$builder.AppendLine('================================================================================')
    [void]$builder.AppendLine("PACKAGE: $($package.name) $($package.version)")
    [void]$builder.AppendLine("DECLARED LICENSE: $($package.license)")
    [void]$builder.AppendLine("SOURCE: $repository")

    $packageDirectory = Split-Path -Parent $package.manifest_path
    $licenseFiles = [Collections.Generic.List[IO.FileInfo]]::new()
    foreach ($candidate in Get-ChildItem -LiteralPath $packageDirectory -File) {
        if ($candidate.Name -match '^(?i)(LICENSE|COPYING|NOTICE)([._-].*)?$') {
            $licenseFiles.Add($candidate)
        }
    }
    $licenseComparison = [Comparison[IO.FileInfo]] {
        param($left, $right)
        return [StringComparer]::Ordinal.Compare($left.Name, $right.Name)
    }
    $licenseFiles.Sort($licenseComparison)
    if ($licenseFiles.Count -eq 0) {
        if (-not $fallbackFiles.ContainsKey($package.name)) {
            throw "Runtime package $($package.name) $($package.version) contains no license notice and has no reviewed fallback"
        }
        $fallback = Join-Path $repoRoot $fallbackFiles[$package.name]
        if (-not (Test-Path -LiteralPath $fallback -PathType Leaf)) {
            throw "Reviewed license fallback is missing for $($package.name): $fallback"
        }
        $licenseFiles = @((Get-Item -LiteralPath $fallback))
    }

    foreach ($licenseFile in $licenseFiles) {
        [void]$builder.AppendLine("LICENSE FILE: $($licenseFile.Name)")
        [void]$builder.AppendLine('--------------------------------------------------------------------------------')
        $text = ([IO.File]::ReadAllText($licenseFile.FullName, $utf8)).Replace("`r`n", "`n").TrimEnd()
        [void]$builder.AppendLine($text)
    }
}

$parent = Split-Path -Parent $DestinationPath
if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
    throw "Notice destination directory does not exist: $parent"
}
$stream = [IO.File]::Open($DestinationPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
try {
    $writer = [IO.StreamWriter]::new($stream, [Text.UTF8Encoding]::new($false))
    try { $writer.Write($builder.ToString().Replace("`r`n", "`n")) } finally { $writer.Dispose() }
} finally {
    $stream.Dispose()
}

Write-Output "THIRD_PARTY_NOTICES=$DestinationPath"
Write-Output "RUNTIME_DEPENDENCY_COUNT=$($packages.Count)"
