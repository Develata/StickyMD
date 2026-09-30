[CmdletBinding()]
param(
    [switch]$Performance
)

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $PSScriptRoot 'invoke-phase.ps1')
Invoke-StickyMdPhase -RepoRoot $repoRoot -Phase '01' -Parameters $PSBoundParameters
exit $LASTEXITCODE
