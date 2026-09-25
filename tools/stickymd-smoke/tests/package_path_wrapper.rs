#![cfg(windows)]

use std::{fs, path::Path, process::Command, time::SystemTime};

#[test]
fn package_path_wrapper_preserves_unicode_paths_and_restores_the_callers_state() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap();
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture = std::env::temp_dir().join(format!(
        "stickymd-package-wrapper-{}-{nonce}-中文 [packages]",
        std::process::id()
    ));
    fs::create_dir(&fixture).unwrap();
    fs::write(fixture.join("StickyMD-old-windows-x64-portable.zip"), []).unwrap();
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command"])
        .arg(
            r#"
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
"#,
        )
        .env("STICKYMD_TEST_EXE", env!("CARGO_BIN_EXE_stickymd-smoke"))
        .env("STICKYMD_TEST_ROOT", root)
        .env("STICKYMD_TEST_DIRECTORY", &fixture)
        .output()
        .expect("start Windows PowerShell compatibility check");
    // Only the two paths exclusively created by this test are removed.
    fs::remove_file(fixture.join("StickyMD-old-windows-x64-portable.zip")).unwrap();
    fs::remove_dir(&fixture).unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "PASS");
}
