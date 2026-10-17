//! Manual acceptance at release readiness: the current candidate's observation context
//! reduced to one verdict per case and role, then tier and waiver policy. The summary and
//! the known coverage gaps are reported on every run, whatever the verdict.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#manual-observation-log

use std::path::Path;

use super::decisions::Decision;
use super::json;
use super::manual_observation::coverage;
use super::manual_observation::reduce::{self, Verdict};
use super::manual_observation::registry::Tier;
use super::manual_observation::target::Role;
use super::manual_observation::{store, waiver};

/// The superseded schema 1 receipt: kept as it is, never converted into observations.
const SUPERSEDED_RECEIPT: &str = "dist/evidence/manual-acceptance.json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Row {
    case: &'static str,
    tier: Tier,
    role: Role,
    status: &'static str,
    detail: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Report {
    context_id: Option<String>,
    /// The reset that started this context: decision key and the replaced context.
    reset: Option<(String, String)>,
    rows: Vec<Row>,
    notes: Vec<String>,
    superseded_receipt: bool,
    /// Whether the headless CI receipt verified the mapping's unit-test owners; until it
    /// did, every obligation a unit test owns awaits that proof.
    unit_owners_proven: bool,
}

impl Report {
    /// Records the headless verdict on the unit-test owners (`automated_readiness::check`).
    pub(super) fn set_unit_owner_proof(&mut self, proven: bool) {
        self.unit_owners_proven = proven;
    }

    /// What is known without a context: only whether the superseded receipt is present.
    pub(super) fn detached(root: &Path) -> Self {
        Self {
            superseded_receipt: root.join(SUPERSEDED_RECEIPT).is_file(),
            ..Self::default()
        }
    }
}

/// Applies the manual policy to the context of `candidate_receipt_sha256`.
///
/// Precondition: the caller holds the manual store lock and read `decisions` (the same
/// snapshot its release gates use) while holding it, so the context and every decision
/// belong to one moment: a waiver cannot count from a later read than a release approval.
pub(super) fn check(
    root: &Path,
    decisions: &[Decision],
    candidate_receipt_sha256: &str,
    candidate_exe_sha256: &str,
    version: &str,
    automated_ok: bool,
    blockers: &mut Vec<String>,
) -> Report {
    let report = Report::detached(root);
    match store::load(root, candidate_receipt_sha256) {
        Ok(context) => evaluate(
            report,
            &context,
            candidate_exe_sha256,
            decisions,
            version,
            automated_ok,
            blockers,
        ),
        Err(error) => {
            blockers.push(format!("mandatory manual acceptance: {}", error.message()));
            report
        }
    }
}

/// The manual policy over one consistent context and decision snapshot.
fn evaluate(
    mut report: Report,
    context: &store::Context,
    candidate_exe_sha256: &str,
    decisions: &[Decision],
    version: &str,
    automated_ok: bool,
    blockers: &mut Vec<String>,
) -> Report {
    let context_id = context.target.context_id.clone();
    report.reset = context
        .log
        .header
        .reset
        .as_ref()
        .map(|reset| (reset.decision.clone(), reset.previous_context_id.clone()));
    for decision in decisions {
        if waiver::is_waiver_key(&decision.key)
            && decision.status == "USER APPROVED"
            && decision.scope.is_none()
        {
            report.notes.push(format!(
                "waiver {} uses the superseded format (no context or cells) and waives nothing",
                decision.key
            ));
        }
    }
    let binding = reduce::Binding {
        target: &context.target,
        exe_sha256: candidate_exe_sha256,
    };
    for cell in reduce::reduce(&context.log, binding) {
        let (case, tier, role) = (cell.case.id, cell.case.tier, cell.role);
        let label = format!("{case} ({})", role.as_str());
        let (status, detail) = match cell.verdict {
            Verdict::Pass => ("MANUAL PASS", String::new()),
            Verdict::Fail(scenarios) => {
                let detail = scenarios.join(", ");
                blockers.push(format!("manual acceptance {label} failed: {detail}"));
                ("MANUAL_FAIL", detail)
            }
            Verdict::Invalid(reason) => {
                blockers.push(format!(
                    "manual acceptance {label} has invalid evidence: {reason}"
                ));
                ("INVALID", reason)
            }
            Verdict::NotTested { covered, missing } => {
                let missing = if covered.is_empty() {
                    format!("missing {}", missing.join(", "))
                } else {
                    format!(
                        "covered {}; missing {}",
                        covered.join(", "),
                        missing.join(", ")
                    )
                };
                if let Some(key) = covering_waiver(decisions, &context_id, case, role, version) {
                    ("USER WAIVED", format!("{key}; {missing}"))
                } else if tier == Tier::C && automated_ok {
                    (
                        "NOT TESTED",
                        format!("Tier C with automated coverage passed; {missing}"),
                    )
                } else {
                    blockers.push(if tier == Tier::C {
                        format!(
                            "Tier C manual acceptance {label} is NOT TESTED while automated coverage is not fully PASSED"
                        )
                    } else {
                        format!(
                            "Tier {} manual acceptance {label} is NOT TESTED without a USER waiver bound to this context ({missing})",
                            tier.as_str()
                        )
                    });
                    ("NOT TESTED", missing)
                }
            }
        };
        report.rows.push(Row {
            case,
            tier,
            role,
            status,
            detail,
        });
    }
    if report.rows.iter().any(|row| row.status == "USER WAIVED") {
        report.notes.push(
            "USER WAIVED cells meet the approved release policy; they are not completed manual acceptance".to_owned(),
        );
    }
    report.context_id = Some(context_id);
    report
}

/// The approved waiver, if any, whose own key and recorded cells cover `(case, role)` in
/// this context. The cells are re-validated against the key, so an edited receipt cannot
/// widen a waiver.
fn covering_waiver<'a>(
    decisions: &'a [Decision],
    context_id: &str,
    case: &str,
    role: Role,
    version: &str,
) -> Option<&'a str> {
    decisions
        .iter()
        .filter(|decision| decision.status == "USER APPROVED")
        .find(|decision| {
            decision.scope.as_ref().is_some_and(|scope| {
                scope.context_id == context_id
                    && scope
                        .cells
                        .iter()
                        .any(|(cell, cell_role)| cell == case && *cell_role == role)
                    && waiver::validate(&decision.key, &scope.cells, version).is_ok()
            })
        })
        .map(|decision| decision.key.as_str())
}

pub(super) fn print(report: &Report) {
    match &report.context_id {
        Some(context) => println!("Manual acceptance (context {context}):"),
        None => println!("Manual acceptance: no usable observation context"),
    }
    if let Some((decision, previous)) = &report.reset {
        println!("  started by {decision}, replacing context {previous}");
    }
    let mut rows = report.rows.iter().peekable();
    while let Some(row) = rows.next() {
        let mut line = format!(
            "  {} [{}] {}: {}",
            row.case,
            row.tier.as_str(),
            row.role.as_str(),
            row.status
        );
        if !row.detail.is_empty() {
            line.push_str(&format!(" ({})", row.detail));
        }
        if let Some(next) = rows.peek().filter(|next| next.case == row.case) {
            line.push_str(&format!("; {}: {}", next.role.as_str(), next.status));
            if !next.detail.is_empty() {
                line.push_str(&format!(" ({})", next.detail));
            }
            rows.next();
        }
        println!("{line}");
    }
    for note in &report.notes {
        println!("  note: {note}");
    }
    if report.superseded_receipt {
        println!("  note: {SUPERSEDED_RECEIPT} (schema 1) is superseded; it is kept but not read");
    }
    println!(
        "Open manual coverage gaps (no owner yet; the manual matrix is not complete, and a complete mapping is not a PASS):"
    );
    for gap in coverage::gaps() {
        println!("  - {}", coverage::describe(gap));
    }
    println!("Retired manual obligations (USER decisions):");
    for retired in coverage::retired() {
        println!("  - {}", coverage::describe(retired));
    }
    if report.unit_owners_proven {
        println!("Unit-test owned obligations (verified by the headless CI receipt):");
    } else {
        println!(
            "Unit-test owned obligations awaiting proof (the headless CI receipt of the frozen source has not verified their owners):"
        );
    }
    for owned in coverage::test_owned() {
        println!("  - {}", coverage::describe(owned));
    }
}

/// The `manual` member of the readiness receipt.
pub(super) fn render(report: &Report) -> String {
    let context = report
        .context_id
        .as_ref()
        .map_or_else(|| "null".to_owned(), |context| format!("\"{context}\""));
    let rows = report
        .rows
        .iter()
        .map(|row| {
            format!(
                "{{\"case\":\"{}\",\"tier\":\"{}\",\"role\":\"{}\",\"status\":\"{}\",\"detail\":\"{}\"}}",
                row.case,
                row.tier.as_str(),
                row.role.as_str(),
                row.status,
                json::escape(&row.detail)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let gaps = coverage::gaps().map(coverage::describe).collect::<Vec<_>>();
    let retired = coverage::retired()
        .map(coverage::describe)
        .collect::<Vec<_>>();
    let unit_owned = coverage::test_owned()
        .map(coverage::describe)
        .collect::<Vec<_>>();
    let quoted = |items: &mut dyn Iterator<Item = &str>| {
        items
            .map(|item| format!("\"{}\"", json::escape(item)))
            .collect::<Vec<_>>()
            .join(",")
    };
    let reset = report.reset.as_ref().map_or_else(
        || "null".to_owned(),
        |(decision, previous)| {
            format!(
                "{{\"decision\":\"{}\",\"previous_context_id\":\"{}\"}}",
                json::escape(decision),
                json::escape(previous)
            )
        },
    );
    format!(
        "{{\"context_id\":{context},\"reset\":{reset},\"cells\":[{rows}],\"notes\":[{}],\"known_gaps\":[{}],\"retired\":[{}],\"unit_test_owned\":{{\"verified\":{},\"obligations\":[{}]}},\"superseded_receipt\":{}}}",
        quoted(&mut report.notes.iter().map(String::as_str)),
        quoted(&mut gaps.iter().map(String::as_str)),
        quoted(&mut retired.iter().map(String::as_str)),
        report.unit_owners_proven,
        quoted(&mut unit_owned.iter().map(String::as_str)),
        report.superseded_receipt
    )
}

#[cfg(test)]
mod tests;
