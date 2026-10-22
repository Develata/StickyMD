# 2026-10-22: Preview resource/overflow maintenance uses the existing Rust-owned Phase 06 tests.
# Includes overwide display-math scrolling, fixed backgrounds and source-copy regression.
[CmdletBinding()]
param(
    [switch]$Performance,
    [switch]$Runtime,
    [switch]$Resources
)

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $PSScriptRoot 'invoke-phase.ps1')
Invoke-StickyMdPhase -RepoRoot $repoRoot -Phase '06' -Parameters $PSBoundParameters
exit $LASTEXITCODE
