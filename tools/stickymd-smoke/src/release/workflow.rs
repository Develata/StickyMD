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
    if workflow != "release" {
        return Err(format!(
            "remote workflow is `{workflow}`, expected `release`"
        ));
    }
    Ok(())
}

pub(super) fn verify_input(source: &str, input: &Path) -> Result<(), String> {
    integrity::validate_hex(source, 40, "approved source SHA")?;
    let text = if input == Path::new("-") {
        io::read_to_string(io::stdin().lock())
    } else {
        fs::read_to_string(input)
    }
    .map_err(|error| format!("cannot read workflow observation: {error}"))?;
    verify_api_json(source, &text)?;
    println!("WORKFLOW_IDENTITY=PASS");
    Ok(())
}

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
