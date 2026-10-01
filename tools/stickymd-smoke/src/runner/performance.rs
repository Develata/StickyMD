//! Cargo recipes for serial headless Release measurements.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use super::{Task, TaskId, cargo, push_unique};

// Keep Cargo's original feature-resolution contexts. A render-only library
// build is not interchangeable with the Windows product dependency graph.
const WINDOWS: &[&str] = &["-p", "stickymd-win"];
const RENDER_LIBRARY: &[&str] = &["-p", "stickymd-render", "--lib"];
const RENDER_WINDOWS: &[&str] = &["-p", "stickymd-render", "-p", "stickymd-win"];
// The smoke CLI has no Phase 6/7 performance cases. Selecting the product
// packages avoids linking its empty Release test targets (including fat LTO).
const PRODUCT: &[&str] = &[
    "-p",
    "stickymd-core",
    "-p",
    "stickymd-render",
    "-p",
    "stickymd-win",
];

fn release_test(
    id: TaskId,
    label: &'static str,
    selector: &[&'static str],
    filter: &'static str,
) -> Task {
    let mut args = vec!["test"];
    args.extend_from_slice(selector);
    args.extend([
        "--release",
        "--locked",
        filter,
        "--",
        "--ignored",
        "--nocapture",
        "--test-threads=1",
    ]);
    Task::Cargo { id, label, args }
}

pub(super) fn phase1_markdown_performance() -> Task {
    cargo(
        TaskId::Phase1MarkdownMathPerformance,
        "Phase 1 Markdown/Math Release measurement",
        &[
            "run",
            "--release",
            "--manifest-path",
            "experiments/phase-01/markdown-math/Cargo.toml",
            "--locked",
        ],
    )
}

pub(super) fn phase1_persistence_performance() -> Task {
    cargo(
        TaskId::Phase1PersistencePerformance,
        "Phase 1 persistence Release smoke",
        &[
            "run",
            "--release",
            "--manifest-path",
            "experiments/phase-01/persistence/Cargo.toml",
            "--locked",
        ],
    )
}

pub(super) fn phase2_performance() -> Task {
    cargo(
        TaskId::Phase2Performance,
        "Phase 2 core Release baseline",
        &[
            "bench",
            "-p",
            "stickymd-core",
            "--bench",
            "release_baseline",
            "--locked",
        ],
    )
}

pub(super) fn phase3_performance() -> Task {
    release_test(
        TaskId::Phase3Performance,
        "Phase 3 source-pipeline Release baseline",
        WINDOWS,
        "phase3_source_pipeline_release_baseline",
    )
}

pub(super) fn phase4_performance() -> Task {
    release_test(
        TaskId::Phase4Performance,
        "Phase 4 persistence Release baseline",
        WINDOWS,
        "phase4_persistence_release_baseline",
    )
}

pub(super) fn phase5_performance() -> Task {
    release_test(
        TaskId::Phase5Performance,
        "Phase 5 native-preview Release baseline",
        RENDER_LIBRARY,
        "phase5_preview_release_baseline",
    )
}

pub(super) fn phase6_performance() -> Task {
    release_test(
        TaskId::Phase6Performance,
        "Phase 6 native-math Release baseline",
        PRODUCT,
        "phase6_",
    )
}

pub(super) fn phase7_performance() -> Task {
    release_test(
        TaskId::Phase7Performance,
        "Phase 7 image/export Release baseline",
        PRODUCT,
        "phase7_",
    )
}

pub(super) fn phase8_performance() -> Task {
    release_test(
        TaskId::Phase8Performance,
        "Phase 8 native-window Release baseline",
        WINDOWS,
        "phase8_",
    )
}

pub(super) fn phase10_performance() -> Task {
    release_test(
        TaskId::Phase10Performance,
        "Phase 10 zoom/window Release baseline",
        RENDER_WINDOWS,
        "phase10_",
    )
}

pub(super) fn phase11b_performance() -> Task {
    release_test(
        TaskId::Phase11BPerformance,
        "Phase 11-B semantic-conversion Release baseline",
        RENDER_LIBRARY,
        "phase11b_performance_",
    )
}

pub(super) fn phase14_performance() -> Task {
    release_test(
        TaskId::Phase14Performance,
        "Phase 14 viewport selection Release baseline",
        RENDER_LIBRARY,
        "phase14_preview_selection_geometry_release_baseline",
    )
}

pub(super) fn phase14_search_performance() -> Task {
    release_test(
        TaskId::Phase14SearchPerformance,
        "Phase 14 source-search Release baseline",
        WINDOWS,
        "phase14_one_mib_unicode_case_insensitive_search_p95_is_bounded",
    )
}

pub(super) fn push_source_performance(tasks: &mut Vec<Task>) {
    push_unique(tasks, phase3_performance());
    push_unique(
        tasks,
        release_test(
            TaskId::SourceScrollbarPerformance,
            "Phase 3 source scrollbar Release baseline",
            RENDER_LIBRARY,
            "scrollbar_release_baseline",
        ),
    );
}
