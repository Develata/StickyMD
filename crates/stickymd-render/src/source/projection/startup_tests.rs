//! Opt-in initialization timings, without native windows or acceptance receipts.
//! plan_ref: docs/plan/10_performance_reliability.md#initial-engineering-targets

use super::*;
use std::{hint::black_box, sync::Arc, time::Instant};
use stickymd_core::LineEnding;

#[test]
#[ignore = "Release-only Source/font initialization investigation; no GUI or qualification receipt"]
fn source_initialization_release_baseline() {
    if cfg!(debug_assertions) {
        panic!("run with --release");
    }
    let line = "中文 English office e\u{301} 🙂 source paragraph.\n";
    let text = line.repeat((20_usize * 1024).div_ceil(line.len()));
    let snapshot = DocumentSnapshot {
        text: Arc::from(text),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    };
    for run in 1..=10 {
        let started = Instant::now();
        let mut milestones = Vec::new();
        let projection = SourceProjection::new_observed(&snapshot, 780, 948, 1.5, |stage| {
            milestones.push((stage, started.elapsed()));
        });
        println!(
            "source_init run={run} bytes={} font_ms={:.3} buffer_ms={:.3} shape_ms={:.3} total_ms={:.3} faces={} cjk={:?} latin={:?}",
            snapshot.text.len(),
            milestones[0].1.as_secs_f64() * 1000.0,
            (milestones[1].1 - milestones[0].1).as_secs_f64() * 1000.0,
            (milestones[2].1 - milestones[1].1).as_secs_f64() * 1000.0,
            started.elapsed().as_secs_f64() * 1000.0,
            projection.font_system.db().faces().count(),
            projection.fonts.cjk_family,
            projection.fonts.latin_family,
        );
        black_box(projection);
    }

    // Isolate first font loading from document layout. This is an attribution
    // probe, not an alternate product initialization or an acceptance sample.
    for run in 1..=5 {
        let started = Instant::now();
        let mut font_system = FontSystem::new();
        let scan = started.elapsed();
        let fonts = FontSelection::resolve(&mut font_system);
        let selected = started.elapsed();
        println!(
            "font_scan run={run} scan_ms={:.3} selection_ms={:.3}",
            scan.as_secs_f64() * 1000.0,
            (selected - scan).as_secs_f64() * 1000.0,
        );
        for family in [fonts.latin_family, fonts.cjk_family] {
            let attrs = Attrs::new().family(Family::Name(family));
            let started = Instant::now();
            let matches = font_system.get_font_matches(&attrs);
            let first = started.elapsed();
            let started = Instant::now();
            let cached = font_system.get_font_matches(&attrs);
            println!(
                "font_match run={run} family={family:?} first_ms={:.3} cached_ms={:.3} candidates={}",
                first.as_secs_f64() * 1000.0,
                started.elapsed().as_secs_f64() * 1000.0,
                matches.len(),
            );
            assert!(Arc::ptr_eq(&matches, &cached));
            let query = cosmic_text::fontdb::Query {
                families: &[Family::Name(family)],
                ..Default::default()
            };
            if let Some(id) = font_system.db().query(&query) {
                let started = Instant::now();
                let font = font_system.get_font(id, cosmic_text::fontdb::Weight::NORMAL);
                println!(
                    "font_load run={run} family={family:?} available={} elapsed_ms={:.3}",
                    font.is_some(),
                    started.elapsed().as_secs_f64() * 1000.0,
                );
                black_box(font);
            }
        }
    }
}
