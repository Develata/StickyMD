//! One instance-set predicate for recording, importing and readiness.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#manual-observation-log

use super::{log::Instance, registry::InstanceRule};

pub(in crate::qualification) fn count(rule: InstanceRule) -> usize {
    match rule {
        InstanceRule::FirstRunDeclared => 0,
        InstanceRule::TwoCopies(_) => 2,
        InstanceRule::Launched(_) | InstanceRule::Preserved(_) => 1,
    }
}

/// `expected` is supplied when the candidate binding is available. Parsing still
/// checks the count, PID uniqueness and digest syntax without that binding.
pub(in crate::qualification) fn validate(
    rule: InstanceRule,
    instances: &[Instance],
    expected: Option<&str>,
) -> Result<(), String> {
    if instances.len() != count(rule) {
        return Err(format!(
            "expected {} recorder-launched instances, found {}",
            count(rule),
            instances.len()
        ));
    }
    for (index, instance) in instances.iter().enumerate() {
        if instance.pid == 0
            || instances[..index]
                .iter()
                .any(|other| other.pid == instance.pid)
        {
            return Err("instance PIDs must be nonzero and distinct".to_owned());
        }
        crate::integrity::validate_sha256(&instance.exe_sha256, "instance EXE")?;
        if let Some(expected) = expected
            && instance.exe_sha256 != expected
        {
            return Err(format!(
                "observed EXE {}, not the candidate EXE {expected}",
                instance.exe_sha256
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::registry::Fixture;
    use super::*;

    #[test]
    fn zero_one_two_instances_share_count_pid_and_digest_checks() {
        let digest = "a".repeat(64);
        let first = Instance {
            exe_sha256: digest.clone(),
            pid: 1,
        };
        let second = Instance {
            exe_sha256: digest.clone(),
            pid: 2,
        };
        let rules = [
            InstanceRule::FirstRunDeclared,
            InstanceRule::Launched(Fixture::Blank),
            InstanceRule::TwoCopies(Fixture::Rendering),
        ];
        let valid = [
            vec![],
            vec![first.clone()],
            vec![first.clone(), second.clone()],
        ];
        for (index, rule) in rules.into_iter().enumerate() {
            for (length, instances) in valid.iter().enumerate() {
                assert_eq!(
                    validate(rule, instances, Some(&digest)).is_ok(),
                    index == length
                );
            }
        }
        let rule = InstanceRule::TwoCopies(Fixture::Rendering);
        for bad in [
            vec![first.clone(), first.clone()],
            vec![
                first.clone(),
                Instance {
                    pid: 0,
                    ..second.clone()
                },
            ],
            vec![
                first.clone(),
                Instance {
                    exe_sha256: "b".repeat(64),
                    ..second.clone()
                },
            ],
            vec![
                first,
                Instance {
                    exe_sha256: "invalid".to_owned(),
                    ..second
                },
            ],
        ] {
            assert!(validate(rule, &bad, Some(&digest)).is_err());
        }
    }
}
