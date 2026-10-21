//! Unit-test owners of manual obligations and their verification (plan 11: an obligation a
//! unit test owns counts as owned only when the headless CI receipt lists that owner as
//! verified).
//!
//! An owner is one libtest test of a workspace package's `lib` or `bin` target. Verification
//! builds the test executables once, finds each owner's executable from Cargo's structured
//! `compiler-artifact` messages, lists that executable's tests (`--list` and
//! `--list --ignored`) and runs every owner in its own process (`--exact <path> --nocapture`),
//! so one test cannot end the process before another owner ran. Parsing is platform
//! independent and tested against this toolchain's output; execution is Windows-only, like the
//! package whose tests own obligations today.
//!
//! What it guards against is accidental: an owner deleted, renamed, ignored, excluded by `cfg`,
//! missed by a filter, failing or never run. A test body that ends its own process early with
//! success, or asserts less than its name says, is a code-review concern.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::fmt;

// The parsers serve the Windows executor; their tests run everywhere.
#[cfg(any(windows, test))]
mod artifacts;
#[cfg(any(windows, test))]
mod listing;
#[cfg(windows)]
mod verify;

#[cfg(windows)]
use artifacts::{TestArtifact, parse_artifacts};
#[cfg(windows)]
use listing::parse_list;
#[cfg(windows)]
pub(crate) use verify::verify;

/// A libtest target that owns tests: only the default harness, so `--list` and `--exact`
/// mean what libtest documents (governance keeps `harness = false` to benches).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum OwnerTarget {
    Lib,
    Bin(&'static str),
}

impl fmt::Display for OwnerTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lib => formatter.write_str("lib"),
            Self::Bin(name) => write!(formatter, "bin:{name}"),
        }
    }
}

/// One unit test that owns obligations: `path` is its full libtest name.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct Owner {
    pub(crate) package: &'static str,
    pub(crate) target: OwnerTarget,
    pub(crate) path: &'static str,
}

impl fmt::Display for Owner {
    /// The identity the receipt records: `package/target/path`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}/{}", self.package, self.target, self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::{Owner, OwnerTarget};

    #[test]
    fn an_owner_is_recorded_as_package_target_and_path() {
        let owner = Owner {
            package: "stickymd-win",
            target: OwnerTarget::Bin("stickymd-win"),
            path: "app::escape::tests::modifiers_never_change_esc",
        };
        assert_eq!(
            owner.to_string(),
            "stickymd-win/bin:stickymd-win/app::escape::tests::modifiers_never_change_esc"
        );
        let lib = Owner {
            target: OwnerTarget::Lib,
            ..owner
        };
        assert!(lib.to_string().starts_with("stickymd-win/lib/"));
    }
}
