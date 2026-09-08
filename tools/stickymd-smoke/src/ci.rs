//! Source-scoped headless CI planning and job-result aggregation.
//! plan_ref: docs/plan/11_testing_and_release.md#modular-headless-ci

mod git;
mod projection;
mod registry;
mod results;
mod selection;
#[cfg(test)]
mod workflow_tests;

use std::path::Path;

use crate::headless::Module;
use selection::Selection;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Command {
    Plan { base: Option<String> },
    Verify(results::Results),
}

impl Command {
    pub(crate) fn parse(args: &[String]) -> Result<Self, String> {
        match args {
            [command, flag] if command == "plan" && flag == "--full" => {
                Ok(Self::Plan { base: None })
            }
            [command, flag] if command == "plan" && flag.starts_with("--base=") => {
                Ok(Self::Plan { base: Some(flag[7..].to_owned()) })
            }
            [command, rest @ ..] if command == "verify" => {
                results::Results::parse(rest).map(Self::Verify)
            }
            _ => Err("usage: stickymd-smoke ci plan <--full|--base=<commit>> | ci verify --cancelled=<bool> --full=<bool> --modules=<list> --plan=<status> --dependency=<status> --quality=<status> --headless=<status> --release=<status> --portable=<status>".to_owned()),
        }
    }
}

pub(crate) fn execute(root: &Path, command: &Command) -> Result<(), String> {
    match command {
        Command::Plan { base } => {
            let facts = git::inspect(root, base.as_deref())?;
            let mut selection = match facts.paths {
                Ok(paths) => selection::select(&paths),
                Err(reason) => Selection::full(reason),
            };
            if !selection.full
                && !selection.modules.is_empty()
                && let Err(error) = registry::verify(root)
            {
                selection = Selection::full(format!("Cargo module registry drift: {error}"));
            }
            println!(
                "{}",
                projection::json(&facts.head, base.as_deref(), facts.dirty, &selection)
            );
            Ok(())
        }
        Command::Verify(results) => {
            results.verify()?;
            println!("CI requested scope PASS; no candidate qualification receipt written");
            Ok(())
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Checks {
    dependency: bool,
    quality: bool,
    headless: bool,
    release: bool,
    portable: bool,
}

impl Checks {
    fn for_modules(full: bool, modules: &[Module]) -> Self {
        Self {
            dependency: full || !modules.is_empty(),
            quality: full || modules.iter().any(|module| module.package().is_some()),
            headless: full || !modules.is_empty(),
            release: full || modules.contains(&Module::Windows),
            portable: full
                || modules
                    .iter()
                    .any(|module| matches!(module, Module::Core | Module::Render)),
        }
    }
}
