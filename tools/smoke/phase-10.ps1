# 2026-10-22: Preview resource/overflow maintenance uses the existing Rust-owned Phase 10 tests.
[CmdletBinding()]
param(
    [switch]$Performance,
    [switch]$Runtime,
    [switch]$Resources,
    [string]$ResourceModule,
    [switch]$Release,
    [switch]$Package,
    [switch]$Json
)

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $PSScriptRoot 'invoke-phase.ps1')
Invoke-StickyMdPhase -RepoRoot $repoRoot -Phase '10' -Parameters $PSBoundParameters
exit $LASTEXITCODE
