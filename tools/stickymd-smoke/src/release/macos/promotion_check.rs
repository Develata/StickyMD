//! The promotion-check record: written by `release macos-promote` on a clean checkout of the
//! release commit R, after the candidate receipt, and read by `release macos-ready`. It binds the
//! promotion's re-checks (the attempt, the download by artifact id, the comparison with the
//! user's copy) to R and to the receipt they produced, while the receipt itself names neither R
//! nor a time, so promoting the same attempt again on R writes the same receipt (M5 gate review
//! B2).
//! plan_ref: docs/plan/11_testing_and_release.md#macos-release-artifact
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use crate::integrity;
use crate::release::json;

/// Where the record lies; reserved beside the candidate receipt and the reuse record.
pub(crate) const PATH: &str = crate::qualification::reserved_paths::MACOS_PROMOTION_CHECK;
const KIND: &str = "macos-promotion-check";

/// What a record states: the promotion was re-checked on `release_source` and produced the
/// receipt whose SHA-256 is `candidate_receipt_sha256`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Check {
    pub release_source: String,
    pub candidate_receipt_sha256: String,
}

/// The record's bytes, the one form `parse` accepts.
pub(crate) fn render(release: &str, receipt_sha256: &str) -> String {
    format!(
        "{{\"schema_version\":1,\"kind\":\"{KIND}\",\"release_source\":\"{}\",\"candidate_receipt_sha256\":\"{}\"}}\n",
        release.to_ascii_lowercase(),
        receipt_sha256.to_ascii_lowercase()
    )
}

/// The record in `bytes`, parsed strictly: exactly the bytes `render` writes for a full commit
/// and a SHA-256, so no other field, schema, kind, spelling or layout passes.
pub(crate) fn parse(bytes: &[u8]) -> Result<Check, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "it is not UTF-8".to_owned())?;
    let document = json::parse(text)?;
    if document.field("schema_version")?.unsigned()? != 1 {
        return Err("its schema_version is not 1".to_owned());
    }
    if document.field("kind")?.string()? != KIND {
        return Err(format!("its kind is not {KIND}"));
    }
    let release = document.field("release_source")?.string()?;
    integrity::validate_hex(release, 40, "its release_source")?;
    let receipt = document.field("candidate_receipt_sha256")?.string()?;
    integrity::validate_sha256(receipt, "its candidate_receipt_sha256")?;
    if text != render(release, receipt) {
        return Err("it is not in the form macos-promote writes".to_owned());
    }
    Ok(Check {
        release_source: release.to_owned(),
        candidate_receipt_sha256: receipt.to_owned(),
    })
}

/// Whether `bytes` are a promotion-check record at all, of any layout: a JSON object whose
/// `kind` is this record's. Promotion retires only such a file; anything else at the path is
/// someone's (fix-round review R1-9). Not a validity check: `parse` is.
pub(crate) fn is_record(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| json::parse(text).ok())
        .is_some_and(|document| {
            document
                .field("kind")
                .and_then(|kind| kind.string().map(|kind| kind == KIND))
                .unwrap_or(false)
        })
}

/// The record's bytes under `root`, `None` when there is none; anything but a regular file there
/// is an error.
#[cfg(target_os = "macos")]
pub(crate) fn load(root: &std::path::Path) -> Result<Option<Vec<u8>>, String> {
    let path = root.join(PATH);
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot inspect it: {error}")),
        Ok(metadata) if !metadata.file_type().is_file() => {
            Err("it is not a regular file".to_owned())
        }
        Ok(_) => std::fs::read(&path)
            .map(Some)
            .map_err(|error| format!("cannot read it: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_rendered_form_parses() {
        let release = "a".repeat(40);
        let digest = "b".repeat(64);
        let record = render(&release, &digest);
        assert_eq!(
            parse(record.as_bytes()).unwrap(),
            Check {
                release_source: release.clone(),
                candidate_receipt_sha256: digest.clone(),
            }
        );
        for (bad, words) in [
            (
                record.replace("\"schema_version\":1", "\"schema_version\":2"),
                "schema_version",
            ),
            (record.replace(KIND, "macos-promotion"), "kind"),
            (record.replace(&release, &"a".repeat(39)), "release_source"),
            (record.replace(&digest, &"B".repeat(64)), "form"),
            (record.replace("}\n", ",\"extra\":1}\n"), "form"),
            (record.replace('\n', ""), "form"),
            (record.replace(",\"kind\"", " ,\"kind\""), "form"),
            ("{}".to_owned(), "schema_version"),
            ("not json".to_owned(), ""),
        ] {
            let error = parse(bad.as_bytes()).unwrap_err();
            assert!(error.contains(words), "{bad}: {error}");
        }
        assert!(parse(&[0xff]).unwrap_err().contains("UTF-8"));
    }
}
