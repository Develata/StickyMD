//! One Cargo-built integration target lets libtest overlap independent CLI cases.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

mod cli_exit;
#[cfg(windows)]
mod package_path_wrapper;
#[cfg(windows)]
mod release_wrappers;

#[cfg(windows)]
fn powershell_command(shell: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let mut command = std::process::Command::new(shell);
    // Cases read the repository/prebuilt CLI and own distinct temporary directories.
    // Keep console and environment changes local to the child; Git queries cannot
    // refresh a shared index. Cargo still owns any dependency metadata/cache access.
    command
        .creation_flags(CREATE_NO_WINDOW)
        .env_remove("PSModulePath")
        .env("GIT_OPTIONAL_LOCKS", "0");
    command
}
