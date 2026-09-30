. (Join-Path $PSScriptRoot 'invoke-smoke.ps1')

function Get-StickyMdRemoteState {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][ValidateSet('tag', 'draft')][string]$Kind,
        [Parameter(Mandatory = $true)][string]$SourceSha,
        [Parameter(Mandatory = $true)][string]$ReleaseTag,
        [string]$Repository = $env:GH_REPO,
        [switch]$AllowMissing
    )
    $parts = $Repository.Split('/')
    if ($parts.Count -ne 2 -or -not $parts[0] -or -not $parts[1]) { throw 'Expected owner/repository' }
    if ($Kind -eq 'tag') {
        $query = @('api', '--include', "repos/$Repository/git/ref/tags/$([Uri]::EscapeDataString($ReleaseTag))")
    } else {
        # Draft releases use a pending tag; the REST tags endpoint only finds published releases.
        $query = @('api', '--include', 'graphql', '-f', "owner=$($parts[0])", '-f', "name=$($parts[1])", '-f', "tag=$ReleaseTag", '-f',
            'query=query($owner: String!, $name: String!, $tag: String!) { repository(owner: $owner, name: $name) { release(tagName: $tag) { tagName isDraft } } }')
    }
    $observation = [IO.Path]::GetTempFileName()
    $previousEncoding = [Console]::OutputEncoding
    try {
        [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
        $previousPreference = $ErrorActionPreference
        try {
            # Nonzero gh exits still carry authoritative HTTP status (e.g. 404).
            # Keep native stderr separate from the response; Rust decides whether absence is allowed.
            $ErrorActionPreference = 'Continue'
            $PSNativeCommandUseErrorActionPreference = $false
            $response = @(& gh @query 2>$null)
            $queryExit = $LASTEXITCODE
        } finally { $ErrorActionPreference = $previousPreference }
        [IO.File]::WriteAllText($observation, ($response -join "`n"), [Text.UTF8Encoding]::new($false))
        $arguments = @('verify-remote-state', '--kind', $Kind, '--source-sha', $SourceSha,
            '--release-tag', $ReleaseTag, '--query-exit', "$queryExit", '--http-response', $observation)
        if ($AllowMissing) { $arguments += '--allow-missing' }
        (Invoke-StickyMdReleaseTool -RepoRoot $RepoRoot -Arguments $arguments) | ConvertFrom-Json
    } finally {
        try { [Console]::OutputEncoding = $previousEncoding } finally {
            Remove-Item -LiteralPath $observation -Force
        }
    }
}
