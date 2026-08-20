//! Release-profile baseline for the Phase 2 document model.
//!
//! plan_ref: docs/plan/10_performance_reliability.md#initial-engineering-targets

use std::time::{Duration, Instant};

use stickymd_core::{CursorSnapshot, DocumentState, EditKind, EditMeta, EditRequest, LineEnding};

const CHARS: &[char] = &[
    'a', 'b', 'c', '中', '文', 'x', '测', '试', 'd', 'e', '📝', 'f', '字', 'g',
];

fn fixture(bytes: usize) -> String {
    let mut text = String::with_capacity(bytes + 8);
    let mut index = 0;
    while text.len() < bytes {
        text.push(CHARS[index % CHARS.len()]);
        index += 1;
    }
    while text.len() > bytes {
        text.pop();
    }
    text
}

fn boundary_floor(text: &str, position: usize) -> usize {
    let mut position = position.min(text.len());
    while !text.is_char_boundary(position) {
        position -= 1;
    }
    position
}

struct Stats {
    median: Duration,
    p95: Duration,
    max: Duration,
}

fn stats(samples: &mut [Duration]) -> Stats {
    samples.sort_unstable();
    let len = samples.len();
    Stats {
        median: samples[len / 2],
        p95: samples[((len as f64) * 0.95).ceil() as usize - 1],
        max: samples[len - 1],
    }
}

fn format_duration(duration: Duration) -> String {
    let nanos = duration.as_nanos();
    if nanos >= 1_000_000 {
        format!("{:.2} ms", nanos as f64 / 1_000_000.0)
    } else if nanos >= 1_000 {
        format!("{:.2} µs", nanos as f64 / 1_000.0)
    } else {
        format!("{nanos} ns")
    }
}

fn measure<F: FnMut(&mut DocumentState)>(
    doc: &mut DocumentState,
    warmup: usize,
    iterations: usize,
    mut operation: F,
) -> Stats {
    for _ in 0..warmup {
        operation(doc);
    }
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let started = Instant::now();
        operation(doc);
        samples.push(started.elapsed());
    }
    stats(&mut samples)
}

fn apply_edit(
    doc: &mut DocumentState,
    range: std::ops::Range<usize>,
    inserted: &str,
    kind: EditKind,
    timestamp_ms: u64,
) {
    let before = CursorSnapshot::caret(range.start);
    let after = CursorSnapshot::caret(range.start + inserted.len());
    let request = EditRequest::new(
        doc.generation(),
        range,
        inserted,
        before,
        after,
        EditMeta::new(kind, timestamp_ms),
    );
    doc.edit(request).expect("benchmark edit must be valid");
}

fn print_stats(label: &str, stats: Stats) {
    println!(
        "  {label:<13} median={:>10} p95={:>10} max={:>10}",
        format_duration(stats.median),
        format_duration(stats.p95),
        format_duration(stats.max)
    );
}

fn bench_size(label: &str, bytes: usize) {
    let mut doc = DocumentState::loaded(&fixture(bytes), LineEnding::Lf, None);
    let mut timestamp_ms = 0u64;

    let append = measure(&mut doc, 100, 1_000, |doc| {
        timestamp_ms += 1_000;
        let position = doc.text().len();
        apply_edit(
            doc,
            position..position,
            "字",
            EditKind::Typing,
            timestamp_ms,
        );
    });

    let middle_insert = measure(&mut doc, 100, 1_000, |doc| {
        timestamp_ms += 1_000;
        let position = boundary_floor(doc.text(), doc.text().len() / 2);
        apply_edit(doc, position..position, "x", EditKind::Typing, timestamp_ms);
    });

    let middle_delete = measure(&mut doc, 100, 1_000, |doc| {
        timestamp_ms += 1_000;
        let start = boundary_floor(doc.text(), doc.text().len() / 2);
        let end = doc.text()[start..]
            .char_indices()
            .nth(1)
            .map_or(doc.text().len(), |(offset, _)| start + offset);
        apply_edit(doc, start..end, "", EditKind::Backspace, timestamp_ms);
    });

    let snapshot = measure(&mut doc, 50, 200, |doc| {
        std::hint::black_box(doc.snapshot());
    });
    let undo = measure(&mut doc, 20, 200, |doc| {
        std::hint::black_box(doc.undo().expect("undo history must be populated"));
    });
    let redo = measure(&mut doc, 20, 200, |doc| {
        std::hint::black_box(doc.redo().expect("redo history must be populated"));
    });

    println!("{label}: bytes={}", doc.text().len());
    print_stats("append", append);
    print_stats("middle_insert", middle_insert);
    print_stats("middle_delete", middle_delete);
    print_stats("snapshot", snapshot);
    print_stats("undo", undo);
    print_stats("redo", redo);
}

fn main() {
    bench_size("20 KiB", 20 * 1024);
    bench_size("100 KiB", 100 * 1024);
    bench_size("1 MiB", 1024 * 1024);
}
