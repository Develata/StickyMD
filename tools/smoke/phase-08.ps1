# Windowless StickyApp state-application tests run through the existing Rust phase plan.
# Native key/IME delivery and Preview capture use opt-in Rust G4-02/G4-06 cases.
# Local preflights keep manual rows NOT TESTED and do not qualify a candidate.
[CmdletBinding()]
param(
    [switch]$Performance,
    [switch]$Runtime,
    [switch]$Resources
)

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $PSScriptRoot 'invoke-phase.ps1')
Invoke-StickyMdPhase -RepoRoot $repoRoot -Phase '08' -Parameters $PSBoundParameters
exit $LASTEXITCODE
