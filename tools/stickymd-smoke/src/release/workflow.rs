//! Shared remote-run identity checks; observations do not authorize release or write receipts.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use crate::integrity;
use std::{fs, io, path::Path};

pub(crate) fn verify_run(
    source: &str,
    head: &str,
    conclusion: &str,
    workflow: &str,
) -> Result<(), String> {
    verify_named_run(source, head, conclusion, workflow, "release")
}

/// A run (or one attempt of it) of the workflow `expected` that succeeded on `source`.
pub(crate) fn verify_named_run(
    source: &str,
    head: &str,
    conclusion: &str,
    workflow: &str,
    expected: &str,
) -> Result<(), String> {
    integrity::validate_hex(source, 40, "approved source SHA")?;
    integrity::validate_hex(head, 40, "remote run head SHA")?;
    if head != source {
        return Err(format!(
            "remote run head {head} does not match approved source {source}"
        ));
    }
    if conclusion != "success" {
        return Err(format!("remote workflow conclusion is `{conclusion}`"));
    }
    if workflow != expected {
        return Err(format!(
            "remote workflow is `{workflow}`, expected `{expected}`"
        ));
    }
    Ok(())
}

/// The observation of a run of `workflow` read from `input` (`-` for stdin); with `attempt`, the
/// observation is that attempt
/// (`/actions/runs/<id>/attempts/<n>`) and must say so.
pub(super) fn verify_named_input(
    source: &str,
    input: &Path,
    workflow: &str,
    attempt: Option<u64>,
) -> Result<(), String> {
    integrity::validate_hex(source, 40, "approved source SHA")?;
    let text = if input == Path::new("-") {
        io::read_to_string(io::stdin().lock())
    } else {
        fs::read_to_string(input)
    }
    .map_err(|error| format!("cannot read workflow observation: {error}"))?;
    let document = super::json::parse(&text)?;
    if let Some(attempt) = attempt {
        let observed = document.field("run_attempt")?.unsigned()?;
        if observed != attempt {
            return Err(format!(
                "the observation is attempt {observed}, expected {attempt}"
            ));
        }
    }
    verify_named_run(
        source,
        document.field("head_sha")?.string()?,
        document.field("conclusion")?.string()?,
        document.field("name")?.string()?,
        workflow,
    )?;
    println!("WORKFLOW_IDENTITY=PASS");
    Ok(())
}

#[cfg(test)]
fn verify_api_json(source: &str, text: &str) -> Result<(), String> {
    let document = super::json::parse(text)?;
    verify_run(
        source,
        document.field("head_sha")?.string()?,
        document.field("conclusion")?.string()?,
        document.field("name")?.string()?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_identity_requires_full_matching_source_success_and_release_workflow() {
        let source = "a".repeat(40);
        verify_run(&source, &source, "success", "release").unwrap();
        for (expected, observed, conclusion, workflow) in [
            ("", source.as_str(), "success", "release"),
            (source.as_str(), "abc", "success", "release"),
            (
                source.as_str(),
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "success",
                "release",
            ),
            (source.as_str(), source.as_str(), "failure", "release"),
            (source.as_str(), source.as_str(), "cancelled", "release"),
            (source.as_str(), source.as_str(), "", "release"),
            (source.as_str(), source.as_str(), "success", "ci"),
        ] {
            assert!(verify_run(expected, observed, conclusion, workflow).is_err());
        }
    }

    #[test]
    fn api_observation_rejects_missing_duplicate_null_and_wrongly_typed_fields() {
        let source = "a".repeat(40);
        let valid = format!(r#"{{"head_sha":"{source}","conclusion":"success","name":"release"}}"#);
        verify_api_json(&source, &valid).unwrap();
        for invalid in [
            "{}".to_owned(),
            valid.replace("\"conclusion\":\"success\"", "\"conclusion\":null"),
            valid.replace("\"name\":\"release\"", "\"name\":123"),
            valid.replace(
                "\"name\":\"release\"",
                "\"name\":\"release\",\"name\":\"ci\"",
            ),
            valid.replace("\"head_sha\"", "\"headSha\""),
        ] {
            assert!(verify_api_json(&source, &invalid).is_err(), "{invalid}");
        }
    }
}
