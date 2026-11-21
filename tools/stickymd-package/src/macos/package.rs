//! Building and checking the macOS ZIP on a Mac: stage `StickyMD/` with the signed bundle and the
//! notices, archive it with `ditto`, then take the archive apart again and check every file, the
//! signature and the architecture of what came out.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-release-artifact

use super::content::{self, APP, FILES, README, Source};
use crate::macos::bundle::{app_inputs, bundle_hash};
use crate::notices;
use crate::platform::Platform;
use crate::temporary::TemporaryDirectory;
use crate::{integrity, json::escape as escape_json, repository};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What a checked package holds, for the workflow log and the candidate receipt.
pub struct Facts {
    pub zip: PathBuf,
    pub zip_sha256: String,
    pub source_commit: String,
    pub executable_sha256: String,
    pub bundle_sha256: String,
    pub app_inputs: String,
    /// The signing kind observed on the extracted bundle.
    pub signing: &'static str,
    /// The SHA-256 of each file the SBOM must describe, from the same extraction.
    pub members: std::collections::BTreeMap<&'static str, String>,
}

impl Facts {
    pub fn json(&self) -> String {
        format!(
            "{{\"zip\":\"{}\",\"zip_sha256\":\"{}\",\"source_commit\":\"{}\",\"app_executable_sha256\":\"{}\",\"app_bundle_manifest_sha256\":\"{}\",\"app_inputs\":\"{}\",\"signing\":\"{}\"}}",
            escape_json(&self.zip.display().to_string()),
            self.zip_sha256,
            self.source_commit,
            self.executable_sha256,
            self.bundle_sha256,
            self.app_inputs,
            self.signing
        )
    }
}

/// Runs a system tool and returns its trimmed stdout, or its stderr as the error.
pub fn run(program: &str, arguments: &[&std::ffi::OsStr]) -> Result<String, String> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(format!(
            "{program} {arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// The bundle as `build-app.sh` must have made it: a valid Info.plist of this version with the
/// product input fingerprint, an arm64-only executable, an intact signature, and that signature
/// ad-hoc, which is observed rather than assumed.
fn check_app(app: &Path, version: &str) -> Result<(String, &'static str), String> {
    let plist = app.join("Contents/Info.plist");
    run("/usr/bin/plutil", &["-lint".as_ref(), plist.as_os_str()])?;
    let short_version = run(
        "/usr/bin/plutil",
        &[
            "-extract".as_ref(),
            "CFBundleShortVersionString".as_ref(),
            "raw".as_ref(),
            plist.as_os_str(),
        ],
    )?;
    if short_version != version {
        return Err(format!(
            "the bundle is version {short_version}, the workspace {version}"
        ));
    }
    let inputs = app_inputs(app)?.ok_or("the bundle records no product input fingerprint")?;
    let executable = app.join("Contents/MacOS/StickyMD");
    let architectures = run(
        "/usr/bin/lipo",
        &["-archs".as_ref(), executable.as_os_str()],
    )?;
    if architectures != "arm64" {
        return Err(format!(
            "expected an arm64-only executable, found {architectures}"
        ));
    }
    run(
        "/usr/bin/codesign",
        &["--verify".as_ref(), "--strict".as_ref(), app.as_os_str()],
    )?;
    // `codesign -dv` writes its report to stderr.
    let display = Command::new("/usr/bin/codesign")
        .args(["-dv", "--verbose=2"])
        .arg(app)
        .output()
        .map_err(|error| format!("cannot run codesign: {error}"))?;
    if !display.status.success() {
        return Err("codesign -dv failed".to_owned());
    }
    let signing = content::ad_hoc_signature(&String::from_utf8_lossy(&display.stderr))?;
    Ok((inputs, signing))
}

fn commit_and_version(root: &Path, allow_dirty: bool) -> Result<(String, String), String> {
    if !allow_dirty
        && !repository::command_text(root, "git", &["status", "--porcelain"])?.is_empty()
    {
        return Err("the macOS package is built from a clean worktree".to_owned());
    }
    let commit =
        repository::command_text(root, "git", &["rev-parse", "HEAD"])?.to_ascii_lowercase();
    Ok((commit, repository::workspace_version(root)?))
}

/// The product input fingerprint of this checkout, computed the way `build-app.sh` computes it
/// before it builds: in the repository, with only `HOME` and a fixed `PATH`.
fn product_inputs(root: &Path) -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME is not set")?;
    let cargo = std::env::var_os("CARGO")
        .map(PathBuf::from)
        .ok_or("run through cargo, so the toolchain's cargo is known")?;
    let cargo_dir = cargo.parent().ok_or("cargo has no directory")?;
    let output = Command::new(&cargo)
        .current_dir(root)
        .env_clear()
        .env("HOME", &home)
        .env(
            "PATH",
            format!("/usr/bin:/bin:/usr/sbin:/sbin:{}", cargo_dir.display()),
        )
        .args([
            "run",
            "--quiet",
            "--locked",
            "-p",
            "stickymd-package",
            "--",
            "app-inputs",
            "--home",
        ])
        .arg(&home)
        .output()
        .map_err(|error| format!("cannot compute the product inputs: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cannot compute the product inputs: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Stages the package in a private directory beside `output`, archives and checks it there, and
/// only then moves the ZIP into `output`; an existing ZIP of that name is refused, not replaced,
/// and a failure leaves nothing behind. An exact package (`allow_dirty` false) must hold the
/// bundle this commit builds: its recorded product inputs must equal this checkout's.
pub fn build(root: &Path, app: &Path, output: &Path, allow_dirty: bool) -> Result<Facts, String> {
    let (commit, version) = commit_and_version(root, allow_dirty)?;
    let (inputs, _) = check_app(app, &version)?;
    if !allow_dirty {
        let expected = product_inputs(root)?;
        if inputs != expected {
            return Err(format!(
                "the bundle was built from product inputs {inputs}, this commit has {expected}"
            ));
        }
    }
    let zip = output.join(Platform::MacOsArm64.archive_name(&version));
    if fs::symlink_metadata(&zip).is_ok() {
        return Err(format!("{} already exists", zip.display()));
    }
    let private = output.join(format!(".stickymd-macos-package-{}", std::process::id()));
    fs::create_dir(&private)
        .map_err(|error| format!("cannot create {}: {error}", private.display()))?;
    let outcome = package_in(root, app, &private, &commit, &version, allow_dirty).and_then(
        |(staged, mut facts)| {
            fs::rename(&staged, &zip).map_err(|error| error.to_string())?;
            facts.zip = zip.clone();
            Ok(facts)
        },
    );
    let cleanup = fs::remove_dir_all(&private);
    let facts = outcome?;
    cleanup.map_err(|error| format!("cannot remove {}: {error}", private.display()))?;
    Ok(facts)
}

/// Builds and checks the ZIP inside `private`; returns its path there.
fn package_in(
    root: &Path,
    app: &Path,
    private: &Path,
    commit: &str,
    version: &str,
    allow_dirty: bool,
) -> Result<(PathBuf, Facts), String> {
    let zip = private.join(Platform::MacOsArm64.archive_name(version));
    let staging = TemporaryDirectory::new("macos-package")?;
    let directory = staging.path().join("StickyMD");
    fs::create_dir(&directory).map_err(|error| error.to_string())?;
    // A plain copy: no extended attributes, quarantine or resource forks travel with the bundle.
    run(
        "/usr/bin/ditto",
        &[
            "--norsrc".as_ref(),
            "--noextattr".as_ref(),
            "--noqtn".as_ref(),
            app.as_os_str(),
            staging.path().join(APP).as_os_str(),
        ],
    )?;
    for (name, source) in FILES {
        let path = staging.path().join(name);
        fs::create_dir_all(path.parent().ok_or("a package file has no parent")?)
            .map_err(|error| error.to_string())?;
        match source {
            Source::Readme => fs::write(&path, content::readme(version, commit, !allow_dirty))
                .map_err(|error| error.to_string())?,
            Source::License(relative) => {
                fs::write(&path, notices::normalized_license(&root.join(relative))?)
                    .map_err(|error| error.to_string())?;
            }
            Source::Notices => notices::generate_with_output(
                root,
                &path,
                Platform::MacOsArm64,
                &mut std::io::sink(),
            )?,
        }
    }
    run(
        "/usr/bin/ditto",
        &[
            "-c".as_ref(),
            "-k".as_ref(),
            "--norsrc".as_ref(),
            "--keepParent".as_ref(),
            directory.as_os_str(),
            zip.as_os_str(),
        ],
    )?;
    staging.close()?;
    let facts = verify(&zip, Some(commit), version)?;
    Ok((zip, facts))
}

/// Takes `zip` apart in a fresh directory and checks it holds exactly the inventory, names
/// `expected_commit` (when given) in its README, and carries a valid arm64 ad-hoc bundle.
pub fn verify(zip: &Path, expected_commit: Option<&str>, version: &str) -> Result<Facts, String> {
    // Every fact below comes from one private copy, so a file replaced meanwhile cannot mix in.
    let snapshot = TemporaryDirectory::new("macos-package-snapshot")?;
    let copy = snapshot.path().join("package.zip");
    fs::copy(zip, &copy).map_err(|error| format!("cannot snapshot {}: {error}", zip.display()))?;
    let zip_sha256 = integrity::sha256(&copy)?;
    super::listing::check_archive(&copy)?;
    let extracted = snapshot.path().join("extracted");
    run(
        "/usr/bin/ditto",
        &[
            "-x".as_ref(),
            "-k".as_ref(),
            copy.as_os_str(),
            extracted.as_os_str(),
        ],
    )?;
    let mut files = BTreeSet::new();
    collect_files(&extracted, &extracted, &mut files)?;
    content::check_files(&files)?;
    let readme = fs::read_to_string(extracted.join(README))
        .map_err(|error| format!("cannot read the package README: {error}"))?;
    let source_commit = content::source_commit(&readme)?;
    if let Some(expected) = expected_commit
        && !source_commit.eq_ignore_ascii_case(expected)
    {
        return Err(format!(
            "the package names source {source_commit}, expected {expected}"
        ));
    }
    // The README is generated text: it must be exactly what this version and source give.
    if readme != content::readme(version, &source_commit, true)
        && readme != content::readme(version, &source_commit, false)
    {
        return Err("the package README is not the one this version generates".to_owned());
    }
    let app = extracted.join(APP);
    let (app_inputs, signing) = check_app(&app, version)?;
    let facts = Facts {
        zip: zip.to_owned(),
        zip_sha256,
        source_commit,
        executable_sha256: integrity::sha256(&app.join("Contents/MacOS/StickyMD"))?,
        bundle_sha256: bundle_hash(&app)?,
        app_inputs,
        signing,
        members: content::member_hashes(&extracted)?,
    };
    snapshot.close()?;
    Ok(facts)
}

/// Regular files under `directory`, relative to `root` with `/`; a link or special file fails.
fn collect_files(root: &Path, directory: &Path, into: &mut BTreeSet<String>) -> Result<(), String> {
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        let kind = fs::symlink_metadata(&path)
            .map_err(|error| error.to_string())?
            .file_type();
        let relative = path
            .strip_prefix(root)
            .map_err(|error| error.to_string())?
            .to_str()
            .ok_or("a package path is not UTF-8")?
            .to_owned();
        if kind.is_dir() {
            // An empty directory has no file to show it, so each one is checked on its own.
            if !content::is_inventory_directory(&relative) {
                return Err(format!(
                    "the package holds an unexpected directory {relative}"
                ));
            }
            collect_files(root, &path, into)?;
        } else if kind.is_file() {
            into.insert(relative);
        } else {
            return Err(format!(
                "the package holds a link or special file: {relative}"
            ));
        }
    }
    Ok(())
}
