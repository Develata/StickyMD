//! Which target role an observation covers. Recording, import and readiness all call
//! `match_role` on the facts stored in the observation, so the role is always recomputed
//! and never taken from the observer's word.

use super::target::{Machine, Role, Target};

/// Facts the recorder collects on the tested machine: `DisplayVersion`, `CurrentBuild`,
/// `EditionID` (`REG_SZ`) and `UBR` (`REG_DWORD`) from the 64-bit view of
/// `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`, and `IsWow64Process2`'s native
/// machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::qualification) struct Facts {
    pub(in crate::qualification) display_version: String,
    pub(in crate::qualification) current_build: u32,
    pub(in crate::qualification) ubr: u32,
    pub(in crate::qualification) edition_id: String,
    pub(in crate::qualification) native_machine: Machine,
}

impl Facts {
    pub(in crate::qualification) fn build(&self) -> String {
        format!("{}.{}", self.current_build, self.ubr)
    }
}

/// The unique role whose feature version, build branch, approved builds, SKU and native
/// machine all equal `facts`. A match means only that the system is one of the public
/// service builds the USER approved; it says nothing about Insider enrolment and does not
/// prove that two machines on the same build are otherwise equivalent.
pub(in crate::qualification) fn match_role(facts: &Facts, target: &Target) -> Result<Role, String> {
    let matches = Role::ALL
        .into_iter()
        .filter(|role| {
            let wanted = target.role(*role);
            facts.display_version == wanted.feature_version
                && facts.current_build == wanted.build_branch
                && wanted.approved_ubrs.contains(&facts.ubr)
                && wanted.edition_ids.contains(&facts.edition_id)
                && facts.native_machine == wanted.native_machine
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [role] => Ok(*role),
        [] => Err(format!(
            "Windows {} build {} ({} on {}) matches no approved target role",
            facts.display_version,
            facts.build(),
            facts.edition_id,
            facts.native_machine.as_str()
        )),
        _ => Err(format!(
            "Windows {} build {} matches more than one target role",
            facts.display_version,
            facts.build()
        )),
    }
}

#[cfg(test)]
pub(in crate::qualification) mod fixtures {
    use super::Facts;
    use crate::qualification::manual_observation::target::Machine;

    pub(in crate::qualification) fn current() -> Facts {
        Facts {
            display_version: "25H2".to_owned(),
            current_build: 26200,
            ubr: 7019,
            edition_id: "Professional".to_owned(),
            native_machine: Machine::Amd64,
        }
    }

    pub(in crate::qualification) fn previous() -> Facts {
        Facts {
            display_version: "24H2".to_owned(),
            current_build: 26100,
            ubr: 6899,
            edition_id: "Professional".to_owned(),
            native_machine: Machine::Amd64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{current, previous};
    use super::match_role;
    use crate::qualification::manual_observation::target::{Machine, Role, fixtures::target};

    #[test]
    fn each_approved_environment_matches_exactly_its_role() {
        let target = target();
        assert_eq!(match_role(&current(), &target), Ok(Role::Current));
        assert_eq!(match_role(&previous(), &target), Ok(Role::Previous));
    }

    #[test]
    fn any_differing_fact_matches_no_role() {
        let target = target();
        let mut cases = Vec::new();
        let mut facts = current();
        facts.ubr = 7020; // a later, unapproved cumulative update
        cases.push(facts);
        let mut facts = current();
        facts.display_version = "24H2".to_owned(); // feature version and branch disagree
        cases.push(facts);
        let mut facts = current();
        facts.edition_id = "Enterprise".to_owned();
        cases.push(facts);
        let mut facts = current();
        facts.native_machine = Machine::Arm64; // an x64 process on an ARM64 host
        cases.push(facts);
        let mut facts = previous();
        facts.edition_id = "Core".to_owned(); // approved for current, not for previous
        cases.push(facts);
        for facts in cases {
            assert!(match_role(&facts, &target).is_err(), "{facts:?}");
        }
    }

    #[test]
    fn an_environment_matching_both_roles_is_rejected() {
        let mut target = target();
        target.roles[1] = target.roles[0].clone();
        target.roles[1].role = Role::Previous;
        let error = match_role(&current(), &target).unwrap_err();
        assert!(error.contains("more than one"), "{error}");
    }
}
