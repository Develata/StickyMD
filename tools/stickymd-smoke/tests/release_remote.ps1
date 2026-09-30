$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
function cargo {
    if (($args[0..4] -join ' ') -cne 'run --quiet -p stickymd-smoke --locked') { throw 'Unexpected release bootstrap' }
    Push-Location -LiteralPath $env:STICKYMD_TEST_ROOT
    try { & $env:STICKYMD_TEST_EXE @($args[5..($args.Length - 1)]) } finally { Pop-Location }
}
function gh {
    if ($args[0] -eq 'api' -and $args[1] -eq '--include') {
        if ($args[2] -eq 'graphql') {
            if (($args -join '|') -notmatch 'release\(tagName: \$tag\)' -or $args -cnotcontains "tag=$env:RELEASE_TAG") { throw 'Wrong draft query' }
            $body = '{"data":{"repository":{"release":{"tagName":"' + $env:RELEASE_TAG + '","isDraft":true}}}}'
            if ($global:RemoteCase -eq 'missing') { $body = '{"data":{"repository":{"release":null}}}' }
            if ($global:RemoteCase -eq 'invalid') { $body = $body.Replace('true', 'false') }
            if ($global:RemoteCase -eq 'graphql-error') { $body = '{"errors":[{"message":"denied"}],"data":{"repository":{"release":null}}}' }
        } else {
            if ($args[2] -cne "repos/owner/StickyMD/git/ref/tags/$([Uri]::EscapeDataString($env:RELEASE_TAG))") { throw 'Wrong tag query' }
            $body = '{"ref":"refs/tags/' + $env:RELEASE_TAG + '","object":{"sha":"' + $env:APPROVED_SOURCE_SHA + '"}}'
            if ($global:RemoteCase -eq 'missing') { $global:RemoteStatus = 404 }
            if ($global:RemoteCase -eq 'invalid') { $body = $body.Replace($env:APPROVED_SOURCE_SHA, ('b' * 40)) }
        }
        $global:LASTEXITCODE = 0
        if ($global:RemoteStatus -ne 200) { $global:LASTEXITCODE = 1 }
        if ($global:RemoteCase -eq 'transport') { $global:LASTEXITCODE = 23; return }
        "HTTP/2.0 $global:RemoteStatus`nContent-Type: application/json`n`n$body"
        return
    }
    if ($args[0] -eq 'api') {
        if (($args[0..3] -join '|') -cne 'api|--method|POST|repos/owner/StickyMD/git/refs') { throw 'Unexpected tag mutation' }
        $global:RemoteWrites.Add('tag')
    } elseif ($args[0] -eq 'release') {
        if ($args[2] -cne $env:RELEASE_TAG) { throw 'Unexpected release identity' }
        $global:RemoteWrites.Add([string]$args[1])
    } else { throw 'Unexpected GitHub operation' }
    $global:LASTEXITCODE = 0
}
Set-Location -LiteralPath $env:STICKYMD_TEST_DIRECTORY
[Console]::OutputEncoding = [Text.Encoding]::GetEncoding(936)
$env:RUNNER_TEMP = $env:STICKYMD_TEST_DIRECTORY
$env:GH_REPO = 'owner/StickyMD'
$env:APPROVED_SOURCE_SHA = 'a' * 40
New-Item -ItemType Directory -Path 'dist/promoted' -Force | Out-Null
[IO.File]::WriteAllText((Join-Path $env:STICKYMD_TEST_DIRECTORY 'dist/promoted/StickyMD-test-windows-x64-portable.zip'), 'test fixture')
$workflow = [IO.File]::ReadAllText((Join-Path $env:STICKYMD_TEST_ROOT '.github/workflows/promote-release.yml')).Replace("`r`n", "`n")
$steps = @(
    @('Create or verify exact source tag', '', 'tag'),
    @('Verify tag identity', '', $null),
    @('Create or update draft only', 'edit|upload', 'create'),
    @('Download existing draft assets', 'download', $null)
)
foreach ($step in $steps) {
    $body = $workflow.Split(@("- name: $($step[0])`n"), [StringSplitOptions]::None)[1].Split(@("        run: |`n"), [StringSplitOptions]::None)[1].Split(@("`n      - name:"), [StringSplitOptions]::None)[0]
    $lines = [Collections.Generic.List[string]]::new()
    foreach ($line in ($body -split "`n")) {
        if ($line.Trim() -and -not $line.StartsWith('          ')) { break }
        $lines.Add(($line -replace '^          ', ''))
    }
    $body = $lines -join "`n"
    $helper = (Join-Path $env:STICKYMD_TEST_ROOT 'tools/release/github-observation.ps1').Replace("'", "''")
    $body = $body.Replace('./tools/release/github-observation.ps1', "'$helper'")
    $scriptPath = Join-Path $env:STICKYMD_TEST_DIRECTORY 'workflow-step.ps1'
    [IO.File]::WriteAllText($scriptPath, $body, [Text.UTF8Encoding]::new($true))
    foreach ($global:RemoteCase in @('valid', 'missing', 'invalid', 'denied', 'transport', 'graphql-error')) {
        if ($global:RemoteCase -eq 'graphql-error' -and $step[0] -notmatch 'draft') { continue }
        $global:RemoteStatus = 200
        if ($global:RemoteCase -eq 'denied') { $global:RemoteStatus = 403 }
        $global:RemoteWrites = [Collections.Generic.List[string]]::new()
        $caught = $false
        $problem = ''
        $global:LASTEXITCODE = 0
        try { & $scriptPath 2> (Join-Path $env:STICKYMD_TEST_DIRECTORY 'error.txt') | Out-Null } catch { $caught = $true; $problem = $_.Exception.Message }
        $allowed = $global:RemoteCase -eq 'valid' -or ($global:RemoteCase -eq 'missing' -and $null -ne $step[2])
        if ($allowed) {
            $expected = $step[1]
            if ($global:RemoteCase -eq 'missing') { $expected = $step[2] }
            if ($caught -or $LASTEXITCODE -ne 0 -or ($global:RemoteWrites -join '|') -cne $expected) { throw "Unexpected $($step[0]) result for $global:RemoteCase exit=$LASTEXITCODE writes=$($global:RemoteWrites -join '|') error=$problem; $([IO.File]::ReadAllText((Join-Path $env:STICKYMD_TEST_DIRECTORY 'error.txt')))" }
        } elseif (-not $caught -or $LASTEXITCODE -eq 0 -or $global:RemoteWrites.Count -ne 0) { throw "Failed observation allowed $($step[0]) mutation for $global:RemoteCase" }
        if ((Get-Location).Path -cne $env:STICKYMD_TEST_DIRECTORY -or [Console]::OutputEncoding.CodePage -ne 936) { throw 'Remote adapter changed caller state' }
    }
}
'REMOTE_STEPS=PASS'
