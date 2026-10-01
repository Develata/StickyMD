//! Input mutation regressions for qualification dependencies.

use super::{GLOBAL, calculate, path_domains};
use crate::qualification::module_ledger::ModuleId;
use std::fs;
use std::process::Command;

#[test]
fn identical_clock_values_do_not_alias_temporary_inputs() {
    let paths = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..16)
            .map(|_| scope.spawn(|| super::temporary_path_at(42)))
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<std::collections::BTreeSet<_>>()
    });
    assert_eq!(paths.len(), 16);
}

#[test]
fn measurement_fixture_bytes_invalidate_only_their_qualification_module() {
    let root = fixture();
    let before_performance = calculate(&root, ModuleId::Performance).unwrap();
    let before_resources = calculate(
        &root,
        ModuleId::Resource(crate::cli::ResourceModule::SourcePreview),
    )
    .unwrap();
    let before_g4 = calculate(&root, ModuleId::G4).unwrap();
    fs::write(
        root.join("tests/fixtures/performance/resource-note-seed.md"),
        "new resource fixture",
    )
    .unwrap();
    let after_resources = calculate(
        &root,
        ModuleId::Resource(crate::cli::ResourceModule::SourcePreview),
    )
    .unwrap();
    assert_ne!(before_resources, after_resources);
    assert_eq!(
        before_performance,
        calculate(&root, ModuleId::Performance).unwrap()
    );
    assert_eq!(before_g4, calculate(&root, ModuleId::G4).unwrap());

    fs::write(
        root.join("tests/fixtures/performance/typical-note-seed.md"),
        "new startup fixture",
    )
    .unwrap();
    assert_ne!(
        before_performance,
        calculate(&root, ModuleId::Performance).unwrap()
    );
    assert_eq!(
        after_resources,
        calculate(
            &root,
            ModuleId::Resource(crate::cli::ResourceModule::SourcePreview)
        )
        .unwrap()
    );
    assert_eq!(before_g4, calculate(&root, ModuleId::G4).unwrap());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn g5_image_bytes_invalidate_g5_without_invalidating_other_groups() {
    let root = fixture();
    let before = super::super::modules()
        .map(|module| (module, calculate(&root, module).unwrap()))
        .collect::<Vec<_>>();
    fs::write(
        root.join("crates/stickymd-render/tests/fixtures/qualification-images/g5.png"),
        b"changed encoded image bytes",
    )
    .unwrap();
    for (module, fingerprint) in before {
        let changed = calculate(&root, module).unwrap() != fingerprint;
        assert_eq!(changed, module == ModuleId::G5, "{module:?}");
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unrelated_group_harness_does_not_change_module_fingerprint() {
    let root = fixture();
    let before_g3 = calculate(&root, ModuleId::G3).expect("G3 fingerprint");
    let before_g4 = calculate(&root, ModuleId::G4).expect("G4 fingerprint");
    fs::write(
        root.join("tools/stickymd-smoke/src/qualification/g4/cases/dock.rs"),
        "changed",
    )
    .expect("change G4 harness");
    assert_eq!(before_g3, calculate(&root, ModuleId::G3).expect("G3 after"));
    assert_ne!(before_g4, calculate(&root, ModuleId::G4).expect("G4 after"));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn report_text_is_not_a_behavior_input_but_contract_is_global() {
    assert_eq!(path_domains("docs/report/note.md"), 0);
    assert_eq!(path_domains("dist/evidence/module-success/g4.json"), 0);
    assert_ne!(
        path_domains("docs/plan/11_testing_and_release.md") & GLOBAL,
        0
    );
}

#[test]
fn shared_window_and_process_adapters_invalidate_every_consumer() {
    let root = fixture();
    for path in [
        "tools/stickymd-smoke/src/managed_process.rs",
        "tools/stickymd-smoke/src/window_control/input.rs",
    ] {
        let before = super::super::modules()
            .map(|module| (module, calculate(&root, module).unwrap()))
            .collect::<Vec<_>>();
        fs::write(root.join(path), "changed adapter").unwrap();
        for (module, fingerprint) in before {
            assert_ne!(
                fingerprint,
                calculate(&root, module).unwrap(),
                "{path}: {module:?}"
            );
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn resource_group_harness_only_invalidates_its_group() {
    use crate::cli::ResourceModule::{Window, Zoom};
    let root = fixture();
    for (path, group) in [
        (
            "tools/stickymd-smoke/src/runtime/resources/window.rs",
            Window,
        ),
        ("tools/stickymd-smoke/src/runtime/resources/zoom.rs", Zoom),
        ("tools/stickymd-smoke/src/resource_plan/zoom.rs", Zoom),
    ] {
        let before = super::super::modules()
            .map(|module| (module, calculate(&root, module).unwrap()))
            .collect::<Vec<_>>();
        fs::write(root.join(path), "changed").unwrap();
        for (module, fingerprint) in before {
            assert_eq!(
                fingerprint != calculate(&root, module).unwrap(),
                module == ModuleId::Resource(group),
                "{module:?}"
            );
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn candidate_planning_and_resource_cohort_changes_invalidate_their_consumers() {
    let root = fixture();
    for (path, resource_only) in [
        ("tools/stickymd-smoke/src/runtime/resources/cohort.rs", true),
        ("tools/stickymd-smoke/src/runner/candidate_input.rs", false),
    ] {
        let before = super::super::modules()
            .map(|module| (module, calculate(&root, module).unwrap()))
            .collect::<Vec<_>>();
        fs::write(root.join(path), "changed verification behavior").unwrap();
        for (module, fingerprint) in before {
            let affected = matches!(module, ModuleId::Resource(_))
                || (!resource_only && matches!(module, ModuleId::Runtime | ModuleId::Performance));
            assert_eq!(
                fingerprint != calculate(&root, module).unwrap(),
                affected,
                "{path}: {module:?}"
            );
        }
    }
    fs::remove_dir_all(root).unwrap();
}

fn fixture() -> std::path::PathBuf {
    let root = super::temporary_path()
        .expect("fixture path")
        .with_extension("fixture");
    fs::create_dir(&root).expect("exclusive fixture directory");
    for path in [
        "tools/stickymd-smoke/src/qualification/g3/cases.rs",
        "tools/stickymd-smoke/src/qualification/g4/cases/dock.rs",
        "tools/stickymd-smoke/src/managed_process.rs",
        "tools/stickymd-smoke/src/window_control/input.rs",
        "tools/stickymd-smoke/src/runtime/resources/window.rs",
        "tools/stickymd-smoke/src/runtime/resources/zoom.rs",
        "tools/stickymd-smoke/src/resource_plan/zoom.rs",
        "tools/stickymd-smoke/src/runtime/resources/cohort.rs",
        "tools/stickymd-smoke/src/runner/candidate_input.rs",
        "docs/report/note.md",
        "tests/fixtures/performance/typical-note-seed.md",
        "tests/fixtures/performance/resource-note-seed.md",
        "crates/stickymd-render/tests/fixtures/qualification-images/g5.png",
    ] {
        let path = root.join(path);
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, "initial").expect("write fixture");
    }
    assert!(
        Command::new("git")
            .arg("init")
            .arg("--quiet")
            .current_dir(&root)
            .status()
            .expect("git init")
            .success()
    );
    assert!(
        Command::new("git")
            .args(["add", "."])
            .current_dir(&root)
            .status()
            .expect("git add")
            .success()
    );
    root
}

#[test]
fn planning_enumeration_preserves_digests_but_fresh_validation_detects_new_inputs() {
    let root = fixture();
    let inputs = super::PlanningInputs::read(&root).unwrap();
    let modules = super::super::modules().collect::<Vec<_>>();
    let planned = inputs.calculate_many(&root, &modules).unwrap();
    for (module, digest) in modules.into_iter().zip(planned.digests) {
        assert_eq!(digest, calculate(&root, module).unwrap());
    }
    let module = ModuleId::Resource(crate::cli::ResourceModule::Window);
    let before = calculate(&root, module).unwrap();
    fs::write(root.join("new-unclassified-input.bin"), b"new bytes").unwrap();
    assert_eq!(
        before,
        inputs.calculate_many(&root, &[module]).unwrap().digests[0]
    );
    assert_ne!(before, calculate(&root, module).unwrap());
    let existing = root.join("tools/stickymd-smoke/src/managed_process.rs");
    fs::write(&existing, "updated").unwrap(); // Same length as initial; bytes must be re-read.
    assert_ne!(
        before,
        inputs.calculate_many(&root, &[module]).unwrap().digests[0]
    );
    fs::remove_file(existing).unwrap();
    assert!(inputs.calculate_many(&root, &[module]).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_identity_binds_test_sources_and_execution_settings_even_when_module_ignored() {
    let root = fixture();
    let initial = super::workspace_inputs(&root, b"toolchain and settings A").unwrap();
    assert_ne!(
        initial,
        super::workspace_inputs(&root, b"toolchain and settings B").unwrap()
    );
    fs::write(root.join("docs/report/note.md"), "changed governance input").unwrap();
    assert_ne!(
        initial,
        super::workspace_inputs(&root, b"toolchain and settings A").unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "explicit local planning profile; reads this checkout without qualifying a candidate"]
fn resource_planning_profile() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let inputs = super::PlanningInputs::read(root).unwrap();
    let modules = crate::resource_plan::GROUPS.map(ModuleId::Resource);
    let serial = || {
        let started = std::time::Instant::now();
        let values = modules.map(|m| legacy_digest(root, Some(m), &inputs.tracked, &[]));
        (values, started.elapsed().as_secs_f64())
    };
    let batched = || {
        let started = std::time::Instant::now();
        let values = inputs.calculate_many(root, &modules).unwrap();
        (values, started.elapsed().as_secs_f64())
    };
    for round in 0..3 {
        let (serial, batched) = if round % 2 == 0 {
            (serial(), batched())
        } else {
            let batched = batched();
            (serial(), batched)
        };
        for (legacy, digest) in serial.0.iter().zip(&batched.0.digests) {
            assert_eq!(&legacy.0, digest);
        }
        println!(
            "FINGERPRINT_PROFILE round={} legacy_seconds={:.6} batch_seconds={:.6} legacy_reads={} batch_reads={} legacy_bytes={} batch_bytes={}",
            round + 1,
            serial.1,
            batched.1,
            serial.0.iter().map(|v| v.1).sum::<usize>(),
            batched.0.input_files,
            serial.0.iter().map(|v| v.2).sum::<u64>(),
            batched.0.input_bytes,
        );
    }
}

#[test]
fn batched_fingerprints_preserve_v1_bytes_with_bounded_shared_reads() {
    let root = fixture();
    fs::write(root.join("空文件.bin"), []).unwrap();
    fs::write(root.join("binary input.bin"), [0, 255, 13, 10, 0]).unwrap();
    fs::write(root.join("large.bin"), vec![137; 131_073]).unwrap();
    let inputs = super::PlanningInputs::read(&root).unwrap();
    let modules = super::super::modules().collect::<Vec<_>>();
    let batch = inputs.calculate_many(&root, &modules).unwrap();
    let mut expected_files = 0;
    let mut expected_bytes = 0;
    for path in &inputs.tracked {
        if modules
            .iter()
            .any(|m| path_domains(path) & super::domains(*m) != 0)
        {
            expected_files += 1;
            expected_bytes += root.join(path).metadata().unwrap().len();
        }
    }
    assert_eq!(batch.input_files, expected_files);
    assert_eq!(batch.input_bytes, expected_bytes);
    let mut serial_reads = 0;
    for (module, digest) in modules.iter().zip(batch.digests) {
        let legacy = legacy_digest(&root, Some(*module), &inputs.tracked, &[]);
        assert_eq!(digest, legacy.0, "{module:?}");
        serial_reads += legacy.1;
    }
    assert!(serial_reads > batch.input_files);
    let extra = b"toolchain\0execution identity";
    assert_eq!(
        super::workspace_inputs(&root, extra).unwrap(),
        legacy_digest(&root, None, &inputs.tracked, extra).0
    );
    let empty = inputs.calculate_many(&root, &[]).unwrap();
    assert!(empty.digests.is_empty());
    assert_eq!((empty.input_files, empty.input_bytes), (0, 0));
    fs::remove_dir_all(root).unwrap();
}

// Retain the previous serializer as a compatibility oracle and opt-in timing baseline.
// It deliberately uses the old unbuffered, one-module-at-a-time file stream and sync_all.
fn legacy_digest(
    root: &std::path::Path,
    module: Option<ModuleId>,
    tracked: &[String],
    extra: &[u8],
) -> (String, usize, u64) {
    use std::io::Write;
    let path = super::temporary_path().unwrap();
    let mut output = fs::File::create_new(&path).unwrap();
    output
        .write_all(b"StickyMD qualification module fingerprint v1\0")
        .unwrap();
    output
        .write_all(
            module
                .map_or("workspace-tests", ModuleId::as_str)
                .as_bytes(),
        )
        .unwrap();
    output.write_all(&[0]).unwrap();
    let mut files = 0;
    let mut bytes = 0;
    for relative in tracked {
        if module.is_some_and(|m| path_domains(relative) & super::domains(m) == 0) {
            continue;
        }
        let name = relative.as_bytes();
        output
            .write_all(&(name.len() as u64).to_le_bytes())
            .unwrap();
        output.write_all(name).unwrap();
        let source = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        let length = source.metadata().unwrap().len();
        output.write_all(&length.to_le_bytes()).unwrap();
        let mut input = fs::File::open(source).unwrap();
        bytes += std::io::copy(&mut input, &mut output).unwrap();
        files += 1;
    }
    if module.is_none() {
        output
            .write_all(&(extra.len() as u64).to_le_bytes())
            .unwrap();
        output.write_all(extra).unwrap();
    }
    output.sync_all().unwrap();
    drop(output);
    let digest = crate::qualification::receipt::sha256(&path);
    fs::remove_file(path).unwrap();
    (digest.unwrap(), files, bytes)
}
