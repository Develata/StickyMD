[CmdletBinding()]
param(
    [switch]$Performance,
    [switch]$Runtime,
    [switch]$Resources,
    [switch]$Release,
    [switch]$Package,
    [switch]$Json,
    [string]$EvidenceFile
)

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $PSScriptRoot 'invoke-phase.ps1')
Invoke-StickyMdPhase -RepoRoot $repoRoot -Phase '11' -Parameters $PSBoundParameters
exit $LASTEXITCODE
