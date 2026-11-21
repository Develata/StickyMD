//! Source/version binding independent of ZIP processing or release authority.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use crate::{integrity, repository};
use std::path::Path;

pub(super) fn source(root: &Path) -> Result<String, String> {
    let source = repository::command_text(root, "git", &["rev-parse", "HEAD"])?;
    integrity::validate_hex(&source, 40, "source commit")?;
    Ok(source.to_ascii_lowercase())
}

pub(super) fn approved_source(
    head: &str,
    approved: &str,
    version: &str,
    tag: &str,
) -> Result<(), String> {
    integrity::validate_hex(approved, 40, "SourceSha")?;
    if !head.eq_ignore_ascii_case(approved) {
        return Err(format!(
            "Checked-out source {head} does not match approved source {approved}"
        ));
    }
    release_tag(version, tag)
}

pub(super) fn release_tag(version: &str, tag: &str) -> Result<(), String> {
    if !tag.eq_ignore_ascii_case(&format!("v{version}")) {
        return Err(format!("Release tag {tag} does not match v{version}"));
    }
    Ok(())
}

pub(crate) fn readme_source(text: &str, source: &str) -> Result<(), String> {
    integrity::validate_hex(source, 40, "source commit")?;
    let bindings = text
        .trim_start_matches('\u{feff}')
        .lines()
        .filter(|line| line.to_ascii_lowercase().starts_with("source commit:"))
        .collect::<Vec<_>>();
    if bindings.len() != 1 || !bindings[0].eq_ignore_ascii_case(&format!("Source commit: {source}"))
    {
        return Err(format!(
            "Packaged README does not uniquely bind source commit {source}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_and_version_fail_closed_without_implying_candidate_authority() {
        let head = "a".repeat(40);
        approved_source(&head, &head.to_uppercase(), "0.1.0", "v0.1.0").unwrap();
        for source in ["", "abcd", &"b".repeat(40)] {
            assert!(approved_source(&head, source, "0.1.0", "v0.1.0").is_err());
        }
        assert!(approved_source(&head, &head, "0.1.0", "v0.2.0").is_err());
        let line = format!("Source commit: {head}\r\n");
        readme_source(&line, &head).unwrap();
        for text in [
            "missing".to_owned(),
            line.replace('a', "b"),
            format!("{line}{line}"),
            format!("prefix {line}"),
        ] {
            assert!(readme_source(&text, &head).is_err());
        }
    }
}
