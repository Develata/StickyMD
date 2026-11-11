//! Scenarios every host's adapter must pass, with their assertions. A host keeps its own test
//! function, under its old name, and calls the scenario with its own constructor, so one fix
//! reaches every host and the hosts cannot drift apart.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

use std::path::PathBuf;
use std::sync::Arc;

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
