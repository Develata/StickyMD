//! StickyMD checked-in phase verification CLI.
#![deny(unsafe_op_in_unsafe_fn)]

mod atomic_evidence;
mod ci;
mod cli;
mod development;
mod evidence;
mod governance;
mod headless;
mod integrity;
#[cfg(windows)]
mod managed_process;
mod package_path;
mod pe_dependencies;
mod phase_entry;
#[cfg(windows)]
mod process_metrics;
mod qualification;
mod qualification_environment;
#[cfg(windows)]
mod ready_event;
mod release;
mod repository;
mod resource_plan;
mod runner;
#[cfg(windows)]
mod runtime;
mod startup_details;
mod startup_timing;
mod startup_trace;
mod timing_summary;
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
    #[cfg(windows)]
    window_control::enable_per_monitor_v2_dpi_awareness()?;
    let command = cli::CommandLine::parse(std::env::args().skip(1))?;
    if let cli::CommandLine::Timings(options) = &command {
        return timing_summary::execute(options);
    }
    if let cli::CommandLine::StartupDetails(options) = &command {
        return startup_details::execute(options);
    }
    let root = governance::find_repository_root(
        &std::env::current_dir()
            .map_err(|error| format!("cannot read current directory: {error}"))?,
    )?;
    match command {
        cli::CommandLine::PhaseEntryPlan(arguments) => {
            phase_entry::print_plan(&arguments);
            Ok(())
        }
        cli::CommandLine::Smoke(options) => runner::execute(&root, &options),
        cli::CommandLine::Ci(command) => ci::execute(&root, &command),
        cli::CommandLine::Development(command) => development::execute(&root, &command),
        cli::CommandLine::Timings(options) => timing_summary::execute(&options),
        cli::CommandLine::StartupDetails(options) => startup_details::execute(&options),
        cli::CommandLine::Release(command) => release::execute(&root, &command),
        cli::CommandLine::Modules(command) => runner::headless::execute(&root, &command),
        cli::CommandLine::AcceptanceManual(command) => qualification::record_manual(&root, command),
        cli::CommandLine::Qualification(command) => qualification::execute(&root, command),
        cli::CommandLine::PackagePath(directory) => {
            println!("{}", package_path::resolve(&root, &directory)?.display());
            Ok(())
        }
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
