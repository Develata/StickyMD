//! StickyMD checked-in phase verification CLI.
#![deny(unsafe_op_in_unsafe_fn)]

mod cli;
mod governance;
#[cfg(windows)]
mod process_metrics;
#[cfg(windows)]
mod ready_event;
mod runner;
#[cfg(windows)]
mod runtime;
#[cfg(windows)]
mod window_control;

fn main() {
    if let Err(error) = run() {
        eprintln!("stickymd-smoke: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let options = cli::Options::parse(std::env::args().skip(1))?;
    let root = governance::find_repository_root(
        &std::env::current_dir()
            .map_err(|error| format!("cannot read current directory: {error}"))?,
    )?;
    runner::execute(&root, &options)
}
