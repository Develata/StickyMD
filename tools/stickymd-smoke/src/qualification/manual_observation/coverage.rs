//! The manual obligation mapping (plan 11 `manual-observation-log`, 人工义务映射): every Phase 8
//! `Manual` row and the plan 09 clauses that need a real desktop are broken into assertions with
//! their required parameters, and each `(source, assertion, parameter)` is owned by a Phase 12
//! scenario, an exact G3/G4 case, a USER retirement or a named open gap. Owners proven by other
//! automation are bound to the module receipts of the executed tests with the exact / automated
//! extensions (coverage batch C2); until then they are gaps that name the existing entry.
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
    /// An exact-candidate G3/G4 case; per-check labels arrive with the exact extensions.
    Exact { case: &'static str },
    /// Retired by the USER decision `decision` (see `RETIREMENTS`).
    Retired { decision: &'static str },
    /// Not owned yet; `planned` names the intended owner.
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

use table::OBLIGATIONS;

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
    let owner = match obligation.target {
        Target::Gap { planned } => planned,
        Target::Retired { decision } => decision,
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
