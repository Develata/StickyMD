[CmdletBinding()]
param(
    [string]$PackageDirectory,
    [string]$ZipPath,
    [string]$OutputPath,
    [string]$SyftPath
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$SyftVersion = '1.50.0'
$SyftArchiveSha256 = '815ee6973ec5dff6a671d7f41b0e78835a8c45b91d5a39f4743ea1cee833d3be'
$SyftChecksumsSha256 = 'bb8824a06c27c625fc103db5d7e9d7131ba2cc6e7c7a79318ee71686ede3c3f0'
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $PSScriptRoot 'package-path.ps1')
$workspaceManifest = Get-Content -LiteralPath (Join-Path $repoRoot 'Cargo.toml') -Raw
$versionMatch = [regex]::Match($workspaceManifest, '(?m)^version\s*=\s*"([^"]+)"\s*$')
if (-not $versionMatch.Success) { throw 'Cannot read workspace version from Cargo.toml' }
$workspaceVersion = $versionMatch.Groups[1].Value
if (-not $PackageDirectory) { $PackageDirectory = Join-Path $repoRoot 'dist' }
$PackageDirectory = [IO.Path]::GetFullPath($PackageDirectory)
if (-not $ZipPath) {
    $ZipPath = Resolve-StickyMdPackagePath -RepoRoot $repoRoot -PackageDirectory $PackageDirectory
}
$ZipPath = [IO.Path]::GetFullPath($ZipPath)
if (-not $OutputPath) { $OutputPath = Join-Path $PackageDirectory 'SBOM.spdx.json' }
$OutputPath = [IO.Path]::GetFullPath($OutputPath)

function Get-VerifiedCachedFile {
    param(
        [Parameter(Mandatory = $true)][string]$Uri,
        [Parameter(Mandatory = $true)][string]$CachePath,
        [Parameter(Mandatory = $true)][string]$ExpectedSha256,
        [Parameter(Mandatory = $true)][string]$Label
    )

    if (Test-Path -LiteralPath $CachePath -PathType Leaf) {
        $cachedHash = (Get-FileHash -LiteralPath $CachePath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($cachedHash -eq $ExpectedSha256) { return $CachePath }
    }

    $cacheDirectory = Split-Path -Parent $CachePath
    New-Item -ItemType Directory -Path $cacheDirectory -Force | Out-Null
    $lastFailure = 'download was not attempted'
    for ($attempt = 1; $attempt -le 3; $attempt++) {
        $partial = "$CachePath.partial-$PID-$attempt-$([guid]::NewGuid().ToString('N'))"
        try {
            Invoke-WebRequest -UseBasicParsing -Uri $Uri -OutFile $partial
            $actual = (Get-FileHash -LiteralPath $partial -Algorithm SHA256).Hash.ToLowerInvariant()
            if ($actual -ne $ExpectedSha256) {
                throw "$Label checksum mismatch after download: $actual"
            }
            Move-Item -LiteralPath $partial -Destination $CachePath -Force
            return $CachePath
        } catch {
            $lastFailure = $_.Exception.Message
        } finally {
            if (Test-Path -LiteralPath $partial) {
                Remove-Item -LiteralPath $partial -Force
            }
        }
        if ($attempt -lt 3) { Start-Sleep -Seconds $attempt }
    }
    throw "$Label download failed after 3 attempts: $lastFailure"
}

$temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) ("stickymd-sbom-" + [guid]::NewGuid().ToString('N'))
$context = Join-Path $temporaryRoot 'context'
New-Item -ItemType Directory -Path $context -Force | Out-Null
try {
    Copy-Item -LiteralPath (Join-Path $repoRoot 'Cargo.lock') -Destination (Join-Path $context 'Cargo.lock')
    Copy-Item -LiteralPath (Join-Path $repoRoot 'Cargo.toml') -Destination (Join-Path $context 'Cargo.toml')
    Expand-Archive -LiteralPath $ZipPath -DestinationPath (Join-Path $context 'package')

    if (-not $SyftPath) {
        $toolRoot = Join-Path $temporaryRoot 'syft'
        New-Item -ItemType Directory -Path $toolRoot | Out-Null
        $archiveName = "syft_${SyftVersion}_windows_amd64.zip"
        $cacheRoot = Join-Path $repoRoot "target/release-tools/syft/$SyftVersion"
        $checksumsName = "syft_${SyftVersion}_checksums.txt"
        $base = "https://github.com/anchore/syft/releases/download/v$SyftVersion"
        $archive = Get-VerifiedCachedFile `
            -Uri "$base/$archiveName" `
            -CachePath (Join-Path $cacheRoot $archiveName) `
            -ExpectedSha256 $SyftArchiveSha256 `
            -Label 'Syft archive'
        $checksums = Get-VerifiedCachedFile `
            -Uri "$base/$checksumsName" `
            -CachePath (Join-Path $cacheRoot $checksumsName) `
            -ExpectedSha256 $SyftChecksumsSha256 `
            -Label 'Syft checksum manifest'
        $actualArchive = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
        $actualChecksums = (Get-FileHash -LiteralPath $checksums -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actualArchive -ne $SyftArchiveSha256) { throw "Syft archive checksum mismatch: $actualArchive" }
        if ($actualChecksums -ne $SyftChecksumsSha256) { throw "Syft checksum manifest mismatch: $actualChecksums" }
        $officialLine = @(Get-Content -LiteralPath $checksums | Where-Object { $_ -match "\s+$([regex]::Escape($archiveName))$" })
        if ($officialLine.Count -ne 1 -or -not $officialLine[0].StartsWith($SyftArchiveSha256, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Pinned Syft archive hash is not present in the verified upstream checksum manifest'
        }
        Expand-Archive -LiteralPath $archive -DestinationPath $toolRoot
        $SyftPath = Join-Path $toolRoot 'syft.exe'
    }
    if (-not (Test-Path -LiteralPath $SyftPath -PathType Leaf)) { throw "Syft executable does not exist: $SyftPath" }

    $previousFileSelection = $env:SYFT_FILE_METADATA_SELECTION
    $previousUpdateCheck = $env:SYFT_CHECK_FOR_APP_UPDATE
    try {
        $env:SYFT_FILE_METADATA_SELECTION = 'all'
        $env:SYFT_CHECK_FOR_APP_UPDATE = 'false'
        & $SyftPath "dir:$context" '--source-name' 'StickyMD' '--source-version' $workspaceVersion '--output' "spdx-json=$OutputPath"
        if ($LASTEXITCODE -ne 0) { throw "Syft $SyftVersion failed with exit code $LASTEXITCODE" }
    } finally {
        $env:SYFT_FILE_METADATA_SELECTION = $previousFileSelection
        $env:SYFT_CHECK_FOR_APP_UPDATE = $previousUpdateCheck
    }
    $sbom = Get-Content -LiteralPath $OutputPath -Raw | ConvertFrom-Json
    if ($sbom.spdxVersion -notmatch '^SPDX-2\.') { throw 'Generated document is not an SPDX 2.x JSON SBOM' }
    if (@($sbom.packages).Count -eq 0) { throw 'Generated SBOM contains no packages' }
    $sbomFileNames = @($sbom.files | ForEach-Object { $_.fileName })
    foreach ($requiredFile in @(
        '\package\StickyMD\StickyMD.exe',
        '\package\StickyMD\THIRD_PARTY_NOTICES.txt',
        '\package\StickyMD\licenses\SIL-OFL-1.1.txt',
        '\package\StickyMD\licenses\KaTeX-fonts-NOTICE.txt'
    )) {
        if ($requiredFile -notin $sbomFileNames) { throw "Generated SBOM does not cover packaged file $requiredFile" }
    }

    $zipHash = (Get-FileHash -LiteralPath $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $sbomHash = (Get-FileHash -LiteralPath $OutputPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $checksumPath = Join-Path $PackageDirectory 'SHA256SUMS.txt'
    $lines = @(
        "$zipHash *$([IO.Path]::GetFileName($ZipPath))"
        "$sbomHash *$([IO.Path]::GetFileName($OutputPath))"
    )
    [IO.File]::WriteAllText($checksumPath, ($lines -join "`n") + "`n", [Text.UTF8Encoding]::new($false))
    Write-Output "SBOM_PATH=$OutputPath"
    Write-Output "SBOM_SHA256=$sbomHash"
    Write-Output "SYFT_VERSION=$SyftVersion"
} finally {
    $resolvedTemp = [IO.Path]::GetFullPath($temporaryRoot)
    $systemTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if (-not $resolvedTemp.StartsWith($systemTemp, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove unexpected temporary path: $resolvedTemp"
    }
    if (Test-Path -LiteralPath $resolvedTemp) { Remove-Item -LiteralPath $resolvedTemp -Recurse -Force }
}
