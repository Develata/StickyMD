# Shell binding and process state only; Rust owns routing and combination rules.
function Invoke-StickyMdPhase {
    param([string]$RepoRoot, [string]$Phase, [System.Collections.IDictionary]$Parameters)
    $arguments = @('run', '-p', 'stickymd-smoke', '--locked', '--', 'phase-entry', $Phase)
    $common = @([System.Management.Automation.PSCmdlet]::CommonParameters) + @([System.Management.Automation.PSCmdlet]::OptionalCommonParameters)
    foreach ($entry in $Parameters.GetEnumerator()) {
        if ($entry.Key -in $common) { continue }
        $value = $entry.Value
        if ($value -is [System.Management.Automation.SwitchParameter]) { $value = $value.IsPresent.ToString().ToLowerInvariant() }
        $arguments += "--$($entry.Key)=$value"
    }
    $previousEncoding = [Console]::OutputEncoding
    $phaseExitCode = 1
    Push-Location -LiteralPath $RepoRoot
    try {
        [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
        & cargo @arguments
        $phaseExitCode = $LASTEXITCODE
    } finally {
        try { [Console]::OutputEncoding = $previousEncoding } finally { Pop-Location }
    }
    $global:LASTEXITCODE = $phaseExitCode
}
