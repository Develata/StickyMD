//! The standalone manual recorder is this tool copied to a tested machine that has only
//! Windows (plan 11 `manual-observation-log`). Checked on the compiled tool itself, with the
//! product's import gate: it accepts everything the tool imports except CNG hashing and the
//! session query, two DLLs that also ship with Windows.

#[test]
fn the_tool_imports_only_dlls_that_ship_with_windows() {
    let tool = env!("CARGO_BIN_EXE_stickymd-smoke");
    let output = std::process::Command::new(tool)
        .args(["qualification", "native-runtime", &format!("--exe={tool}")])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    // Developer runtimes are refused before this message, so reaching it rules them out.
    let listed = stderr
        .trim()
        .split_once("outside the approved Windows inbox set: ")
        .map(|(_, listed)| listed)
        .unwrap_or_else(|| panic!("unexpected refusal: {stderr}"));
    let mut outside = listed
        .split(", ")
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    outside.sort();
    assert_eq!(outside, ["bcrypt.dll", "wtsapi32.dll"], "{stderr}");
}
