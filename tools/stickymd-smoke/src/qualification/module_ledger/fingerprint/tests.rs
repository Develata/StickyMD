//! Input mutation regressions for qualification dependencies.

use super::{GLOBAL, calculate, is_manual_only, path_domains};
use crate::qualification::module_registry::ModuleId;
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
    let before = crate::qualification::module_registry::modules()
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

/// Reports of judgements are not inputs; the rules that make the judgements are global.
#[test]
fn status_and_readiness_reports_do_not_invalidate_but_acceptance_rules_do() {
    for path in [
        "tools/stickymd-smoke/src/qualification/module_ledger/status.rs",
        "tools/stickymd-smoke/src/qualification/readiness.rs",
    ] {
        assert_eq!(path_domains(path), 0, "{path}");
    }
    for path in [
        "tools/stickymd-smoke/src/qualification/module_evidence.rs",
        "tools/stickymd-smoke/src/qualification/module_registry.rs",
        "tools/stickymd-smoke/src/qualification/module_ledger.rs",
        "tools/stickymd-smoke/src/qualification/module_ledger/record.rs",
    ] {
        assert_eq!(path_domains(path), GLOBAL, "{path}");
    }
}

/// Manual observations are not ledger records and no functional module runs the recorder,
/// so manual-only code and guidance invalidate nothing; what they share stays classified.
#[test]
fn manual_acceptance_code_invalidates_no_functional_module() {
    for path in [
        "tools/manual/phase-14-guide.md",
        "tools/stickymd-smoke/src/qualification/manual.rs",
        "tools/stickymd-smoke/src/qualification/manual/flow.rs",
        "tools/stickymd-smoke/src/qualification/manual/instance.rs",
        "tools/stickymd-smoke/src/qualification/guided.rs",
        "tools/stickymd-smoke/src/qualification/manual_observation.rs",
        "tools/stickymd-smoke/src/qualification/manual_observation/store.rs",
        "tools/stickymd-smoke/src/qualification/manual_readiness.rs",
        "tools/stickymd-smoke/src/qualification/manual_readiness/tests.rs",
    ] {
        assert_eq!(path_domains(path), 0, "{path}");
    }
    for path in [
        "tools/stickymd-smoke/src/qualification/manual_receipt.rs",
        "tools/stickymd-smoke/src/qualification/manualish.rs",
        "tools/stickymd-smoke/src/qualification/exact_desktop.rs",
        "tools/stickymd-smoke/src/qualification/decisions.rs",
        "tools/stickymd-smoke/src/qualification/reserved_paths.rs",
    ] {
        assert_ne!(path_domains(path), 0, "{path}");
    }
}

/// The same through the fingerprints themselves: editing manual-only code changes no
/// module's fingerprint, while the reserved paths every evidence writer checks change all.
#[test]
fn manual_acceptance_edits_keep_every_module_fingerprint() {
    let root = fixture();
    for path in [
        "tools/manual/phase-14-guide.md",
        "tools/stickymd-smoke/src/qualification/manual/flow.rs",
        "tools/stickymd-smoke/src/qualification/manual_observation/store.rs",
        "tools/stickymd-smoke/src/qualification/reserved_paths.rs",
    ] {
        let path = root.join(path);
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, "initial").expect("write input");
    }
    assert!(
        Command::new("git")
            .args(["add", "."])
            .current_dir(&root)
            .status()
            .expect("git add")
            .success()
    );
    let modules = crate::qualification::module_registry::modules().collect::<Vec<_>>();
    let fingerprints = || {
        modules
            .iter()
            .map(|module| calculate(&root, *module).expect("fingerprint"))
            .collect::<Vec<_>>()
    };
    let before = fingerprints();
    for path in [
        "tools/manual/phase-14-guide.md",
        "tools/stickymd-smoke/src/qualification/manual/flow.rs",
        "tools/stickymd-smoke/src/qualification/manual_observation/store.rs",
    ] {
        fs::write(root.join(path), "changed").expect("change manual code");
    }
    assert_eq!(fingerprints(), before);
    fs::write(
        root.join("tools/stickymd-smoke/src/qualification/reserved_paths.rs"),
        "changed",
    )
    .expect("change reserved paths");
    let after = fingerprints();
    for ((module, before), after) in modules.iter().zip(&before).zip(&after) {
        assert_ne!(before, after, "{}", module.as_str());
    }
    fs::remove_dir_all(root).expect("cleanup");
}

/// Code outside the manual-only set must not reach into it, or editing manual code would
/// change what a formal module run does without changing its fingerprint. The files that
/// may name manual modules (besides test modules), and why none lets manual code alter
/// module evidence:
/// - `qualification/mod.rs`: module declarations and the `acceptance manual` dispatch;
/// - `qualification/readiness.rs`: a judgement projection with no domains of its own;
/// - `qualification/decisions.rs`: decision-key validation. The formal-run output guard
///   calls only its `is_internal_path`, built from its own constants; the campaign projects
///   decisions before any module runs, so a validation failure aborts the campaign instead
///   of changing a module's evidence.
#[test]
fn only_listed_code_names_manual_only_modules() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut pending = vec![source.clone()];
    let mut offenders = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("list sources") {
            let path = entry.expect("source entry").path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            let relative = path
                .strip_prefix(&source)
                .expect("under src")
                .to_string_lossy()
                .replace('\\', "/");
            // Test modules (`tests.rs`) never run in a formal module run.
            if is_manual_only(&format!("tools/stickymd-smoke/src/{relative}"))
                || relative.ends_with("tests.rs")
                || matches!(
                    relative.as_str(),
                    "qualification/readiness.rs" | "qualification/decisions.rs"
                )
            {
                continue;
            }
            let text = fs::read_to_string(&path).expect("read source");
            for (line, uses) in manual_module_paths(&text) {
                // `mod.rs` declares the modules and dispatches `acceptance manual`.
                if relative == "qualification/mod.rs" && uses == "manual::execute" {
                    continue;
                }
                offenders.push(format!("{relative}:{line}: {uses}"));
            }
        }
    }
    assert!(offenders.is_empty(), "{offenders:#?}");
}

#[test]
fn shared_window_and_process_adapters_invalidate_every_consumer() {
    let root = fixture();
    for path in [
        "tools/stickymd-smoke/src/managed_process.rs",
        "tools/stickymd-smoke/src/window_control/input.rs",
    ] {
        let before = crate::qualification::module_registry::modules()
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
        (
            "tools/stickymd-smoke/src/resource_plan/window_comparison.rs",
            Window,
        ),
        (
            "tools/stickymd-smoke/src/runtime/resources/comparison.rs",
            Window,
        ),
        (
            "tools/stickymd-smoke/src/window_control/resource_comparison.rs",
            Window,
        ),
    ] {
        let before = crate::qualification::module_registry::modules()
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
        let before = crate::qualification::module_registry::modules()
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
        "tools/stickymd-smoke/src/resource_plan/window_comparison.rs",
        "tools/stickymd-smoke/src/runtime/resources/comparison.rs",
        "tools/stickymd-smoke/src/window_control/resource_comparison.rs",
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
    let modules = crate::qualification::module_registry::modules().collect::<Vec<_>>();
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
fn module_planning_profile() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let inputs = super::PlanningInputs::read(root).unwrap();
    // Every registered module, as readiness and `qualification modules` compute them.
    let modules = crate::qualification::module_registry::modules().collect::<Vec<_>>();
    let serial = || {
        let started = std::time::Instant::now();
        let values = modules
            .iter()
            .map(|m| legacy_digest(root, Some(*m), &inputs.tracked, &[]))
            .collect::<Vec<_>>();
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
            "FINGERPRINT_PROFILE modules={} round={} legacy_seconds={:.6} batch_seconds={:.6} legacy_reads={} batch_reads={} legacy_bytes={} batch_bytes={}",
            modules.len(),
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
    let modules = crate::qualification::module_registry::modules().collect::<Vec<_>>();
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
    // Module digests moved to v2 (version normalization); these fixtures contain no
    // Cargo manifests, so only the header differs from the original serializer.
    let header: &[u8] = if module.is_some() {
        b"StickyMD qualification module fingerprint v2\0"
    } else {
        b"StickyMD qualification module fingerprint v1\0"
    };
    output.write_all(header).unwrap();
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
    // Same v2 semantics as the batch: module digests see normalized manifests.
    let normalizer = module.and_then(|_| super::normalize::VersionNormalizer::read(root, tracked));
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
        if let Some(normalizer) = normalizer.as_ref()
            && super::normalize::VersionNormalizer::applies_to(relative)
        {
            let raw = fs::read(&source).unwrap();
            let content = normalizer.normalize(relative, &raw);
            output
                .write_all(&(content.len() as u64).to_le_bytes())
                .unwrap();
            output.write_all(&content).unwrap();
            bytes += raw.len() as u64;
            files += 1;
            continue;
        }
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

#[test]
fn every_product_source_has_an_explicit_rule() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    super::verify_product_classification(root).unwrap();

    let fixture = fixture();
    let path = fixture.join("apps/stickymd-win/src/unclassified.rs");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "//! new product file").unwrap();
    let error = super::verify_product_classification(&fixture).unwrap_err();
    assert!(
        error.contains("apps/stickymd-win/src/unclassified.rs"),
        "{error}"
    );
    fs::remove_dir_all(fixture).unwrap();
}

/// Files the G5 cases reach without any G5-specific trigger: the note load, recovery
/// inspection, autosave after the paste case, reconciliation of that write, and the atomic
/// configuration publish the view and opacity cases wait for. Each must stay an input of
/// every module, not only of the persistence-facing ones.
#[test]
fn every_session_persistence_invalidates_every_module() {
    for path in [
        "apps/stickymd-win/src/startup/bootstrap.rs",
        "apps/stickymd-win/src/persistence/storage.rs",
        "apps/stickymd-win/src/flow/recovery.rs",
        "apps/stickymd-win/src/flow/save.rs",
        "apps/stickymd-win/src/flow/reconciliation.rs",
        "apps/stickymd-win/src/config/storage.rs",
        "apps/stickymd-win/src/platform/windows/atomic_file.rs",
        "apps/stickymd-win/src/assets/storage.rs",
    ] {
        for module in crate::qualification::module_registry::modules() {
            assert_ne!(
                path_domains(path) & super::domains(module),
                0,
                "{path}: {module:?}"
            );
        }
    }
}

/// The formal Performance plan runs Release unit baselines by name filter wherever they live
/// (for example `phase7_export_release_baseline` in `export/mod.rs` and the Phase 8 window
/// reducer and control-layout baselines), so the files holding them are its inputs.
#[test]
fn performance_baselines_outside_its_features_invalidate_it() {
    for path in [
        "apps/stickymd-win/src/export/mod.rs",
        "apps/stickymd-win/src/assets/transaction.rs",
        "crates/stickymd-render/src/math/engine.rs",
        "crates/stickymd-render/src/image.rs",
        "apps/stickymd-win/src/flow/window/reducer.rs",
        "apps/stickymd-win/src/app/controls.rs",
        "apps/stickymd-win/src/source_search.rs",
    ] {
        assert_ne!(
            path_domains(path) & super::domains(ModuleId::Performance),
            0,
            "{path}"
        );
    }
}

/// Where `text` may reach a manual-only module: the distinctive names
/// (`manual_observation`, `manual_readiness`) anywhere in code, including as a macro
/// argument; the generic ones (`manual`, `guided`) inside a `use` declaration (aliased,
/// grouped or spread over lines) or next to `::`; and any `#[path]` attribute. Comments,
/// string and character literals are skipped; `mod manual;` declares rather than uses.
/// A lexical guard: it does not expand macros or follow re-exports of listed files.
/// Returns the line and the name with the path segment that follows it.
fn manual_module_paths(text: &str) -> Vec<(usize, String)> {
    const DISTINCTIVE: [&str; 2] = ["manual_observation", "manual_readiness"];
    const GENERIC: [&str; 2] = ["manual", "guided"];
    // (line, token) with comments and literals removed.
    let mut tokens: Vec<(usize, String)> = Vec::new();
    // Rust normalizes CRLF before lexing.
    let characters = text.replace("\r\n", "\n").chars().collect::<Vec<_>>();
    let (mut index, mut line) = (0, 1);
    while index < characters.len() {
        let character = characters[index];
        let next = characters.get(index + 1).copied();
        match character {
            '\n' => {
                line += 1;
                index += 1;
            }
            '/' if next == Some('/') => {
                while index < characters.len() && characters[index] != '\n' {
                    index += 1;
                }
            }
            '/' if next == Some('*') => {
                // Block comments nest in Rust.
                index += 2;
                let mut depth = 1;
                while index < characters.len() && depth > 0 {
                    match (characters[index], characters.get(index + 1)) {
                        ('/', Some('*')) => {
                            depth += 1;
                            index += 2;
                        }
                        ('*', Some('/')) => {
                            depth -= 1;
                            index += 2;
                        }
                        (character, _) => {
                            line += usize::from(character == '\n');
                            index += 1;
                        }
                    }
                }
            }
            '"' => {
                // Kept as one quoted token, so a `#[path]` target can be inspected.
                let (start, opening_line) = (index, line);
                index += 1;
                while index < characters.len() && characters[index] != '"' {
                    line += usize::from(characters[index] == '\n');
                    index += if characters[index] == '\\' { 2 } else { 1 };
                }
                index += 1;
                let end = index.min(characters.len());
                tokens.push((opening_line, characters[start..end].iter().collect()));
            }
            'r' if matches!(next, Some('"' | '#'))
                && !tokens.last().is_some_and(|(_, token)| token == "::") =>
            {
                let hashes = characters[index + 1..]
                    .iter()
                    .take_while(|character| **character == '#')
                    .count();
                if characters.get(index + 1 + hashes) != Some(&'"') {
                    // `r#ident` is a raw identifier: lex it as `ident`.
                    index += if hashes == 1 { 2 } else { 1 };
                    if hashes != 1 {
                        tokens.push((line, "r".to_owned()));
                    }
                    continue;
                }
                let opening_line = line;
                index += hashes + 2;
                let start = index;
                let close = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
                    .collect::<Vec<_>>();
                while index < characters.len() && !characters[index..].starts_with(&close) {
                    line += usize::from(characters[index] == '\n');
                    index += 1;
                }
                // Kept as a quoted token, like an ordinary string, for `#[path]` checks.
                let content = characters[start..index.min(characters.len())]
                    .iter()
                    .collect::<String>();
                tokens.push((opening_line, format!("r\"{content}\"")));
                index += close.len();
            }
            '\'' if next == Some('\\') || characters.get(index + 2) == Some(&'\'') => {
                index += 1;
                while index < characters.len() && characters[index] != '\'' {
                    index += if characters[index] == '\\' { 2 } else { 1 };
                }
                index += 1;
            }
            ':' if next == Some(':') => {
                tokens.push((line, "::".to_owned()));
                index += 2;
            }
            _ if character.is_alphanumeric() || character == '_' => {
                let start = index;
                while index < characters.len()
                    && (characters[index].is_alphanumeric() || characters[index] == '_')
                {
                    index += 1;
                }
                tokens.push((line, characters[start..index].iter().collect()));
            }
            // Rust's whitespace is `Pattern_White_Space`, not `char::is_whitespace`.
            '\t' | '\u{000B}' | '\u{000C}' | '\r' | ' ' | '\u{0085}' | '\u{200E}' | '\u{200F}'
            | '\u{2028}' | '\u{2029}' => index += 1,
            _ => {
                tokens.push((line, character.to_string()));
                index += 1;
            }
        }
    }
    let mut uses = Vec::new();
    let mut in_use = false;
    for (position, (line, token)) in tokens.iter().enumerate() {
        match token.as_str() {
            "use" => in_use = true,
            ";" => in_use = false,
            _ => {}
        }
        let before = position
            .checked_sub(1)
            .and_then(|previous| tokens.get(previous))
            .map(|(_, token)| token.as_str());
        let after = tokens.get(position + 1).map(|(_, token)| token.as_str());
        // `#[path = "…"]` naming manual code would compile it under another module name.
        if token == "path" && before == Some("[") && position >= 2 && tokens[position - 2].1 == "#"
        {
            let literal = tokens
                .get(position + 2)
                .map_or("", |(_, target)| target.as_str());
            // Compare the path Rust loads: ordinary strings decode their escapes.
            let target = literal
                .strip_prefix('r')
                .map_or_else(|| unescape(literal), str::to_owned);
            if [
                "manual_observation",
                "manual_readiness",
                "manual/",
                "manual\\",
                "manual.rs",
                "guided.rs",
            ]
            .iter()
            .any(|name| target.contains(name))
            {
                uses.push((*line, format!("#[path = {literal}]")));
            }
            continue;
        }
        // `mod manual_observation;` declares the module rather than using it.
        if before == Some("mod") {
            continue;
        }
        let generic_path = GENERIC.contains(&token.as_str())
            && (in_use || before == Some("::") || after == Some("::"));
        if DISTINCTIVE.contains(&token.as_str()) || generic_path {
            let segment = match (after, tokens.get(position + 2)) {
                (Some("::"), Some((_, next))) => format!("{token}::{next}"),
                _ => token.clone(),
            };
            uses.push((*line, segment));
        }
    }
    uses
}

/// The value of an ordinary string literal's body (between its quotes), as Rust reads it:
/// `\\`, `\"`, `\'`, `\n`, `\r`, `\t`, `\0`, `\xHH`, `\u{...}` and line continuations.
fn unescape(literal: &str) -> String {
    let body = literal
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(literal);
    let mut value = String::new();
    let mut characters = body.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '\\' {
            value.push(character);
            continue;
        }
        match characters.next() {
            Some('n') => value.push('\n'),
            Some('r') => value.push('\r'),
            Some('t') => value.push('\t'),
            Some('0') => value.push('\0'),
            Some('x') => {
                let hex = characters.by_ref().take(2).collect::<String>();
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    value.push(char::from(byte));
                }
            }
            Some('u') => {
                let hex = characters
                    .by_ref()
                    .skip_while(|character| *character == '{')
                    .take_while(|character| *character != '}')
                    .filter(|character| *character != '_')
                    .collect::<String>();
                if let Some(decoded) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    value.push(decoded);
                }
            }
            Some('\n') => {
                // A continuation skips ASCII whitespace only.
                while characters
                    .next_if(|character| matches!(character, ' ' | '\t' | '\n' | '\r'))
                    .is_some()
                {}
            }
            Some(other) => value.push(other),
            None => {}
        }
    }
    value
}

/// Nested comments, Rust's own whitespace and continuations over CRLF line ends hide
/// nothing and invent nothing.
#[test]
fn the_source_guard_lexes_comments_whitespace_and_continuations_as_rust_does() {
    let nested = "#[path /* outer /* inner */ still */ = \"manual/flow.rs\"]\nmod hidden;\n\
                  /* outer /* inner */ manual_observation::store */\n";
    assert_eq!(
        manual_module_paths(nested),
        [(1, r#"#[path = "manual/flow.rs"]"#.to_owned())]
    );
    let marked = "#[\u{200E}path = \"manual/flow.rs\"]\nmod hidden;\n";
    assert_eq!(
        manual_module_paths(marked),
        [(1, r#"#[path = "manual/flow.rs"]"#.to_owned())]
    );
    let continued = "#[path = \"manu\\\r\n    al_observation/registry.rs\"]\r\nmod hidden;\r\n";
    assert_eq!(manual_module_paths(continued).len(), 1);
    // Only ASCII whitespace is skipped after a continuation: this names no manual module.
    let spaced = "#[path = \"manu\\\n\u{2003}al_observation.rs\"]\nmod other;\n";
    assert!(manual_module_paths(spaced).is_empty());
}

#[test]
fn the_source_guard_sees_aliases_groups_macros_and_paths() {
    let source = r##"
        use super::manual_observation as observations;
        use super::{
            decisions,
            guided,
        };
        mod manual;
        // manual::execute in a comment
        /* manual_observation::store */
        let text = "manual::execute";
        let raw = r#"guided::STEPS"#;
        let character = '"';
        manual::execute(root);
        let manual = 1;
        let _ = cases!(manual_observation);
        #[path = "../manual_observation/registry.rs"]
        mod hidden;
        #[path = "priority_tests.rs"]
        mod tests;
        pub(super) mod manual_readiness;
        #[path = r"..\manual\flow.rs"]
        mod raw;
        #[path = "manu\u{61}l_observation/registry.rs"]
        mod escaped;
        #[path = "manu\u{6_1}l_readiness.rs"]
        mod underscored;
        #[r#path = "guided.rs"]
        mod raw_identifier;
    "##;
    assert_eq!(
        manual_module_paths(source),
        [
            (2, "manual_observation".to_owned()),
            (5, "guided".to_owned()),
            (13, "manual::execute".to_owned()),
            (15, "manual_observation".to_owned()),
            (
                16,
                r#"#[path = "../manual_observation/registry.rs"]"#.to_owned()
            ),
            (21, r#"#[path = r"..\manual\flow.rs"]"#.to_owned()),
            (
                23,
                r#"#[path = "manu\u{61}l_observation/registry.rs"]"#.to_owned()
            ),
            (25, r#"#[path = "manu\u{6_1}l_readiness.rs"]"#.to_owned()),
            (27, r#"#[path = "guided.rs"]"#.to_owned()),
        ]
    );
}
