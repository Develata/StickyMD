//! StickyMD checked-in phase verification CLI.
#![forbid(unsafe_code)]

mod cli;
mod governance;
mod runner;
#[cfg(windows)]
mod runtime;

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
