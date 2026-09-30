//! Bind a local native-runtime check to Cargo's current build artifact observation.
//! plan_ref: docs/plan/11_testing_and_release.md#modular-headless-ci

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::release::json;

pub(super) fn run(root: &Path, args: &[&str]) -> Result<PathBuf, String> {
    // Cargo reports its actual executable, respecting target-dir and target settings.
    // This local adapter never asks the formal candidate resolver for old exact bytes.
    let output = Command::new("cargo")
        .args(args)
        .current_dir(root)
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("cannot start local Release build: {error}"))?;
    if !output.status.success() {
        return Err(format!("local Release build failed with {}", output.status));
    }
    let text =
        std::str::from_utf8(&output.stdout).map_err(|_| "non-UTF-8 local build artifact output")?;
    let manifest = root
        .join("apps/stickymd-win/Cargo.toml")
        .canonicalize()
        .map_err(|error| format!("cannot resolve local Windows package manifest: {error}"))?;
    executable(text, &manifest)
}

fn executable(text: &str, expected_manifest: &Path) -> Result<PathBuf, String> {
    let mut executable = None;
    let mut finished = false;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        if finished {
            return Err("local build produced records after its completion".to_owned());
        }
        let record = json::parse(line)?;
        let reason = record.field("reason")?.string()?;
        if reason == "build-finished" {
            if finished || record.field("success")? != &json::Value::Bool(true) {
                return Err("local build did not report a unique successful completion".to_owned());
            }
            finished = true;
            continue;
        }
        if reason != "compiler-artifact" {
            continue;
        }
        let target = record.field("target")?;
        if target.field("name")?.string()? != "stickymd-win" {
            continue;
        }
        if !target
            .field("kind")?
            .array()?
            .iter()
            .any(|kind| kind.string() == Ok("bin"))
        {
            continue;
        }
        let manifest = PathBuf::from(record.field("manifest_path")?.string()?)
            .canonicalize()
            .map_err(|error| format!("cannot resolve Cargo artifact manifest: {error}"))?;
        if manifest != expected_manifest {
            return Err("local build artifact belongs to a different Windows package".to_owned());
        }
        let path = PathBuf::from(record.field("executable")?.string()?);
        if !path.is_absolute()
            || path.file_name().and_then(|name| name.to_str()) != Some("stickymd-win.exe")
        {
            return Err("local build returned an invalid Windows executable identity".to_owned());
        }
        if executable.replace(path).is_some() {
            return Err("local build returned multiple Windows executables".to_owned());
        }
    }
    if !finished {
        return Err("local build is missing its successful completion record".to_owned());
    }
    executable.ok_or_else(|| "local build did not report the Windows executable".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(path: &Path) -> String {
        format!(
            "{{\"reason\":\"compiler-artifact\",\"manifest_path\":\"{}\",\"target\":{{\"name\":\"stickymd-win\",\"kind\":[\"bin\"]}},\"executable\":\"{}\"}}",
            crate::evidence::escape_json(manifest().to_str().unwrap()),
            crate::evidence::escape_json(path.to_str().unwrap()),
        )
    }

    fn manifest() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("apps/stickymd-win/Cargo.toml")
            .canonicalize()
            .unwrap()
    }

    #[test]
    fn local_build_reads_actual_target_dir_and_unicode_executable_observation() {
        let path = std::env::temp_dir()
            .join("中文 space target")
            .join("release/stickymd-win.exe");
        let record = artifact(&path);
        assert_eq!(executable(&format!("{{\"reason\":\"build-script-executed\"}}\n{record}\n{{\"reason\":\"build-finished\",\"success\":true}}\n"), &manifest()).unwrap(), path);
    }

    #[test]
    fn local_build_rejects_missing_duplicate_invalid_or_unrelated_artifacts() {
        let path = std::env::temp_dir().join("release/stickymd-win.exe");
        let valid = format!(
            "{}\n{{\"reason\":\"build-finished\",\"success\":true}}",
            artifact(&path)
        );
        for text in [
            String::new(),
            "not JSON".to_owned(),
            artifact(&path),
            format!("{valid}\n{{\"reason\":\"build-finished\",\"success\":true}}"),
            format!("{valid}\n{valid}"),
            artifact(Path::new("release/stickymd-win.exe")),
            artifact(&path.with_file_name("other.exe")),
            valid.replace("stickymd-win\"", "other-app\""),
            valid.replace("[\"bin\"]", "[\"lib\"]"),
            valid.replace("\"success\":true", "\"success\":false"),
            valid.replace("\"success\":true", "\"success\":null"),
            valid.replace(
                &crate::evidence::escape_json(manifest().to_str().unwrap()),
                &crate::evidence::escape_json(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("Cargo.toml")
                        .to_str()
                        .unwrap(),
                ),
            ),
            valid.replace(
                &format!(
                    "\"{}\"",
                    crate::evidence::escape_json(path.to_str().unwrap())
                ),
                "null",
            ),
        ] {
            assert!(executable(&text, &manifest()).is_err(), "{text}");
        }
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "explicit isolated Cargo build; never launches the fixture executable"]
    fn actual_locked_local_build_ignores_stale_qualification_receipts() {
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };

        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                // Only the exclusively created fixture directory belongs to this test.
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "stickymd-local-build 中文 space-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Fixture(root);
        fs::create_dir_all(fixture.0.join("apps/stickymd-win/src")).unwrap();
        fs::create_dir_all(fixture.0.join("dist/evidence")).unwrap();
        fs::write(
            fixture.0.join("Cargo.toml"),
            "[workspace]\nmembers=[\"apps/stickymd-win\"]\nresolver=\"2\"\n",
        )
        .unwrap();
        fs::write(
            fixture.0.join("Cargo.lock"),
            "version = 4\n\n[[package]]\nname = \"stickymd-win\"\nversion = \"0.0.0\"\n",
        )
        .unwrap();
        fs::write(
            fixture.0.join("apps/stickymd-win/Cargo.toml"),
            "[package]\nname=\"stickymd-win\"\nversion=\"0.0.0\"\nedition=\"2024\"\n",
        )
        .unwrap();
        fs::write(
            fixture.0.join("apps/stickymd-win/src/main.rs"),
            "fn main() {}\n",
        )
        .unwrap();
        for name in ["release-source-freeze.json", "release-candidate.json"] {
            fs::write(
                fixture.0.join("dist/evidence").join(name),
                "malformed stale receipt",
            )
            .unwrap();
        }
        let target = fixture.0.join("target 中文 space");
        let args = [
            "build",
            "-p",
            "stickymd-win",
            "--release",
            "--locked",
            "--offline",
            "--target-dir",
            target.to_str().unwrap(),
            "--message-format=json-render-diagnostics",
        ];
        let path = run(&fixture.0, &args).unwrap();
        assert_eq!(
            path.canonicalize().unwrap(),
            target
                .join("release/stickymd-win.exe")
                .canonicalize()
                .unwrap()
        );
        for name in ["release-source-freeze.json", "release-candidate.json"] {
            assert_eq!(
                fs::read_to_string(fixture.0.join("dist/evidence").join(name)).unwrap(),
                "malformed stale receipt"
            );
        }
    }
}
