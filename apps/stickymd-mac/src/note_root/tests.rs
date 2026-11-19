//! The Note Root rule on real directories (P18-L01, L04–L10). Every test builds its own
//! `Applications` directories and home, so none reads or writes the real ones.

use super::*;
use crate::paths::{RuntimePaths, open_note_directory};
use crate::persistence::mac_io_ports;
use crate::platform::macos::instance::{InstanceDisposition, InstanceGuard};
use crate::test_support::unique_temp_path;
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use stickymd_app::config::RuntimeConfig;
use stickymd_app::ports::ConfigStorage;

/// A system `Applications`, a home with its own `Applications` and `Application Support`.
struct World {
    base: PathBuf,
    roots: Roots,
}

impl World {
    fn new(label: &str) -> Self {
        let base = unique_temp_path(label);
        fs::create_dir_all(base.join("system")).unwrap();
        fs::create_dir_all(base.join("home/Applications")).unwrap();
        fs::create_dir_all(base.join("home/Library/Application Support")).unwrap();
        let roots = Roots {
            system_applications: base.join("system"),
            home: Ok(base.join("home")),
        };
        Self { base, roots }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.base.join(relative)
    }

    /// `<directory>/<name>.app` with an executable inside; returns the executable.
    fn app(&self, directory: &str, name: &str) -> PathBuf {
        let macos = self
            .path(directory)
            .join(format!("{name}.app"))
            .join("Contents/MacOS");
        fs::create_dir_all(&macos).unwrap();
        let executable = macos.join("StickyMD");
        fs::write(&executable, b"").unwrap();
        executable
    }

    fn resolve(&self, executable: &Path) -> Result<NoteRoot, NoteRootError> {
        NoteRoot::resolve(executable, &self.roots)
    }

    fn support(&self) -> PathBuf {
        self.path("home/Library/Application Support")
    }

    /// Every path under `relative`, to show nothing was added.
    fn tree(&self, relative: &str) -> BTreeSet<PathBuf> {
        fn walk(path: &Path, into: &mut BTreeSet<PathBuf>) {
            into.insert(path.to_owned());
            if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir()) {
                for entry in fs::read_dir(path).unwrap() {
                    walk(&entry.unwrap().path(), into);
                }
            }
        }
        let mut paths = BTreeSet::new();
        walk(&self.path(relative), &mut paths);
        paths
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn installed_key(note_root: &NoteRoot) -> &str {
    match &note_root.mode {
        Mode::Installed { key } => key,
        other => panic!("expected the installed copy, found {other:?}"),
    }
}

fn primary(note_root: &NoteRoot) -> InstanceGuard {
    match InstanceGuard::acquire(&note_root.handle).unwrap() {
        InstanceDisposition::Primary(guard) => guard,
        InstanceDisposition::SecondarySignaled => panic!("expected the first copy"),
    }
}

fn is_secondary(note_root: &NoteRoot) -> bool {
    matches!(
        InstanceGuard::acquire(&note_root.handle).unwrap(),
        InstanceDisposition::SecondarySignaled
    )
}

#[test]
fn an_app_in_either_applications_is_installed_under_application_support() {
    let world = World::new("root-installed");
    for directory in ["system", "home/Applications"] {
        let note_root = world.resolve(&world.app(directory, "StickyMD")).unwrap();
        assert_eq!(installed_key(&note_root), "StickyMD");
        assert_eq!(note_root.path, world.support().join("StickyMD/StickyMD"));
        for level in ["StickyMD", "StickyMD/StickyMD"] {
            let mode = fs::metadata(world.support().join(level))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0, "{level} {mode:o}");
        }
        // Only the two levels: `note/` comes after the lock.
        assert_eq!(
            fs::read_dir(world.support().join("StickyMD/StickyMD"))
                .unwrap()
                .count(),
            0
        );
    }
}

/// `/Applications` and `/System/Volumes/Data/Applications` are one directory: identity, not the
/// path, decides. A home `Applications` that is a link to elsewhere counts as its target.
#[test]
fn a_candidate_is_compared_by_identity_after_links() {
    let world = World::new("root-identity");
    std::os::unix::fs::symlink(world.path("system"), world.path("alias")).unwrap();
    let roots = Roots {
        system_applications: world.path("alias"),
        home: Ok(world.path("home")),
    };
    let executable = world.app("system", "StickyMD");
    assert_eq!(
        installed_key(&NoteRoot::resolve(&executable, &roots).unwrap()),
        "StickyMD"
    );

    fs::remove_dir(world.path("home/Applications")).unwrap();
    fs::create_dir(world.path("elsewhere")).unwrap();
    std::os::unix::fs::symlink(world.path("elsewhere"), world.path("home/Applications")).unwrap();
    let linked = world.app("elsewhere", "StickyMD");
    assert_eq!(installed_key(&world.resolve(&linked).unwrap()), "StickyMD");
}

#[test]
fn anywhere_else_is_portable_next_to_the_app_and_creates_nothing() {
    let world = World::new("root-portable");
    let before = world.tree("home/Library");
    for directory in [
        "system/Tools",
        "Downloads/StickyMD",
        "private/var/folders/x/AppTranslocation/ABC/d",
    ] {
        let note_root = world.resolve(&world.app(directory, "StickyMD")).unwrap();
        assert_eq!(note_root.mode, Mode::Portable, "{directory}");
        assert_eq!(note_root.path, world.path(directory));
    }
    // A dangling home `Applications` is no hit either.
    fs::remove_dir(world.path("home/Applications")).unwrap();
    std::os::unix::fs::symlink(world.path("gone"), world.path("home/Applications")).unwrap();
    let note_root = world
        .resolve(&world.app("Downloads/StickyMD", "StickyMD"))
        .unwrap();
    assert_eq!(note_root.mode, Mode::Portable);
    assert_eq!(world.tree("home/Library"), before);
}

#[test]
fn a_bare_executable_is_a_development_build_next_to_itself() {
    let world = World::new("root-development");
    fs::create_dir(world.path("target")).unwrap();
    fs::write(world.path("target/stickymd"), b"").unwrap();
    let note_root = world.resolve(&world.path("target/stickymd")).unwrap();
    assert_eq!(note_root.mode, Mode::Development);
    assert_eq!(note_root.path, world.path("target"));
}

#[test]
fn an_unreadable_candidate_or_unknown_home_stops_unless_another_candidate_hits() {
    let mut world = World::new("root-undecidable");
    fs::create_dir_all(world.path("locked/Applications")).unwrap();
    world.roots.system_applications = world.path("locked/Applications");
    fs::set_permissions(world.path("locked"), fs::Permissions::from_mode(0o000)).unwrap();
    let elsewhere = world.resolve(&world.app("Downloads", "StickyMD"));
    let hit = world.resolve(&world.app("home/Applications", "StickyMD"));
    fs::set_permissions(world.path("locked"), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        matches!(elsewhere, Err(NoteRootError::Undecidable(_))),
        "{elsewhere:?}"
    );
    assert_eq!(installed_key(&hit.unwrap()), "StickyMD");

    world.roots.system_applications = world.path("system");
    world.roots.home = Err("no entry".into());
    assert!(matches!(
        world.resolve(&world.app("Downloads", "StickyMD")),
        Err(NoteRootError::Undecidable(_))
    ));
    assert!(matches!(
        world.resolve(&world.app("system", "StickyMD")),
        Err(NoteRootError::HomeUnknown(_))
    ));
}

#[test]
fn a_missing_application_support_is_not_created() {
    let world = World::new("root-no-support");
    fs::remove_dir(world.support()).unwrap();
    let result = world.resolve(&world.app("system", "StickyMD"));
    assert!(
        matches!(result, Err(NoteRootError::ApplicationSupportMissing(_))),
        "{result:?}"
    );
    assert!(!world.support().exists());
}

/// P18-L05: a link, a file or another user's directory at either level stops the start, and
/// nothing is added in or behind it.
#[test]
fn a_level_that_fails_the_check_is_refused_and_left_unchanged() {
    let world = World::new("root-refused");
    let executable = world.app("system", "StickyMD");
    fs::create_dir(world.path("target")).unwrap();
    std::os::unix::fs::symlink(world.path("target"), world.support().join("StickyMD")).unwrap();
    let before = world.tree("");
    let link = world.resolve(&executable);
    assert!(
        matches!(link, Err(NoteRootError::NotADirectory(_))),
        "{link:?}"
    );
    assert_eq!(world.tree(""), before);

    fs::remove_file(world.support().join("StickyMD")).unwrap();
    fs::create_dir(world.support().join("StickyMD")).unwrap();
    fs::write(world.support().join("StickyMD/StickyMD"), b"x").unwrap();
    let before = world.tree("");
    let file = world.resolve(&executable);
    assert!(
        matches!(file, Err(NoteRootError::NotADirectory(_))),
        "{file:?}"
    );
    assert_eq!(world.tree(""), before);

    let path = Path::new("/x");
    assert!(matches!(
        check_level(true, 0, 501, path),
        Err(NoteRootError::ForeignOwner(_))
    ));
    assert!(check_level(true, 501, 501, path).is_ok());
}

/// P18-L06: the copy that loses the lock adds nothing; empty levels left by an interrupted start
/// are taken as they are.
#[test]
fn the_losing_copy_creates_nothing_and_empty_levels_are_accepted() {
    let world = World::new("root-lost-lock");
    fs::create_dir_all(world.support().join("StickyMD/StickyMD")).unwrap();
    let first = world.resolve(&world.app("system", "StickyMD")).unwrap();
    let _guard = primary(&first);
    open_note_directory(&first.handle).unwrap();
    let executable = world.app("home/Applications", "StickyMD");
    let before = world.tree("home");
    let second = world.resolve(&executable).unwrap();
    assert!(is_secondary(&second));
    assert_eq!(world.tree("home"), before);
}

/// P18-L08 and L09 on this (case-insensitive) volume: one key is one note wherever the copy is
/// installed, a name differing only in case included; another key runs beside it.
#[test]
fn one_key_is_exclusive_and_another_key_runs_beside_it() {
    // The second location holds the same name, or one differing only in case.
    for (label, home_name) in [
        ("root-keys-same", "StickyMD"),
        ("root-keys-case", "stickymd"),
    ] {
        let world = World::new(label);
        let system = world.resolve(&world.app("system", "StickyMD")).unwrap();
        let _guard = primary(&system);
        let home = world
            .resolve(&world.app("home/Applications", home_name))
            .unwrap();
        assert_eq!(installed_key(&home), home_name);
        assert!(is_secondary(&home), "{home_name}");
        let other = world
            .resolve(&world.app("home/Applications", "StickyMD copy"))
            .unwrap();
        let _other_guard = primary(&other);
    }
}

/// Starts both copies (each must be the first) and counts the key directories made.
fn count_keys(upper: &Path, lower: &Path, roots: &Roots, home: &Path) -> usize {
    let first = NoteRoot::resolve(upper, roots).unwrap();
    let second = NoteRoot::resolve(lower, roots).unwrap();
    let guards = (primary(&first), primary(&second));
    drop(guards);
    fs::read_dir(home.join("Library/Application Support/StickyMD"))
        .unwrap()
        .count()
}

/// P18-L08 on a case-sensitive volume: names differing only in case are two notes. Run with
/// `--ignored`.
#[test]
#[ignore = "creates and mounts a case-sensitive APFS disk image"]
fn on_a_case_sensitive_volume_case_makes_another_key() {
    let world = World::new("root-case-sensitive");
    let image = world.path("volume.dmg");
    let mount = world.path("mount");
    fs::create_dir(&mount).unwrap();
    let run = |arguments: &[&std::ffi::OsStr]| {
        let status = std::process::Command::new("hdiutil")
            .args(arguments)
            .status()
            .unwrap();
        assert!(status.success(), "hdiutil {arguments:?}");
    };
    run(&[
        "create".as_ref(),
        "-quiet".as_ref(),
        "-size".as_ref(),
        "64m".as_ref(),
        "-fs".as_ref(),
        "Case-sensitive APFS".as_ref(),
        "-volname".as_ref(),
        "SMDCASE".as_ref(),
        image.as_os_str(),
    ]);
    run(&[
        "attach".as_ref(),
        "-quiet".as_ref(),
        image.as_os_str(),
        "-nobrowse".as_ref(),
        "-mountpoint".as_ref(),
        mount.as_os_str(),
    ]);
    let home = mount.join("home");
    fs::create_dir_all(home.join("Applications")).unwrap();
    fs::create_dir_all(home.join("Library/Application Support")).unwrap();
    // Both copies sit in an `Applications` on the case-sensitive volume, so they are two bundles.
    let roots = Roots {
        system_applications: world.path("mount/system"),
        home: Ok(home.clone()),
    };
    let upper = world.app("mount/system", "StickyMD");
    let lower = world.app("mount/system", "stickymd");
    let outcome = count_keys(&upper, &lower, &roots, &home);
    run(&["detach".as_ref(), "-quiet".as_ref(), mount.as_os_str()]);
    assert_eq!(outcome, 2);
}

/// P18-L10: on an installed root the ports keep the note directory opened at start, as P16-S14
/// requires of a portable one.
#[test]
fn an_installed_note_keeps_its_directory_when_note_is_replaced() {
    let world = World::new("root-replaced");
    let note_root = world.resolve(&world.app("system", "StickyMD")).unwrap();
    let note = open_note_directory(&note_root.handle).unwrap();
    let paths = RuntimePaths::of(&note_root.path);
    let mut ports = mac_io_ports(&note, &paths).unwrap();
    let moved = note_root.path.join("moved");
    fs::rename(&paths.note_dir, &moved).unwrap();
    fs::create_dir(&paths.note_dir).unwrap();
    ports.io.config.save(&RuntimeConfig::default()).unwrap();
    assert!(moved.join("config.toml").is_file());
    assert!(!paths.note_dir.join("config.toml").exists());
}
