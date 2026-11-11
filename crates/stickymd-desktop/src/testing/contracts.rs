//! Scenarios every host's adapter must pass, with their assertions. A host keeps its own test
//! function, under its old name, and calls the scenario with its own constructor, so one fix
//! reaches every host and the hosts cannot drift apart.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use stickymd_app::config::{
    ConfigLoadOutcome, ConfigWarning, ContentZoomPercent, DockEdge, RuntimeConfig, ThemeMode,
    ViewMode,
};
use stickymd_core::{DocumentSnapshot, Generation, LineEnding};
use stickymd_render::image::PreviewImageSource;
use stickymd_render::preview::{PreviewPipeline, PreviewSelection, PreviewTheme};

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
