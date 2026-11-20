//! The manual obligation mapping (plan 11 `manual-observation-log`, 人工义务映射): every Phase 8
//! `Manual` row and the plan 09 clauses that need a real desktop are broken into assertions with
//! their required parameters, and each `(source, assertion, parameter)` is owned by a Phase 12
//! scenario, check labels of an exact G3/G4 case, a unit test, a USER retirement or a named
//! open gap. A unit-test owner counts only when the headless CI receipt lists it as verified;
//! obligations that other existing tests exercise stay gaps naming that entry until they are
//! bound to an owner (coverage batch C2).
//!
//! Two independent tables keep a reduction visible: `declared::ASSERTIONS` (compiled for tests
//! only) declares what each source requires, `table::OBLIGATIONS` says who owns each required
//! parameter; the tests demand that the parameters owned per `(source, assertion)` equal the
//! declared ones exactly. The source text itself is bound by digests in the tests, so a changed
//! row or clause fails until the decomposition is reviewed again.
//!
//! Static tables only: the runtime keeps `OBLIGATIONS` alone, and readiness reads the open gaps
//! and retirements in one linear pass over it (a few hundred entries, no allocation until a
//! line is described); the completeness checks sort and compare sets (O(n log n)) in tests.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#manual-observation-log

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::qualification) enum Target {
    /// A scenario of a Phase 12 manual case.
    Manual {
        case: &'static str,
        scenario: &'static str,
    },
    /// Check labels of an exact-candidate G3/G4 case that prove the obligation; a passing
    /// schema 2 receipt records every declared label (`exact_checks`).
    Exact {
        case: &'static str,
        checks: &'static [&'static str],
    },
    /// Unit tests that prove it together; owned only once the headless CI receipt lists every
    /// one of them as verified.
    Test(&'static [Owner]),
    /// Retired by the USER decision `decision` (see `RETIREMENTS`).
    Retired { decision: &'static str },
    /// Not owned yet; `planned` names the intended owner.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "retained for future unowned obligations")
    )]
    Gap { planned: &'static str },
}

/// Who owns one required parameter of one assertion.
#[derive(Clone, Copy, Debug)]
pub(in crate::qualification) struct Obligation {
    pub(in crate::qualification) source: &'static str,
    pub(in crate::qualification) assertion: &'static str,
    pub(in crate::qualification) param: &'static str,
    pub(in crate::qualification) target: Target,
}

mod table;

use std::collections::BTreeSet;

use crate::unit_owners::Owner;
use table::OBLIGATIONS;

/// The obligations unit tests own, in table order.
pub(in crate::qualification) fn test_owned() -> impl Iterator<Item = &'static Obligation> {
    OBLIGATIONS
        .iter()
        .filter(|obligation| matches!(obligation.target, Target::Test(_)))
}

/// Every unit test that owns an obligation, once each.
pub(in crate::qualification) fn test_owners() -> BTreeSet<Owner> {
    owners_of(OBLIGATIONS)
}

/// The union of the unit tests `obligations` name: every owner of every obligation.
fn owners_of(obligations: &[Obligation]) -> BTreeSet<Owner> {
    obligations
        .iter()
        .flat_map(|obligation| match obligation.target {
            Target::Test(owners) => owners,
            _ => &[],
        })
        .copied()
        .collect()
}

/// The obligations nobody owns yet, in table order.
pub(in crate::qualification) fn gaps() -> impl Iterator<Item = &'static Obligation> {
    OBLIGATIONS
        .iter()
        .filter(|obligation| matches!(obligation.target, Target::Gap { .. }))
}

/// The obligations the USER retired, in table order.
pub(in crate::qualification) fn retired() -> impl Iterator<Item = &'static Obligation> {
    OBLIGATIONS
        .iter()
        .filter(|obligation| matches!(obligation.target, Target::Retired { .. }))
}

/// One line for readiness and `acceptance manual list`.
pub(in crate::qualification) fn describe(obligation: &Obligation) -> String {
    let param = if obligation.param.is_empty() {
        String::new()
    } else {
        format!(" [{}]", obligation.param)
    };
    let test;
    let owner = match obligation.target {
        Target::Gap { planned } => planned,
        Target::Retired { decision } => decision,
        Target::Test(owners) => {
            let names = owners.iter().map(ToString::to_string).collect::<Vec<_>>();
            test = format!(
                "unit test{} {}",
                if names.len() == 1 { "" } else { "s" },
                names.join(" + ")
            );
            &test
        }
        _ => "",
    };
    format!(
        "{} {}{param}: {owner}",
        obligation.source, obligation.assertion
    )
}

#[cfg(test)]
mod declared;
#[cfg(test)]
mod sources;
#[cfg(test)]
mod tests;
