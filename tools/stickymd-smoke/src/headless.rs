//! Explicit Cargo module selection for development-only headless checks.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Module {
    Core,
    Render,
    App,
    Desktop,
    Posix,
    Windows,
    Mac,
    Smoke,
    MarkdownMath,
    Persistence,
}

impl Module {
    pub(crate) const ALL: [Self; 10] = [
        Self::Core,
        Self::Render,
        Self::App,
        Self::Desktop,
        Self::Posix,
        Self::Windows,
        Self::Mac,
        Self::Smoke,
        Self::MarkdownMath,
        Self::Persistence,
    ];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Render => "render",
            Self::App => "app",
            Self::Desktop => "desktop",
            Self::Posix => "posix",
            Self::Windows => "windows",
            Self::Mac => "mac",
            Self::Smoke => "smoke",
            Self::MarkdownMath => "phase1-markdown-math",
            Self::Persistence => "phase1-persistence",
        }
    }

    pub(crate) const fn package(self) -> Option<&'static str> {
        match self {
            Self::Core => Some("stickymd-core"),
            Self::Render => Some("stickymd-render"),
            Self::App => Some("stickymd-app"),
            Self::Desktop => Some("stickymd-desktop"),
            Self::Posix => Some("stickymd-posix"),
            Self::Windows => Some("stickymd-win"),
            Self::Mac => Some("stickymd-mac"),
            Self::Smoke => Some("stickymd-smoke"),
            Self::MarkdownMath | Self::Persistence => None,
        }
    }

    pub(crate) const fn manifest(self) -> Option<&'static str> {
        match self {
            Self::MarkdownMath => Some("experiments/phase-01/markdown-math/Cargo.toml"),
            Self::Persistence => Some("experiments/phase-01/persistence/Cargo.toml"),
            _ => None,
        }
    }

    pub(crate) const fn root(self) -> &'static str {
        match self {
            Self::Core => "crates/stickymd-core/",
            Self::Render => "crates/stickymd-render/",
            Self::App => "crates/stickymd-app/",
            Self::Desktop => "crates/stickymd-desktop/",
            Self::Posix => "crates/stickymd-posix/",
            Self::Windows => "apps/stickymd-win/",
            Self::Mac => "apps/stickymd-mac/",
            Self::Smoke => "tools/stickymd-smoke/",
            Self::MarkdownMath => "experiments/phase-01/markdown-math/",
            Self::Persistence => "experiments/phase-01/persistence/",
        }
    }

    pub(crate) const fn cargo_name(self) -> &'static str {
        match self.package() {
            Some(name) => name,
            None => match self {
                Self::MarkdownMath => "spike-markdown-math",
                _ => "spike-persistence",
            },
        }
    }

    pub(crate) const fn dependencies(self) -> &'static [Self] {
        match self {
            Self::Render => &[Self::Core],
            Self::App => &[Self::Core, Self::Render],
            Self::Desktop => &[Self::Core, Self::Render, Self::App],
            Self::Windows => &[Self::Core, Self::Render, Self::App, Self::Desktop],
            Self::Mac => &[
                Self::Core,
                Self::Render,
                Self::App,
                Self::Desktop,
                Self::Posix,
            ],
            _ => &[],
        }
    }

    /// Whether the Windows lanes (clippy and the headless matrix) cover the module. The macOS
    /// host compiles to a stub with no tests off macOS, so a green Windows lane would prove
    /// nothing about it; its own lane is the macOS job (plan 11 Phase 16).
    pub(crate) const fn windows_lane(self) -> bool {
        // POSIX is empty on Windows, so only the macOS lane tests its native implementation.
        !matches!(self, Self::Mac | Self::Posix)
    }

    /// Whether the module's tests exist only on macOS.
    pub(crate) const fn needs_macos(self) -> bool {
        matches!(self, Self::Mac | Self::Posix)
    }

    /// Whether this process can run the module's tests. Only the macOS host has a body to test;
    /// the Windows host's own constraint is kept where it was.
    pub(crate) const fn runs_on_this_host(self) -> bool {
        !self.needs_macos() || cfg!(target_os = "macos")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Mode {
    Tests,
    Performance,
    All,
}

impl Mode {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Tests => "tests",
            Self::Performance => "performance",
            Self::All => "all",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Request {
    pub(crate) modules: Vec<Module>,
    pub(crate) mode: Mode,
    pub(crate) plan_only: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Command {
    List,
    Run(Request),
}

impl Command {
    pub(crate) fn parse(args: &[String]) -> Result<Self, String> {
        if args.len() == 1 && args[0] == "list" {
            return Ok(Self::List);
        }
        if args.first().map(String::as_str) != Some("run") || args.len() < 2 {
            return Err(Self::usage());
        }
        let modules = parse_modules(&args[1])?;
        let mut mode = None;
        let mut plan_only = false;
        for arg in &args[2..] {
            match arg.as_str() {
                "--plan" if !plan_only => plan_only = true,
                value if value.starts_with("--mode=") && mode.is_none() => {
                    mode = Some(match &value[7..] {
                        "tests" => Mode::Tests,
                        "performance" => Mode::Performance,
                        "all" => Mode::All,
                        _ => return Err(Self::usage()),
                    });
                }
                _ => return Err(Self::usage()),
            }
        }
        Ok(Self::Run(Request {
            modules,
            mode: mode.unwrap_or(Mode::Tests),
            plan_only,
        }))
    }

    fn usage() -> String {
        "usage: stickymd-smoke modules list | modules run <module[,module...]|all> [--mode=tests|performance|all] [--plan]".to_owned()
    }
}

pub(crate) fn parse_modules(value: &str) -> Result<Vec<Module>, String> {
    if value == "all" {
        return Ok(Module::ALL.to_vec());
    }
    let mut modules = BTreeSet::new();
    for name in value.split(',') {
        let module = Module::ALL
            .into_iter()
            .find(|module| module.name() == name)
            .ok_or_else(|| format!("unknown headless module `{name}`; use `modules list`"))?;
        modules.insert(module);
    }
    Ok(modules.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_selection_is_deduplicated_and_invalid_requests_fail_closed() {
        let parse = |args: &[&str]| {
            Command::parse(
                &args
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            parse(&["run", "render,core,render", "--plan"]).unwrap(),
            Command::Run(Request {
                modules: vec![Module::Core, Module::Render],
                mode: Mode::Tests,
                plan_only: true,
            })
        );
        for args in [
            vec!["run"],
            vec!["run", ""],
            vec!["run", "all,core"],
            vec!["run", "core,"],
            vec!["run", "unknown"],
            vec!["run", "core", "--mode=runtime"],
            vec!["run", "core", "--mode=tests", "--mode=all"],
            vec!["run", "core", "--plan", "--plan"],
            vec!["run", "core", "--evidence-file=candidate.json"],
            vec!["list", "core"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
        assert_eq!(parse(&["list"]), Ok(Command::List));
    }
}
