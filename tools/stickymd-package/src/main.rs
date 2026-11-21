//! `stickymd-package`: the only entry point of the release workflows' byte-producing steps
//! (plan 11 #platform-artifact-reuse). It runs in the repository that contains the working
//! directory.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse
#![deny(unsafe_op_in_unsafe_fn)]

use stickymd_package::{cli, commands, repository};

fn main() {
    let result = run();
    if let Err(error) = &result {
        eprintln!("stickymd-package: {error}");
    }
    std::process::exit(if result.is_ok() { 0 } else { 1 });
}

fn run() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let command = cli::parse(&args).map_err(|error| format!("{error}\n{}", cli::USAGE))?;
    let root = repository::find_root(
        &std::env::current_dir()
            .map_err(|error| format!("cannot read current directory: {error}"))?,
    )?;
    commands::execute(&root, &command)
}
