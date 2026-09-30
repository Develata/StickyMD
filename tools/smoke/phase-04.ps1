[CmdletBinding()]
param(
    [switch]$Performance,
    [switch]$Runtime
)

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $PSScriptRoot 'invoke-phase.ps1')
Invoke-StickyMdPhase -RepoRoot $repoRoot -Phase '04' -Parameters $PSBoundParameters
exit $LASTEXITCODE
