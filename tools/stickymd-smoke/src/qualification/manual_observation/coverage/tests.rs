//! The mapping against the documents it decomposes (`sources`): every assertion's parameters
//! are owned exactly once, every declared source exists in the documents, and every owner
//! exists.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::declared::{ASSERTIONS, Assertion, RETIREMENTS};
use super::sources::{decompose, documents};
use super::{OBLIGATIONS, Obligation, Target};
use crate::qualification::exact_checks::{self, DeclaredCase};
use crate::qualification::manual_observation::registry;

fn repository() -> PathBuf {
    crate::governance::find_repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap()
}

pub(super) fn read(relative: &str) -> String {
    std::fs::read_to_string(repository().join(relative)).unwrap()
}

pub(super) fn digest(text: &str) -> String {
    crate::integrity::sha256_bytes(text.as_bytes()).unwrap()
}

/// Errors in how `obligations` own the parameters `assertions` require: per
/// `(source, assertion)` the owned parameters must equal the required ones, merged within
/// one assertion only; unknown and duplicate entries are errors. O(n log n) in the table size.
fn ownership_errors(assertions: &[Assertion], obligations: &[Obligation]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut required: BTreeMap<(&str, &str), BTreeSet<&str>> = BTreeMap::new();
    for assertion in assertions {
        let params = assertion.params.iter().copied().collect::<BTreeSet<_>>();
        if params.len() != assertion.params.len() {
            errors.push(format!(
                "{} {} repeats a parameter",
                assertion.source, assertion.id
            ));
        }
        if required
            .insert((assertion.source, assertion.id), params)
            .is_some()
        {
            errors.push(format!(
                "{} {} is declared twice",
                assertion.source, assertion.id
            ));
        }
    }
    let mut owned: BTreeMap<(&str, &str), BTreeSet<&str>> = BTreeMap::new();
    for obligation in obligations {
        let key = (obligation.source, obligation.assertion);
        if !required.contains_key(&key) {
            errors.push(format!("{} {} is not a declared assertion", key.0, key.1));
        }
        if !owned.entry(key).or_default().insert(obligation.param) {
            errors.push(format!(
                "{} {} [{}] is owned twice",
                key.0, key.1, obligation.param
            ));
        }
    }
    for (key, params) in &required {
        let have = owned.get(key).cloned().unwrap_or_default();
        if &have != params {
            errors.push(format!(
                "{} {} owns {have:?} but requires {params:?}",
                key.0, key.1
            ));
        }
    }
    errors
}

#[test]
fn every_required_parameter_is_owned_exactly_once() {
    let errors = ownership_errors(ASSERTIONS, OBLIGATIONS);
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn every_document_source_is_decomposed_and_nothing_else() {
    let (phase08, plan09) = documents();
    let clauses = decompose(&phase08, &plan09).unwrap().clauses;
    let documented = clauses
        .iter()
        .map(|(id, _)| id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(documented.len(), clauses.len(), "a source ID repeats");
    let declared = ASSERTIONS
        .iter()
        .map(|assertion| assertion.source)
        .collect::<BTreeSet<_>>();
    assert_eq!(declared, documented);
}

/// The checker itself: a reduced parameter, a reduction masked by another assertion of the
/// same source, duplicates and unknown entries are all reported.
#[test]
fn the_ownership_checker_rejects_reductions_masking_and_unknowns() {
    let target = Target::Gap { planned: "x" };
    let assertions = [
        Assertion {
            source: "S",
            id: "a",
            params: &["mode=source", "mode=split"],
        },
        Assertion {
            source: "S",
            id: "b",
            params: &["mode=source", "mode=split"],
        },
    ];
    let full = [
        Obligation {
            source: "S",
            assertion: "a",
            param: "mode=source",
            target,
        },
        Obligation {
            source: "S",
            assertion: "a",
            param: "mode=split",
            target,
        },
        Obligation {
            source: "S",
            assertion: "b",
            param: "mode=source",
            target,
        },
        Obligation {
            source: "S",
            assertion: "b",
            param: "mode=split",
            target,
        },
    ];
    assert!(ownership_errors(&assertions, &full).is_empty());
    // b reduced to Source only, while a still covers both modes.
    assert!(!ownership_errors(&assertions, &full[..3]).is_empty());
    let mut duplicate = full.to_vec();
    duplicate.push(full[0]);
    assert!(!ownership_errors(&assertions, &duplicate).is_empty());
    let mut unknown = full.to_vec();
    unknown.push(Obligation {
        source: "S",
        assertion: "c",
        param: "",
        target,
    });
    assert!(!ownership_errors(&assertions, &unknown).is_empty());
}

/// The owners the repository provides.
struct Owners<'a> {
    /// The declared exact cases with their check labels.
    exact: &'a [&'a DeclaredCase],
    /// Phase 8, where every retirement must be recorded.
    phase08: &'a str,
}

/// The declared G3 and G4 cases, the groups coverage may name.
fn declared_exact() -> Vec<&'static DeclaredCase> {
    exact_checks::G3.iter().chain(exact_checks::G4).collect()
}

/// Errors in the owners `obligations` name: each must exist and match its kind.
fn owner_errors(obligations: &[Obligation], owners: &Owners) -> Vec<String> {
    let mut errors = Vec::new();
    for obligation in obligations {
        let label = format!(
            "{} {} [{}]",
            obligation.source, obligation.assertion, obligation.param
        );
        match obligation.target {
            Target::Manual { case, scenario } => {
                if !registry::find(case).is_some_and(|case| case.scenarios.contains(&scenario)) {
                    errors.push(format!("{label}: no scenario {case}/{scenario}"));
                }
            }
            Target::Exact { case, checks } => {
                let Some(declared) = owners.exact.iter().find(|declared| declared.id == case)
                else {
                    errors.push(format!("{label}: no exact case {case}"));
                    continue;
                };
                if checks.is_empty() {
                    errors.push(format!("{label}: {case} names no check"));
                }
                if checks.iter().collect::<BTreeSet<_>>().len() != checks.len() {
                    errors.push(format!("{label}: {case} names a check twice"));
                }
                for check in checks {
                    if !declared.checks.contains(check) {
                        errors.push(format!("{label}: {case} declares no check {check}"));
                    }
                }
            }
            Target::Retired { decision } => {
                let authorized = RETIREMENTS
                    .iter()
                    .filter(|retirement| {
                        retirement.decision == decision
                            && retirement.source == obligation.source
                            && retirement.assertion == obligation.assertion
                            && retirement.param == obligation.param
                    })
                    .count();
                if authorized != 1 {
                    errors.push(format!("{label}: {decision} does not authorize it"));
                }
                if !owners.phase08.contains(decision) {
                    errors.push(format!(
                        "{label}: {decision} is not recorded in phase-08.md"
                    ));
                }
            }
            Target::Gap { planned } => {
                if planned.is_empty() {
                    errors.push(format!("{label}: a gap names no planned owner"));
                }
            }
            // Existence is the owner verification's job; here only the identity's form.
            Target::Test(owner) => {
                if !crate::headless::Module::ALL
                    .iter()
                    .any(|module| module.package() == Some(owner.package))
                {
                    errors.push(format!("{label}: {owner} names no workspace package"));
                }
                if owner.path.is_empty() || owner.path.contains(char::is_whitespace) {
                    errors.push(format!("{label}: {owner} has no libtest path"));
                }
            }
        }
    }
    errors
}

#[test]
fn every_owner_exists() {
    let phase08 = read("docs/acceptance-cases/phase-08.md");
    let exact = declared_exact();
    let owners = Owners {
        exact: &exact,
        phase08: &phase08,
    };
    let errors = owner_errors(OBLIGATIONS, &owners);
    assert!(errors.is_empty(), "{errors:#?}");
    // Each retirement authorizes exactly one obligation.
    for retirement in RETIREMENTS {
        let used = OBLIGATIONS
            .iter()
            .filter(|obligation| {
                obligation.target
                    == Target::Retired {
                        decision: retirement.decision,
                    }
            })
            .count();
        assert_eq!(used, 1, "{}", retirement.decision);
    }
}

/// An exact owner without a producer, profile or Pin parameter claims every variant: the
/// per-profile matrices are exactly that profile's G4-06 labels, and the producer-free
/// clipboard obligation names every G3-01 label.
#[test]
fn unparameterized_exact_owners_claim_every_variant() {
    let owned = |source: &str, assertion: &str| {
        OBLIGATIONS
            .iter()
            .find(|obligation| obligation.source == source && obligation.assertion == assertion)
            .map(|obligation| match obligation.target {
                Target::Exact { checks, .. } => checks,
                other => panic!("{source} {assertion} is owned by {other:?}"),
            })
            .unwrap()
    };
    for (source, assertion, profile) in [
        ("P08-D117", "pinyin-functional", "ime=pinyin"),
        ("P08-D118", "wetype-functional", "ime=wetype"),
    ] {
        let expected = exact_checks::G4_06
            .iter()
            .filter(|label| label.contains(profile))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            owned(source, assertion).iter().collect::<BTreeSet<_>>(),
            expected
        );
    }
    assert_eq!(owned("P08-M13", "ime-functional"), exact_checks::G4_06);
    assert_eq!(owned("P08-M15", "clipboard-producers"), exact_checks::G3_01);
}

/// The owner checker itself: missing and unauthorized owners are all reported.
#[test]
fn the_owner_checker_rejects_missing_and_unauthorized_owners() {
    let exact = declared_exact();
    let owners = Owners {
        exact: &exact,
        phase08: "RETIRE-P08-M12-ACCESSIBILITY",
    };
    let errors = |target| {
        owner_errors(
            &[Obligation {
                source: "P08-M12",
                assertion: "accessibility",
                param: "",
                target,
            }],
            &owners,
        )
    };
    for good in [
        Target::Exact {
            case: "G4-01",
            checks: &["tray-icon-present"],
        },
        Target::Manual {
            case: "P12-M01",
            scenario: "pinyin-100",
        },
        Target::Retired {
            decision: "RETIRE-P08-M12-ACCESSIBILITY",
        },
        Target::Gap { planned: "x" },
    ] {
        assert!(errors(good).is_empty(), "{good:?}: {:?}", errors(good));
    }
    for bad in [
        Target::Exact {
            case: "G4-99",
            checks: &["tray-icon-present"],
        },
        Target::Exact {
            case: "G4-01",
            checks: &[],
        },
        Target::Exact {
            case: "G4-01",
            checks: &["dock-snap:edge=left,pin=off"],
        },
        Target::Exact {
            case: "G4-01",
            checks: &["tray-icon-present", "tray-icon-present"],
        },
        Target::Manual {
            case: "P12-M01",
            scenario: "no-such-scenario",
        },
        Target::Manual {
            case: "P12-M99",
            scenario: "pinyin-100",
        },
        Target::Gap { planned: "" },
        Target::Retired {
            decision: "RETIRE-ELSEWHERE",
        },
    ] {
        assert!(!errors(bad).is_empty(), "{bad:?} was accepted");
    }
    // A recorded decision that does not list this obligation authorizes nothing.
    let other = owner_errors(
        &[Obligation {
            source: "P08-M12",
            assertion: "supported-builds",
            param: "",
            target: Target::Retired {
                decision: "RETIRE-P08-M12-ACCESSIBILITY",
            },
        }],
        &owners,
    );
    assert!(!other.is_empty());
}
