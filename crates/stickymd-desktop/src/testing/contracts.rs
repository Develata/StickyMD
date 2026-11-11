//! Scenarios every host's adapter must pass, with their assertions. A host keeps its own test
//! function, under its old name, and calls the scenario with its own constructor, so one fix
//! reaches every host and the hosts cannot drift apart.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

/// The tray's show/hide item names the action it takes (plan 09 tray lifecycle).
pub fn tray_show_hide_label_describes_the_action() {
    use crate::tray::show_hide_label;
    assert_eq!(show_hide_label(true), "隐藏");
    assert_eq!(show_hide_label(false), "显示");
}

/// The tray icon is owned RGBA of the stated size with visible pixels.
pub fn tray_icon_is_fully_owned_rgba() {
    let icon = crate::tray::tray_icon();
    assert_eq!(
        icon.rgba.len(),
        icon.width as usize * icon.height as usize * 4
    );
    assert!(icon.rgba.chunks_exact(4).any(|pixel| pixel[3] == 255));
}

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use stickymd_app::config::{
    ConfigLoadOutcome, ConfigWarning, ContentZoomPercent, DockEdge, RuntimeConfig, ThemeMode,
    ViewMode,
};
use stickymd_app::flow::ExportError;
use stickymd_app::ports::ExportCompletion;
use stickymd_core::{
    CursorSnapshot, DocumentSnapshot, DocumentState, EditKind, EditMeta, EditRequest, Generation,
    LineEnding,
};
use stickymd_render::image::PreviewImageSource;
use stickymd_render::preview::{
    PreviewParser, PreviewPipeline, PreviewSelection, PreviewTheme, collect_image_occurrences,
};

/// A host's local image source over a note directory (plan 08 local image read boundary): it reads
/// a percent-encoded Unicode name, reports a missing file as `None`, refuses network shares and
/// device paths, never reads an `http(s)` image, and lets the Preview show such a source's alt
/// text. `root` must not exist yet; `make` builds the host's source over it.
pub fn local_image_source_contract<S: PreviewImageSource>(
    root: PathBuf,
    make: impl FnOnce(PathBuf) -> S,
) {
    std::fs::create_dir(&root).unwrap();
    let image = stickymd_render::image::prepare_rgba_image(2, 2, vec![255; 16]).unwrap();
    let bytes = image.bytes();
    std::fs::write(root.join("数学 图.png"), bytes).unwrap();
    let source = make(root.clone());
    let path = "%E6%95%B0%E5%AD%A6%20%E5%9B%BE.png";
    assert!(source.inspect(path).unwrap().is_some());
    assert_eq!(source.load(path).unwrap().as_deref(), Some(bytes));
    assert!(source.inspect("missing.png").unwrap().is_none());
    for remote in [
        "file://server/share/a.png",
        "%5c%5cserver/share/a.png",
        r"\\.\pipe\image",
    ] {
        assert!(source.inspect(remote).is_err());
        assert!(source.load(remote).is_err());
    }
    assert!(
        source
            .load("https://example.invalid/a.png")
            .unwrap()
            .is_none()
    );
    let snapshot = DocumentSnapshot {
        text: Arc::from("![ALT](file://server/share/a.png)"),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    };
    let mut pipeline = PreviewPipeline::new();
    let frame = pipeline
        .build_with_image_source(
            &snapshot,
            500,
            200,
            1.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
            Some(&source),
        )
        .unwrap();
    assert_eq!(frame.copy_selection(frame.select_all()), Some("ALT"));
    assert_eq!(std::fs::read(root.join("数学 图.png")).unwrap(), bytes);
    std::fs::remove_dir_all(root).unwrap();
}

/// The host operations exercised by the config persistence scenarios (plan 05). Each operation
/// uses the host's real adapter; the host unwraps load and save errors at this test boundary.
pub struct ConfigStorageUnderTest {
    pub load: fn(&Path) -> ConfigLoadOutcome,
    pub save: fn(&Path, &Path, &RuntimeConfig),
    pub preserve_at: fn(&Path, u64) -> ConfigLoadOutcome,
    pub temp_path: fn(&str) -> PathBuf,
}

pub fn missing_and_partial_config_use_defaults(host: ConfigStorageUnderTest) {
    let root = (host.temp_path)("defaults");
    fs::create_dir(&root).unwrap();
    let path = root.join("config.toml");
    let missing = (host.load)(&path);
    assert!(missing.should_create_default);
    assert!(missing.persistence_allowed);
    assert_eq!(missing.config, RuntimeConfig::default());
    fs::write(&path, "version = 1\nopacity = 85\n").unwrap();
    let partial = (host.load)(&path);
    assert_eq!(partial.config.opacity, 85);
    assert_eq!(partial.config.theme, ThemeMode::Light);
    fs::remove_dir_all(root).unwrap();
}

pub fn unknown_fields_are_ignored_but_invalid_enum_is_preserved_as_corrupt(
    host: ConfigStorageUnderTest,
) {
    let root = (host.temp_path)("unknown");
    fs::create_dir(&root).unwrap();
    let path = root.join("config.toml");
    fs::write(&path, "version = 1\nfuture = true\n").unwrap();
    assert!((host.load)(&path).warning.is_none());
    fs::write(&path, "version = 1\ntheme = \"purple\"\n").unwrap();
    let invalid = (host.load)(&path);
    assert!(matches!(
        invalid.warning,
        Some(ConfigWarning::CorruptPreserved(_))
    ));
    assert!(!path.exists());
    fs::remove_dir_all(root).unwrap();
}

pub fn corrupt_config_name_collision_preserves_both_evidence_files(host: ConfigStorageUnderTest) {
    let root = (host.temp_path)("collision");
    fs::create_dir(&root).unwrap();
    let path = root.join("config.toml");
    let occupied = root.join("config.invalid-42.toml");
    fs::write(&occupied, "older evidence").unwrap();
    fs::write(&path, "version = invalid").unwrap();

    let outcome = (host.preserve_at)(&path, 42);
    let Some(ConfigWarning::CorruptPreserved(preserved)) = outcome.warning else {
        panic!("corrupt config should be preserved");
    };
    assert_eq!(preserved, root.join("config.invalid-42-1.toml"));
    assert_eq!(fs::read_to_string(occupied).unwrap(), "older evidence");
    assert_eq!(fs::read_to_string(preserved).unwrap(), "version = invalid");
    assert!(outcome.persistence_allowed);
    fs::remove_dir_all(root).unwrap();
}

pub fn newer_version_is_preserved_and_not_overwritten(host: ConfigStorageUnderTest) {
    let root = (host.temp_path)("newer");
    fs::create_dir(&root).unwrap();
    let path = root.join("config.toml");
    fs::write(&path, "version = 2\n").unwrap();
    let outcome = (host.load)(&path);
    assert_eq!(
        outcome.warning,
        Some(ConfigWarning::UnsupportedNewerVersion(2))
    );
    assert!(!outcome.should_create_default);
    assert!(!outcome.persistence_allowed);
    assert!(path.exists());
    fs::remove_dir_all(root).unwrap();
}

pub fn invalid_v1_values_and_older_versions_are_preserved_without_blocking_note_startup(
    host: ConfigStorageUnderTest,
) {
    for source in [
        "version = 1\nopacity = 1\n",
        "version = 1\n[window]\nwidth_dip = 1\n",
        "version = 1\n[window]\ndock_offset_ratio = nan\n",
        "version = 1\n[window]\nfloating_x_ratio = inf\n",
        "version = 0\n",
    ] {
        let root = (host.temp_path)("semantic-invalid");
        fs::create_dir(&root).unwrap();
        let path = root.join("config.toml");
        fs::write(&path, source).unwrap();
        let outcome = (host.load)(&path);
        assert_eq!(outcome.config, RuntimeConfig::default());
        assert!(matches!(
            outcome.warning,
            Some(ConfigWarning::CorruptPreserved(_))
        ));
        assert!(outcome.persistence_allowed);
        fs::remove_dir_all(root).unwrap();
    }
}

/// The durable bytes of `RuntimeConfig::default()`, fixed before the config model left the Windows
/// host (Phase 15): a change here changes `config.toml` on disk.
const DEFAULT_CONFIG_BYTES: &str = "version = 1
theme = \"light\"
opacity = 96
content_zoom_percent = 100
split_scroll_sync = true
always_on_top = false
view_mode = \"source\"

[window]
width_dip = 520
height_dip = 680
monitor_id = \"\"
dock_edge = \"none\"
dock_offset_ratio = 0.5
floating_x_ratio = 0.5
floating_y_ratio = 0.25
";

/// A config with every field off its default, including a non-ASCII monitor id that needs
/// quoting and ratios whose decimal text depends on the f32 formatting.
const POPULATED_CONFIG_BYTES: &str = "version = 1
theme = \"dark\"
opacity = 83
content_zoom_percent = 175
split_scroll_sync = false
always_on_top = true
view_mode = \"split\"

[window]
width_dip = 640
height_dip = 480
monitor_id = 'monitor-中\"1'
dock_edge = \"right\"
dock_offset_ratio = 0.25
floating_x_ratio = 0.33333334
floating_y_ratio = 0.1
";

fn populated_config() -> RuntimeConfig {
    let mut config = RuntimeConfig {
        theme: ThemeMode::Dark,
        opacity: 83,
        content_zoom_percent: ContentZoomPercent::new_clamped(175),
        split_scroll_sync: false,
        always_on_top: true,
        view_mode: ViewMode::Split,
        ..RuntimeConfig::default()
    };
    config.window.width_dip = 640;
    config.window.height_dip = 480;
    config.window.monitor_id = "monitor-中\"1".to_owned();
    config.window.dock_edge = DockEdge::Right;
    config.window.dock_offset_ratio = 0.25;
    config.window.floating_x_ratio = 1.0 / 3.0;
    config.window.floating_y_ratio = 0.1;
    config
}

/// Phase 15 byte compatibility: the serialized bytes are fixed, and a load followed by a save
/// reproduces them, in this build and in any checkpoint a rollback returns to.
pub fn config_bytes_are_fixed_and_survive_a_load_and_save(host: ConfigStorageUnderTest) {
    let root = (host.temp_path)("bytes");
    fs::create_dir(&root).unwrap();
    let path = root.join("config.toml");
    let temporary = root.join("config.toml.tmp");
    for (config, bytes) in [
        (RuntimeConfig::default(), DEFAULT_CONFIG_BYTES),
        (populated_config(), POPULATED_CONFIG_BYTES),
    ] {
        (host.save)(&path, &temporary, &config);
        assert_eq!(fs::read_to_string(&path).unwrap(), bytes);
        fs::write(&path, bytes).unwrap();
        let loaded = (host.load)(&path);
        assert!(loaded.warning.is_none());
        assert_eq!(loaded.config, config);
        (host.save)(&path, &temporary, &loaded.config);
        assert_eq!(fs::read_to_string(&path).unwrap(), bytes);
    }
    fs::remove_dir_all(root).unwrap();
}

pub fn config_save_is_atomic_and_roundtrips(host: ConfigStorageUnderTest) {
    let root = (host.temp_path)("save");
    fs::create_dir(&root).unwrap();
    let path = root.join("config.toml");
    let temporary = root.join("config.toml.tmp");
    let config = RuntimeConfig {
        opacity: 83,
        ..RuntimeConfig::default()
    };
    (host.save)(&path, &temporary, &config);
    assert_eq!((host.load)(&path).config, config);
    assert!(!temporary.exists());
    fs::remove_dir_all(root).unwrap();
}

pub fn zoom_and_split_sync_roundtrip_and_missing_zoom_defaults(host: ConfigStorageUnderTest) {
    let root = (host.temp_path)("preferences");
    fs::create_dir(&root).unwrap();
    let path = root.join("config.toml");
    let temporary = root.join("config.toml.tmp");
    let config = RuntimeConfig {
        content_zoom_percent: ContentZoomPercent::new_clamped(175),
        split_scroll_sync: false,
        opacity: 40,
        ..RuntimeConfig::default()
    };
    (host.save)(&path, &temporary, &config);
    assert_eq!((host.load)(&path).config, config);

    fs::write(&path, "version = 1\nopacity = 40\n").unwrap();
    let migrated = (host.load)(&path);
    assert_eq!(migrated.config.content_zoom_percent.value(), 100);
    assert!(migrated.config.split_scroll_sync);
    fs::remove_dir_all(root).unwrap();
}

// Export scenarios (plan 08): `root` is the host's empty fixture directory. The host
// builds its request and retains every file operation and safety guarantee. The path
// resolution scenario also uses the host's resolver to check its exported destination.

pub fn export_copies_only_real_local_images_and_preserves_other_source<P: std::fmt::Debug>(
    root: PathBuf,
    export_snapshot: impl Fn(
        &Path,
        DocumentSnapshot,
        PathBuf,
    ) -> Result<ExportCompletion, ExportError<P>>,
) {
    let note = root.join("note");
    let out = root.join("out");
    fs::create_dir(&note).unwrap();
    fs::create_dir(&out).unwrap();
    fs::write(note.join("a.png"), b"local-image").unwrap();
    let source = "before\n\n![x](a.png \"T\")\n\n![remote](https://example/a.png)\n\n`![code](missing.png)`\n";
    let result = export_snapshot(
        &note,
        DocumentSnapshot {
            text: Arc::from(source),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        },
        out.join("copy.md"),
    )
    .unwrap();
    assert_eq!(result.copied_assets, 1);
    let markdown = fs::read_to_string(out.join("copy.md")).unwrap();
    assert!(markdown.contains("![x](copy-assets/stickymd-"));
    assert!(markdown.contains("![remote](https://example/a.png)"));
    assert!(markdown.contains("`![code](missing.png)`"));
    assert_eq!(fs::read_dir(out.join("copy-assets")).unwrap().count(), 1);
    fs::remove_dir_all(root).unwrap();
}

pub fn export_missing_local_image_fails_before_any_publish<P: std::fmt::Debug>(
    root: PathBuf,
    export_snapshot: impl Fn(
        &Path,
        DocumentSnapshot,
        PathBuf,
    ) -> Result<ExportCompletion, ExportError<P>>,
) {
    let target = root.join("copy.md");
    fs::write(root.join("present.png"), b"present").unwrap();
    let result = export_snapshot(
        &root,
        DocumentSnapshot {
            text: Arc::from("![ok](present.png)\n\n![x](missing.png)"),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        },
        target.clone(),
    );
    assert!(matches!(result, Err(ExportError::LocalImage { .. })));
    assert!(!target.exists());
    assert!(!root.join("copy-assets").exists());
    assert!(fs::read_dir(&root).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".stickymd-export-")
    }));
    fs::remove_dir_all(root).unwrap();
}

pub fn export_denied_local_paths_preserve_existing_output<P: std::fmt::Debug>(
    root: PathBuf,
    export_snapshot: impl Fn(
        &Path,
        DocumentSnapshot,
        PathBuf,
    ) -> Result<ExportCompletion, ExportError<P>>,
) {
    let target = root.join("copy.md");
    fs::write(&target, b"previous export").unwrap();
    for destination in [
        "file://server/share/a.png",
        "%5c%5cserver/share/a.png",
        "%5c%5c.%5cpipe%5cimage",
    ] {
        let snapshot = DocumentSnapshot {
            text: Arc::from(format!("![ALT]({destination})")),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        let result = export_snapshot(&root, snapshot, target.clone());
        assert!(
            matches!(result, Err(ExportError::UnsupportedLocalPath(_))),
            "{destination}: {result:?}"
        );
        assert_eq!(fs::read(&target).unwrap(), b"previous export");
        assert!(!root.join("copy-assets").exists());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    }
    fs::remove_dir_all(root).unwrap();
}

pub fn export_deduplicates_sources_suffixes_existing_assets_and_replaces_markdown<
    P: std::fmt::Debug,
>(
    root: PathBuf,
    export_snapshot: impl Fn(
        &Path,
        DocumentSnapshot,
        PathBuf,
    ) -> Result<ExportCompletion, ExportError<P>>,
) {
    let note = root.join("note");
    let out = root.join("out");
    fs::create_dir(&note).unwrap();
    fs::create_dir(&out).unwrap();
    let first = note.join("first.png");
    let second = note.join("second.png");
    fs::write(&first, b"identical bytes").unwrap();
    fs::write(&second, b"identical bytes").unwrap();
    fs::create_dir(out.join("copy-assets")).unwrap();
    fs::write(out.join("copy-assets/user.txt"), b"keep").unwrap();
    fs::write(out.join("copy.md"), b"old").unwrap();
    let absolute = second.to_string_lossy().replace('\\', "/");
    let source = format!(
        "prefix\n\n![one](first.png)\n\n![two]({absolute})\n\n![again](first.png)\n\nsuffix\n"
    );
    let completion = export_snapshot(
        &note,
        DocumentSnapshot {
            text: Arc::from(source),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        },
        out.join("copy.md"),
    )
    .unwrap();
    assert_eq!(completion.copied_assets, 1);
    assert_eq!(fs::read(out.join("copy-assets/user.txt")).unwrap(), b"keep");
    assert_eq!(fs::read_dir(out.join("copy-assets-2")).unwrap().count(), 1);
    let markdown = fs::read_to_string(out.join("copy.md")).unwrap();
    assert_eq!(markdown.matches("copy-assets-2/stickymd-").count(), 3);
    assert!(markdown.starts_with("prefix\n\n"));
    assert!(markdown.ends_with("\n\nsuffix\n"));
    fs::remove_dir_all(root).unwrap();
}

pub fn export_without_images_preserves_markdown_bytes_and_creates_no_asset_dir<
    P: std::fmt::Debug,
>(
    root: PathBuf,
    export_snapshot: impl Fn(
        &Path,
        DocumentSnapshot,
        PathBuf,
    ) -> Result<ExportCompletion, ExportError<P>>,
) {
    let source = "# Exact\n\n`code`  \n\nraw <x>\n";
    export_snapshot(
        &root,
        DocumentSnapshot {
            text: Arc::from(source),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        },
        root.join("copy.md"),
    )
    .unwrap();
    assert_eq!(fs::read(root.join("copy.md")).unwrap(), source.as_bytes());
    assert!(!root.join("copy-assets").exists());
    fs::remove_dir_all(root).unwrap();
}

pub fn export_rejects_the_working_note_and_hard_link_alias_without_modifying_either<
    P: std::fmt::Debug,
>(
    root: PathBuf,
    export_snapshot: impl Fn(
        &Path,
        DocumentSnapshot,
        PathBuf,
    ) -> Result<ExportCompletion, ExportError<P>>,
) {
    let note = root.join("note");
    let out = root.join("out");
    fs::create_dir(&note).unwrap();
    fs::create_dir(&out).unwrap();
    let working = note.join("note.md");
    let alias = out.join("alias.md");
    fs::write(&working, b"durable working note").unwrap();
    fs::hard_link(&working, &alias).unwrap();
    for target in [working.clone(), alias.clone()] {
        let result = export_snapshot(
            &note,
            DocumentSnapshot {
                text: Arc::from("replacement"),
                generation: Generation::initial(),
                line_ending: LineEnding::Lf,
            },
            target,
        );
        assert!(matches!(result, Err(ExportError::WorkingNoteTarget)));
        assert_eq!(fs::read(&working).unwrap(), b"durable working note");
        assert_eq!(fs::read(&alias).unwrap(), b"durable working note");
    }
    fs::remove_file(&alias).unwrap();
    fs::remove_file(&working).unwrap();
    let absent_result = export_snapshot(
        &note,
        DocumentSnapshot {
            text: Arc::from("replacement"),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        },
        note.join("NOTE.MD"),
    );
    assert!(matches!(absent_result, Err(ExportError::WorkingNoteTarget)));
    assert!(!note.join("NOTE.MD").exists());
    fs::remove_dir_all(root).unwrap();
}

pub fn export_percent_encodes_asset_directory_and_remains_parseable_and_resolvable<
    P: std::fmt::Debug,
    E: std::fmt::Debug,
>(
    root: PathBuf,
    export_snapshot: impl Fn(
        &Path,
        DocumentSnapshot,
        PathBuf,
    ) -> Result<ExportCompletion, ExportError<P>>,
    resolve_local_image: impl Fn(&Path, &str) -> Result<PathBuf, E>,
) {
    let note = root.join("note");
    let out = root.join("out");
    fs::create_dir(&note).unwrap();
    fs::create_dir(&out).unwrap();
    fs::write(note.join("source.png"), b"asset bytes").unwrap();
    let target = out.join("my note.md");
    export_snapshot(
        &note,
        DocumentSnapshot {
            text: Arc::from("before ![alt](source.png) after"),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        },
        target.clone(),
    )
    .unwrap();
    let markdown = fs::read_to_string(&target).unwrap();
    assert!(markdown.contains("my%20note-assets/stickymd-"));
    let parsed = PreviewParser
        .parse(&DocumentSnapshot {
            text: Arc::from(markdown),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        })
        .unwrap();
    let occurrence = collect_image_occurrences(&parsed).unwrap().remove(0);
    let resolved = resolve_local_image(&out, &occurrence.destination).unwrap();
    assert!(resolved.is_file());
    fs::remove_dir_all(root).unwrap();
}

pub fn export_snapshot_cannot_mutate_document_authority_or_history<P: std::fmt::Debug>(
    root: PathBuf,
    export_snapshot: impl Fn(
        &Path,
        DocumentSnapshot,
        PathBuf,
    ) -> Result<ExportCompletion, ExportError<P>>,
) {
    let mut document = DocumentState::loaded("before", LineEnding::Lf, None);
    document
        .edit(EditRequest::new(
            document.generation(),
            6..6,
            " after",
            CursorSnapshot::caret(6),
            CursorSnapshot::caret(12),
            EditMeta::new(EditKind::Typing, 0),
        ))
        .unwrap();
    let before = (
        document.text().to_owned(),
        document.generation(),
        document.saved_generation(),
        document.is_dirty(),
        document.can_undo(),
        document.can_redo(),
    );

    export_snapshot(&root, document.snapshot(), root.join("copy.md")).unwrap();

    assert_eq!(
        (
            document.text().to_owned(),
            document.generation(),
            document.saved_generation(),
            document.is_dirty(),
            document.can_undo(),
            document.can_redo(),
        ),
        before
    );
    fs::remove_dir_all(root).unwrap();
}
