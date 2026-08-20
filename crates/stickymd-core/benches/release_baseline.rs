//! Release-profile baseline for the Phase 2 document model.
//!
//! plan_ref: docs/plan/10_performance_reliability.md
//!
//! Measured during Phase 3 preflight (see
//! `docs/report/phase-02-core-document-model.md` § Phase 3 Preflight Release
//! Baseline). Fixtures are deterministic; warm-up and setup are excluded from
//! operation timing.

use std::time::{Duration, Instant};

use stickymd_core::{CursorSnapshot, DocumentState, Generation, InputKind, LineEnding, TextDelta};

const CHARS: &[char] = &[
    'a', 'b', 'c', '中', '文', 'x', '测', '试', 'd', 'e', '📝', 'f', '字', 'g',
];

fn fixture(bytes: usize) -> String {
    let mut s = String::with_capacity(bytes + 8);
    let mut i = 0usize;
    while s.len() < bytes {
        s.push(CHARS[i % CHARS.len()]);
        i += 1;
    }
    while s.len() > bytes {
        s.pop();
    }
    s
}

/// Find the nearest char boundary at or below `pos`.
fn boundary_floor(text: &str, pos: usize) -> usize {
    let mut p = pos.min(text.len());
    while !text.is_char_boundary(p) {
        p -= 1;
    }
    p
}

struct Stats {
    median: Duration,
    p95: Duration,
    max: Duration,
}

fn stats(samples: &mut [Duration]) -> Stats {
    samples.sort();
    let n = samples.len();
    Stats {
        median: samples[n / 2],
        p95: samples[((n as f64) * 0.95).ceil() as usize - 1],
        max: samples[n - 1],
    }
}

fn fmt(d: Duration) -> String {
    let ns = d.as_nanos();
    if ns >= 1_000_000 {
        format!("{:.2} ms", ns as f64 / 1_000_000.0)
    } else if ns >= 1_000 {
        format!("{:.2} µs", ns as f64 / 1_000.0)
    } else {
        format!("{ns} ns")
    }
}

fn measure<F: FnMut(&mut DocumentState)>(
    doc: &mut DocumentState,
    warmup: usize,
    iters: usize,
    mut op: F,
) -> Stats {
    for _ in 0..warmup {
        op(doc);
    }
    let mut samples = Vec::with_capacity(iters);
    for _ in 0..iters {
        let t = Instant::now();
        op(doc);
        samples.push(t.elapsed());
    }
    stats(&mut samples)
}

fn caret(offset: usize, g: Generation) -> CursorSnapshot {
    CursorSnapshot::caret(offset, g)
}

fn bench_size(label: &str, bytes: usize) {
    let mut doc = DocumentState::loaded(&fixture(bytes), LineEnding::Lf, None);
    // Time advances 1 s per edit so undo entries never merge (each op is its
    // own entry), matching worst-case single-keystroke undo cost.
    let mut now_ms: u64 = 0;

    let mut edit = |doc: &mut DocumentState, delta: TextDelta, kind: InputKind| {
        now_ms = now_ms.wrapping_add(1000);
        let g = doc.generation();
        let before = caret(delta.range.start, g);
        let after = caret(delta.caret_after(), g.next());
        doc.apply_delta(&delta, kind, now_ms, before, after)
            .expect("fixture deltas are valid");
    };

    let append: Stats = measure(&mut doc, 100, 1000, |d| {
        let pos = d.text().len();
        let delta = TextDelta::insert(pos, "字");
        edit(d, delta, InputKind::Typing);
    });

    let middle_insert: Stats = measure(&mut doc, 100, 1000, |d| {
        let pos = boundary_floor(d.text(), d.text().len() / 2);
        edit(d, TextDelta::insert(pos, "x"), InputKind::Typing);
    });

    let middle_delete: Stats = measure(&mut doc, 100, 1000, |d| {
        let len = d.text().len();
        let start = boundary_floor(d.text(), len / 2);
        let mut end = start + 1;
        while end < len && !d.text().is_char_boundary(end) {
            end += 1;
        }
        edit(d, TextDelta::new(start..end, ""), InputKind::Backspace);
    });

    let snapshot: Stats = measure(&mut doc, 50, 200, |d| {
        std::hint::black_box(d.snapshot());
    });

    let undo: Stats = measure(&mut doc, 20, 200, |d| {
        assert!(
            d.undo().expect("valid undo").is_some(),
            "undo stack is populated"
        );
    });

    let redo: Stats = measure(&mut doc, 20, 200, |d| {
        assert!(
            d.redo().expect("valid redo").is_some(),
            "redo stack is populated"
        );
    });

    println!("{label}: bytes={}", doc.text().len());
    println!(
        "  append        median={:>10} p95={:>10} max={:>10}",
        fmt(append.median),
        fmt(append.p95),
        fmt(append.max)
    );
    println!(
        "  middle_insert median={:>10} p95={:>10} max={:>10}",
        fmt(middle_insert.median),
        fmt(middle_insert.p95),
        fmt(middle_insert.max)
    );
    println!(
        "  middle_delete median={:>10} p95={:>10} max={:>10}",
        fmt(middle_delete.median),
        fmt(middle_delete.p95),
        fmt(middle_delete.max)
    );
    println!(
        "  snapshot      median={:>10} p95={:>10} max={:>10}",
        fmt(snapshot.median),
        fmt(snapshot.p95),
        fmt(snapshot.max)
    );
    println!(
        "  undo          median={:>10} p95={:>10} max={:>10}",
        fmt(undo.median),
        fmt(undo.p95),
        fmt(undo.max)
    );
    println!(
        "  redo          median={:>10} p95={:>10} max={:>10}",
        fmt(redo.median),
        fmt(redo.p95),
        fmt(redo.max)
    );
}

fn main() {
    bench_size("20 KiB", 20 * 1024);
    bench_size("100 KiB", 100 * 1024);
    bench_size("1 MiB", 1024 * 1024);
}
