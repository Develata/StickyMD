$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
function cargo {
    if (($args[0..4] -join ' ') -cne 'run -p stickymd-smoke --locked --') { throw 'Unexpected phase bootstrap' }
    $forwarded = @($args[5..($args.Length - 1)])
    if ($env:STICKYMD_PHASE_BASELINE -and $forwarded[0] -cne 'phase-entry') {
        $global:LASTEXITCODE = 0
        @{schema_version=1; status='NOT_RUN'; arguments=$forwarded} | ConvertTo-Json -Compress
    } else {
        if ($forwarded[0] -cne 'phase-entry') { throw 'Phase wrapper bypassed the Rust router' }
        # Observe the real router and canonical parser without executing qualification/GUI actions.
        $forwarded[0] = 'phase-entry-plan'
        & $env:STICKYMD_TEST_EXE @forwarded
    }
}
Set-Location -LiteralPath $env:STICKYMD_TEST_DIRECTORY
$location = (Get-Location).Path
[Console]::OutputEncoding = [Text.Encoding]::GetEncoding(936)
function Invoke-PhaseCase([string]$Phase, [hashtable]$Parameters, [string[]]$Expected) {
    $name = if ($Phase -eq 'all') { 'all.ps1' } else { "phase-$Phase.ps1" }
    $script = Join-Path $env:STICKYMD_TEST_ROOT "tools/smoke/$name"
    if ($env:STICKYMD_PHASE_BASELINE) { $script = Join-Path $env:STICKYMD_PHASE_BASELINE "baseline-$name" }
    # Baseline copies use their original script root so that command working directories match.
    if ($env:STICKYMD_PHASE_BASELINE) {
        $source = [IO.File]::ReadAllText($script).Replace('$PSScriptRoot', "'$($env:STICKYMD_TEST_ROOT.Replace("'", "''"))/tools/smoke'")
        $script = Join-Path $location "baseline-$Phase.ps1"
        [IO.File]::WriteAllText($script, $source)
    }
    if ($env:STICKYMD_PHASE_BASELINE) { Set-StrictMode -Off }
    try { $result = @(& $script @Parameters) } finally { Set-StrictMode -Version Latest }
    if ($LASTEXITCODE -ne 0) { throw "Phase $Phase rejected compatible input" }
    if ((Get-Location).Path -cne $location -or [Console]::OutputEncoding.CodePage -ne 936) { throw 'Phase wrapper changed caller state' }
    if ($result.Count -ne 1) { throw 'Routing must return one JSON document' }
    $plan = $result[0] | ConvertFrom-Json
    if ($plan.status -cne 'NOT_RUN' -or ($plan.arguments -join '|') -cne ($Expected -join '|')) { throw "Routing drift: $($result[0])" }
}
Invoke-PhaseCase '00' @{Verbose=$true} @('phase', '00')
foreach ($phase in @('01', '02', '03', '04', '05', '06', '07', '08', '09', '10', '11', '11-b')) {
    Invoke-PhaseCase $phase @{Performance=$true} @('phase', $phase, '--performance')
    Invoke-PhaseCase $phase @{Performance=$false; Verbose=$true} @('phase', $phase)
}
foreach ($phase in @('03', '04', '05', '06', '07', '08', '09', '10', '11', '11-b')) {
    Invoke-PhaseCase $phase @{Runtime=$true} @('phase', $phase, '--runtime')
}
foreach ($phase in @('05', '06', '07', '08', '09', '10', '11', '11-b')) {
    Invoke-PhaseCase $phase @{Resources=$true} @('phase', $phase, '--resources')
}
foreach ($phase in @('09', '10', '11', '11-b')) {
    Invoke-PhaseCase $phase @{Release=$true} @('phase', $phase, '--release')
    Invoke-PhaseCase $phase @{Package=$true} @('phase', $phase, '--package')
}
Invoke-PhaseCase '10' @{Resources=$true; ResourceModule='window'; Json=$true} @('phase', '10', '--resources', '--resource-module=window', '--json')
foreach ($phase in @('11', '11-b')) {
    Invoke-PhaseCase $phase @{Json=$true; EvidenceFile='target/中文 space.json'} @('phase', $phase, '--json', '--evidence-file=target/中文 space.json')
}
Invoke-PhaseCase 'all' @{Ci=$true; CiShard='tests'; Json=$true} @('all', '--ci', '--ci-shard=tests', '--json')
Invoke-PhaseCase 'all' @{Resources=$true; ResourceModule='images'; Performance=$false} @('all', '--resources', '--resource-module=images')
foreach ($phase in @('12', '13', '14')) {
    Invoke-PhaseCase $phase @{Json=$true; EvidenceFile='target/中文 space.json'; Ci=$false; Verbose=$true} @('phase', $phase, '--json', '--evidence-file=target/中文 space.json')
    Invoke-PhaseCase $phase @{Readiness=$true; Explain=$true} @('qualification', 'readiness', '--explain')
    Invoke-PhaseCase $phase @{RemoteRunId=123; RemoteAttempt=2} @('qualification', 'remote', '--run-id=123', '--attempt=2')
    Invoke-PhaseCase $phase @{DownloadedZip='中文 package.zip'} @('qualification', 'downloaded', '--zip=中文 package.zip')
    Invoke-PhaseCase $phase @{DecisionKey='key'; DecisionStatus='state'; DecisionEvidence='中文 evidence'} @('qualification', 'decision', '--key=key', '--status=state', '--evidence=中文 evidence')
}
Invoke-PhaseCase '12' @{Manual=$true} @('acceptance', 'manual')
foreach ($phase in @('13', '14')) {
    Invoke-PhaseCase $phase @{ManualSession='M3'} @('acceptance', 'manual', 'run', '--session=M3')
    Invoke-PhaseCase $phase @{Environment=$true; EvidenceFile='target/env.json'} @('qualification', 'environment', '--evidence-file=target/env.json')
}
Invoke-PhaseCase '14' @{SourceFreeze=$true} @('qualification', 'source-freeze')
Invoke-PhaseCase '14' @{Readiness=$true; WindowStressScenario='COMBINED'; ResourceModule='WINDOW'} @('qualification', 'readiness')
Invoke-PhaseCase '14' @{WindowStress=$true; TrayCycles=0} @('qualification', 'window-stress', '--scenario=combined', '--runs=10', '--collapse-cycles=1000', '--tray-cycles=0', '--control-cycles=100', '--view-mode-cycles=100', '--persistence-cycles=100')
Invoke-PhaseCase '14' @{G3=$true; G3Zip='中文.zip'; G3Case='G3-01'} @('qualification', 'g3', '--zip=中文.zip', '--case=G3-01')
Invoke-PhaseCase '14' @{Resources=$true; ResourceResume=$true; ResourcePlan=$true; EvidenceFile='target/keep.json'} @('phase', '14', '--resources', '--resource-resume', '--resource-plan', '--evidence-file=target/keep.json')
if (-not $env:STICKYMD_PHASE_BASELINE) {
    foreach ($case in @(
        @{Entry='all'; Parameters=@{CiShard='tests'}},
        @{Entry='all'; Parameters=@{Ci=$true; CiShard='invalid'}},
        @{Entry='10'; Parameters=@{ResourceModule='window'}},
        @{Entry='09'; Parameters=@{Release=$true; Package=$true}},
        @{Entry='11'; Parameters=@{ResourceModule='window'}},
        @{Entry='00'; Parameters=@{Performance=$true}}
    )) {
        $name = if ($case.Entry -eq 'all') { 'all.ps1' } else { "phase-$($case.Entry).ps1" }
        $parameters = $case.Parameters
        $rejected = $false
        $result = @()
        try {
            $result = @(& (Join-Path $env:STICKYMD_TEST_ROOT "tools/smoke/$name") @parameters 2> (Join-Path $location 'error.txt'))
            $rejected = $LASTEXITCODE -ne 0
        } catch { $rejected = $true }
        if (-not $rejected -or $result.Count -ne 0) { throw 'Invalid legacy input returned a successful route' }
        if ((Get-Location).Path -cne $location -or [Console]::OutputEncoding.CodePage -ne 936) { throw 'Legacy failure changed caller state' }
    }
    foreach ($mode in @('ResourcePlan', 'ResourceFailureFirst', 'ResourceResume')) {
        foreach ($action in @('SourceFreeze', 'Environment', 'WindowStress', 'Campaign')) {
            $parameters = @{Resources=$true; ResourceResume=$true; EvidenceFile='target/keep.json'}
            $parameters[$mode] = $true
            $parameters[$action] = $true
            $result = @()
            try { $result = @(& (Join-Path $env:STICKYMD_TEST_ROOT 'tools/smoke/phase-14.ps1') @parameters 2> (Join-Path $location 'error.txt')) } catch { }
            if ($LASTEXITCODE -eq 0 -or $result.Count -ne 0) { throw 'Invalid combination returned a successful route' }
            if ((Get-Location).Path -cne $location -or [Console]::OutputEncoding.CodePage -ne 936) { throw 'Failure changed caller state' }
        }
    }
}
'PHASE_ENTRY=PASS'
