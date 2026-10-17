//! Test executables from `cargo test --no-run --message-format=json`.
//!
//! Every stdout line is one JSON message. A test executable is a `compiler-artifact` with
//! `profile.test` and a non-null `executable`; its package comes from `package_id`, a Package ID
//! Specification (`path+file:///…/dir#name@version`, or `…/name#version` when the name is the
//! last path segment). The stream must end with a successful `build-finished`, so a truncated
//! or failed build is an error rather than fewer executables.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::path::PathBuf;

use super::OwnerTarget;
use crate::release::json::{self, Value};

/// One built test executable and where Cargo would run it from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TestArtifact {
    pub(crate) package: String,
    pub(crate) kind: Vec<String>,
    pub(crate) name: String,
    pub(crate) executable: PathBuf,
    /// The package directory: Cargo runs a test executable from it.
    pub(crate) package_dir: PathBuf,
}

impl TestArtifact {
    /// Whether this executable holds `target`'s unit tests.
    pub(crate) fn is(&self, package: &str, target: OwnerTarget) -> bool {
        self.package == package
            && match target {
                OwnerTarget::Lib => self.kind == ["lib"],
                OwnerTarget::Bin(name) => self.kind == ["bin"] && self.name == name,
            }
    }
}

/// The test executables of a complete, successful build, in message order.
pub(crate) fn parse_artifacts(stdout: &str) -> Result<Vec<TestArtifact>, String> {
    let mut artifacts = Vec::new();
    let mut finished = false;
    for line in stdout.lines().filter(|line| !line.is_empty()) {
        if finished {
            return Err("Cargo printed a message after build-finished".to_owned());
        }
        let message = json::parse(line).map_err(|error| format!("Cargo message: {error}"))?;
        match message.field("reason")?.string()? {
            "build-finished" => {
                if message.field("success")? != &Value::Bool(true) {
                    return Err("the Cargo test build did not succeed".to_owned());
                }
                finished = true;
            }
            "compiler-artifact" => {
                if let Some(artifact) = test_artifact(&message)? {
                    artifacts.push(artifact);
                }
            }
            _ => {}
        }
    }
    if !finished {
        return Err("Cargo output ended before build-finished".to_owned());
    }
    Ok(artifacts)
}

fn test_artifact(message: &Value) -> Result<Option<TestArtifact>, String> {
    if message.field("profile")?.field("test")? != &Value::Bool(true) {
        return Ok(None);
    }
    let Some(executable) = message.field("executable")?.optional_string()? else {
        return Ok(None);
    };
    let target = message.field("target")?;
    let manifest = PathBuf::from(message.field("manifest_path")?.string()?);
    Ok(Some(TestArtifact {
        package: package_name(message.field("package_id")?.string()?)?.to_owned(),
        kind: target
            .field("kind")?
            .array()?
            .iter()
            .map(|kind| kind.string().map(str::to_owned))
            .collect::<Result<_, _>>()?,
        name: target.field("name")?.string()?.to_owned(),
        executable: PathBuf::from(executable),
        package_dir: manifest
            .parent()
            .ok_or("Cargo manifest path has no directory")?
            .to_path_buf(),
    }))
}

/// The package name a Package ID Specification names.
fn package_name(spec: &str) -> Result<&str, String> {
    let invalid = || format!("unrecognized Cargo package id {spec:?}");
    let (url, fragment) = spec.split_once('#').ok_or_else(invalid)?;
    let name = match fragment.split_once('@') {
        Some((name, _version)) => name,
        // A bare fragment is a version when it starts with a digit, otherwise a name.
        None if fragment.starts_with(|c: char| c.is_ascii_digit()) => {
            url.rsplit('/').next().ok_or_else(invalid)?
        }
        None => fragment,
    };
    (!name.is_empty() && !name.contains(['/', ':', '#', '@']))
        .then_some(name)
        .ok_or_else(invalid)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{OwnerTarget, package_name, parse_artifacts};

    /// Messages of `cargo test -p stickymd-win --no-run --message-format=json` on Rust
    /// 1.97.1, reduced to the fields read here.
    const BUILD: &str = concat!(
        r#"{"reason":"compiler-artifact","package_id":"path+file:///E:/gitclone/StickyMD/crates/stickymd-render#0.1.3","manifest_path":"E:\\gitclone\\StickyMD\\crates\\stickymd-render\\Cargo.toml","target":{"kind":["lib"],"name":"stickymd_render"},"profile":{"test":false},"executable":null}"#,
        "\n",
        r#"{"reason":"build-script-executed","package_id":"path+file:///E:/gitclone/StickyMD/apps/stickymd-win#0.1.3"}"#,
        "\n",
        r#"{"reason":"compiler-artifact","package_id":"path+file:///E:/gitclone/StickyMD/apps/stickymd-win#0.1.3","manifest_path":"E:\\gitclone\\StickyMD\\apps\\stickymd-win\\Cargo.toml","target":{"kind":["bin"],"name":"stickymd-win"},"profile":{"test":true},"executable":"E:\\gitclone\\StickyMD\\target\\debug\\deps\\stickymd_win-fdb67d2525bbb692.exe"}"#,
        "\n",
        r#"{"reason":"compiler-artifact","package_id":"path+file:///E:/gitclone/StickyMD/crates/stickymd-render#0.1.3","manifest_path":"E:\\gitclone\\StickyMD\\crates\\stickymd-render\\Cargo.toml","target":{"kind":["test"],"name":"phase6_math"},"profile":{"test":true},"executable":"E:\\gitclone\\StickyMD\\target\\debug\\deps\\phase6_math-0bd34b70c6463a04.exe"}"#,
        "\n",
        r#"{"reason":"build-finished","success":true}"#,
        "\n",
    );

    #[test]
    fn test_executables_come_from_a_complete_build() {
        let artifacts = parse_artifacts(BUILD).unwrap();
        assert_eq!(artifacts.len(), 2);
        let win = &artifacts[0];
        assert!(win.is("stickymd-win", OwnerTarget::Bin("stickymd-win")));
        assert!(!win.is("stickymd-win", OwnerTarget::Lib));
        assert!(!win.is("stickymd-render", OwnerTarget::Bin("stickymd-win")));
        // Compared whole, so these Windows paths read the same on every host.
        assert_eq!(
            win.executable,
            PathBuf::from(
                r"E:\gitclone\StickyMD\target\debug\deps\stickymd_win-fdb67d2525bbb692.exe"
            )
        );
        assert_eq!(
            win.package_dir,
            Path::new(r"E:\gitclone\StickyMD\apps\stickymd-win\Cargo.toml")
                .parent()
                .unwrap()
        );
        // An integration test target is never a unit-test owner target.
        assert!(!artifacts[1].is("stickymd-render", OwnerTarget::Lib));
    }

    #[test]
    fn an_incomplete_failed_or_malformed_build_is_an_error() {
        let without_end = BUILD.replace("{\"reason\":\"build-finished\",\"success\":true}\n", "");
        let failed = BUILD.replace("\"success\":true", "\"success\":false");
        let after_end = format!("{BUILD}{{\"reason\":\"compiler-artifact\"}}\n");
        let truncated = &BUILD[..BUILD.len() / 2];
        for broken in [
            without_end.as_str(),
            &failed,
            &after_end,
            truncated,
            "not json\n",
        ] {
            assert!(parse_artifacts(broken).is_err(), "{broken}");
        }
    }

    #[test]
    fn package_ids_name_their_package() {
        for (spec, name) in [
            (
                "path+file:///E:/repo/apps/stickymd-win#0.1.3",
                "stickymd-win",
            ),
            (
                "path+file:///E:/repo/experiments/a#spike-a@0.1.0",
                "spike-a",
            ),
            ("path+file:///E:/repo/x#named", "named"),
        ] {
            assert_eq!(package_name(spec).unwrap(), name, "{spec}");
        }
        for broken in ["stickymd-win", "path+file:///E:/repo/x#@0.1.0", "x#a/b@1"] {
            assert!(package_name(broken).is_err(), "{broken}");
        }
    }
}
