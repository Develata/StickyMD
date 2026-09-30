//! Validate remote observations without networking, mutation, receipts or authorization.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

#[cfg(test)]
mod tests;

use super::{cli::RemoteStateOptions, json::Value};
use crate::integrity;
use std::{fs, io, path::Path};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    Tag,
    Draft,
}

impl Kind {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "tag" => Ok(Self::Tag),
            "draft" => Ok(Self::Draft),
            _ => Err("remote state kind must be tag or draft".into()),
        }
    }
}

pub(super) fn execute(root: &Path, options: &RemoteStateOptions) -> Result<(), String> {
    super::identity::release_tag(&crate::repository::workspace_version(root)?, &options.tag)?;
    let text = if options.input == Path::new("-") {
        io::read_to_string(io::stdin().lock())
    } else {
        fs::read_to_string(&options.input)
    }
    .map_err(|error| format!("cannot read remote state observation: {error}"))?;
    let exists = verify(options, &text)?;
    println!("{{\"exists\":{exists}}}");
    Ok(())
}

fn verify(options: &RemoteStateOptions, text: &str) -> Result<bool, String> {
    integrity::validate_hex(&options.source, 40, "approved source SHA")?;
    let normalized = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let (headers, body) = normalized
        .split_once("\n\n")
        .ok_or("missing HTTP response headers/body boundary")?;
    let mut status = headers
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace();
    if !matches!(
        status.next(),
        Some("HTTP/1.0" | "HTTP/1.1" | "HTTP/2" | "HTTP/2.0")
    ) {
        return Err("missing HTTP response status".into());
    }
    let code = status.next().ok_or("missing HTTP status code")?;
    if options.kind == Kind::Tag && code == "404" && options.query_exit == 1 {
        return missing(options);
    }
    if code != "200" || options.query_exit != 0 {
        return Err(format!(
            "remote query failed: HTTP {code}, exit {}",
            options.query_exit
        ));
    }
    let document = super::json::parse(body)?;
    match options.kind {
        Kind::Tag => {
            if document.field("ref")?.string()? != format!("refs/tags/{}", options.tag) {
                return Err("observed tag ref does not match requested release tag".into());
            }
            let sha = document.field("object")?.field("sha")?.string()?;
            integrity::validate_hex(sha, 40, "remote tag SHA")?;
            if sha != options.source {
                return Err(format!(
                    "Existing tag points to {sha}, expected {}",
                    options.source
                ));
            }
        }
        Kind::Draft => {
            // Drafts have pending tags: REST releases/tags cannot reliably find them.
            // GraphQL may return HTTP 200 with errors, or an inaccessible repository.
            if let Value::Object(fields) = &document
                && let Some(errors) = fields.get("errors")
                && !errors.array()?.is_empty()
            {
                return Err("GitHub GraphQL query returned errors".into());
            }
            let release = document
                .field("data")?
                .field("repository")?
                .field("release")?;
            if *release == Value::Null {
                return missing(options);
            }
            if release.field("tagName")?.string()? != options.tag {
                return Err("observed draft does not match requested release tag".into());
            }
            if *release.field("isDraft")? != Value::Bool(true) {
                return Err(
                    "Refusing to modify a published release: expected an unpublished draft".into(),
                );
            }
        }
    }
    Ok(true)
}

fn missing(options: &RemoteStateOptions) -> Result<bool, String> {
    if options.allow_missing {
        Ok(false)
    } else {
        Err("required remote tag or draft does not exist".into())
    }
}
