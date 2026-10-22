# Windowless StickyApp state-application tests run through the existing Rust phase plan.
# Native key/IME delivery and Preview capture outside the window remain desktop obligations.
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
