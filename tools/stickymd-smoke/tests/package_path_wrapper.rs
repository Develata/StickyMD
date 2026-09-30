#![cfg(windows)]

use super::support::powershell::{HOSTS, ScriptFixture, assert_pass};

#[test]
fn package_path_wrapper_preserves_unicode_paths_and_restores_the_callers_state() {
    let script = r#"
$ErrorActionPreference = 'Stop'
# Use the already-built binary; PowerShell consumes -- when calling a function.
function cargo {
    if (($args[0..4] -join ' ') -cne 'run --quiet -p stickymd-smoke --locked') {
        throw ('Unexpected Cargo invocation: ' + ($args -join '/'))
    }
    & $env:STICKYMD_TEST_EXE @($args[5..($args.Length - 1)])
}
. (Join-Path $env:STICKYMD_TEST_ROOT 'tools/release/package-path.ps1')
Set-Location -LiteralPath $env:STICKYMD_TEST_DIRECTORY
$expectedLocation = (Get-Location).Path
[Console]::OutputEncoding = [Text.Encoding]::GetEncoding(936)
$observed = Resolve-StickyMdPackagePath -RepoRoot $env:STICKYMD_TEST_ROOT -PackageDirectory $env:STICKYMD_TEST_DIRECTORY
if ($observed -cne (Join-Path $env:STICKYMD_TEST_DIRECTORY 'StickyMD-old-windows-x64-portable.zip')) {
    throw 'Non-ASCII package path did not round trip'
}
if ([Console]::OutputEncoding.CodePage -ne 936 -or (Get-Location).Path -cne $expectedLocation) {
    throw 'Success changed the caller state'
}
$relative = Resolve-StickyMdPackagePath -RepoRoot $env:STICKYMD_TEST_ROOT -PackageDirectory '.'
# Set-Location expands 8.3 aliases; relative inputs follow that actual location.
$expectedRelative = Join-Path $expectedLocation 'StickyMD-old-windows-x64-portable.zip'
if ($relative -cne $expectedRelative) {
    throw "Relative package path mismatch: expected '$expectedRelative', observed '$relative'"
}
$failed = $false
try {
    Resolve-StickyMdPackagePath -RepoRoot $env:STICKYMD_TEST_ROOT -PackageDirectory (Join-Path $env:STICKYMD_TEST_DIRECTORY 'missing')
} catch {
    $failed = $true
}
if (-not $failed) { throw 'Missing package directory was accepted' }
if ([Console]::OutputEncoding.CodePage -ne 936 -or (Get-Location).Path -cne $expectedLocation) {
    throw 'Failure changed the caller state'
}
'PASS'
"#;
    for host in HOSTS {
        let fixture = ScriptFixture::new(
            "package-wrapper",
            &format!("{}-中文 [packages]", host.edition),
            script,
        );
        std::fs::write(
            fixture.path().join("StickyMD-old-windows-x64-portable.zip"),
            [],
        )
        .unwrap();
        let Some(output) = fixture.run(host, |_| {}) else {
            continue;
        };
        assert_pass(host, &output, &["PASS"]);
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "PASS");
    }
}
