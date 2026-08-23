//! StickyMD checked-in phase verification CLI.
#![deny(unsafe_op_in_unsafe_fn)]

mod cli;
mod evidence;
mod governance;
#[cfg(windows)]
mod process_metrics;
mod qualification;
#[cfg(windows)]
mod ready_event;
mod runner;
#[cfg(windows)]
mod runtime;
#[cfg(windows)]
mod window_control;

fn main() {
    let result = run();
    if let Err(error) = &result {
        eprintln!("stickymd-smoke: {error}");
    }
    std::process::exit(exit_code(&result));
}

const fn exit_code(result: &Result<(), String>) -> i32 {
    if result.is_ok() { 0 } else { 1 }
}

fn run() -> Result<(), String> {
    let command = cli::CommandLine::parse(std::env::args().skip(1))?;
    let root = governance::find_repository_root(
        &std::env::current_dir()
            .map_err(|error| format!("cannot read current directory: {error}"))?,
    )?;
    match command {
        cli::CommandLine::Smoke(options) => runner::execute(&root, &options),
        cli::CommandLine::AcceptanceManual => qualification::record_manual(&root),
        cli::CommandLine::Qualification(command) => qualification::execute(&root, command),
    }
}

#[cfg(test)]
mod tests {
    use super::exit_code;

    #[test]
    fn phase10_exit_code_is_zero_only_for_a_passed_suite() {
        assert_eq!(exit_code(&Ok(())), 0);
        assert_ne!(exit_code(&Err("blocked".to_owned())), 0);
    }
}
