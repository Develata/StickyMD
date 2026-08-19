//! Property-style tests for the core document model.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#property（property-based）
//!
//! These use a deterministic in-test PRNG (no external proptest dependency) to
//! exercise the two core properties:
//!   1. Arbitrary Unicode `TextDelta`s never break UTF-8 or panic.
//!   2. Undo restores the original text; redo restores the edited text.

use stickymd_core::{CursorSnapshot, DocumentState, InputKind, LineEnding, TextDelta};

// Small deterministic xorshift PRNG so the property tests are reproducible.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

// ASCII + CJK + emoji + combining marks + accented/symbol letters.
const CORPUS: &[char] = &[
    'a', 'b', 'c', 'Z', '9', ' ', '\n', '#', 'é', 'ñ', 'ß', 'Ω', 'Ж', 'あ', 'ア', '한', '你', '好',
    '世', '界', '中', '文', '草', '稿', '😀', '🎉', '🚀', '\u{0301}', '\u{0327}',
];

fn char_boundaries(text: &str) -> Vec<usize> {
    text.char_indices()
        .map(|(i, _)| i)
        .chain([text.len()])
        .collect()
}

fn random_insert(rng: &mut Rng) -> String {
    let n = 1 + rng.below(3);
    (0..n).map(|_| CORPUS[rng.below(CORPUS.len())]).collect()
}

/// Apply `ops` random insert/delete deltas at a fixed `time_step` cadence, asserting
/// UTF-8 validity throughout and that a full undo run returns exactly to the starting
/// text, then redo returns to the final text. `time_step` controls merging: a large
/// step keeps every edit as its own entry; a small step lets adjacent edits merge.
///
/// `ops` must stay under `MAX_UNDO_ENTRIES` (256) so no entry is evicted; eviction is
/// covered by dedicated unit tests, and evicted history cannot round-trip by design.
fn fuzz_roundtrip(seed: u64, ops: usize, time_step: u64) {
    let mut rng = Rng(seed);
    let mut doc = DocumentState::empty(LineEnding::Lf);
    let original = doc.text().to_string();
    let mut now: u64 = 0;

    let mut applied = 0usize;
    for _ in 0..ops {
        now = now.wrapping_add(time_step);
        let bounds = char_boundaries(doc.text());
        let text_len = doc.text().len();

        // Choose insert (2/3) or single-char delete (1/3) when possible.
        let do_delete = text_len > 0 && rng.below(3) == 0;
        if do_delete {
            // Delete one character: pick a boundary > 0 and remove [prev, idx).
            let idx_pos = 1 + rng.below(bounds.len() - 1);
            let end = bounds[idx_pos];
            let start = bounds[idx_pos - 1];
            let delta = TextDelta::new(start..end, "");
            let before = CursorSnapshot::caret(end, doc.generation());
            let after = CursorSnapshot::caret(start, doc.generation());
            doc.apply_delta(&delta, InputKind::Backspace, now, before, after)
                .expect("delete must apply on a valid boundary");
        } else {
            let start = bounds[rng.below(bounds.len())];
            let ins = random_insert(&mut rng);
            let delta = TextDelta::insert(start, ins.clone());
            let before = CursorSnapshot::caret(start, doc.generation());
            let after = CursorSnapshot::caret(start + ins.len(), doc.generation());
            doc.apply_delta(&delta, InputKind::Typing, now, before, after)
                .expect("insert must apply on a valid boundary");
        }
        applied += 1;

        // Property 1: the text is always valid UTF-8 and never panics to read.
        assert!(std::str::from_utf8(doc.text().as_bytes()).is_ok());
    }
    assert!(applied > 0);
    let final_text = doc.text().to_string();

    // Property 2a: undoing everything restores the original text.
    let mut undo_steps = 0usize;
    while doc.can_undo() {
        doc.undo().expect("undo must succeed");
        undo_steps += 1;
        assert!(std::str::from_utf8(doc.text().as_bytes()).is_ok());
    }
    assert_eq!(doc.text(), original, "full undo must restore the original");

    // Property 2b: redoing everything restores the final text.
    let mut redo_steps = 0usize;
    while doc.can_redo() {
        doc.redo().expect("redo must succeed");
        redo_steps += 1;
    }
    assert_eq!(
        doc.text(),
        final_text,
        "full redo must restore the edited text"
    );
    assert_eq!(undo_steps, redo_steps);
}

#[test]
fn unicode_deltas_preserve_utf8_and_undo_redo_roundtrip() {
    // No merging (large time step): every edit is its own entry, all retained.
    for seed in 1..=8u64 {
        fuzz_roundtrip(seed, 120, 1000);
    }
}

#[test]
fn unicode_deltas_roundtrip_with_merge_grouping() {
    // Small time step: adjacent same-kind edits merge; round-trip must still hold.
    for seed in 1..=8u64 {
        fuzz_roundtrip(seed, 150, 100);
    }
}

#[test]
fn cjk_emoji_combining_marks_roundtrip() {
    let mut doc = DocumentState::empty(LineEnding::Lf);
    let pieces = ["你好", "😀🎉", "e\u{0301}", "世界🚀", "ñßΩ"];
    let mut off = 0usize;
    for (i, p) in pieces.iter().enumerate() {
        let delta = TextDelta::insert(off, *p);
        let before = CursorSnapshot::caret(off, doc.generation());
        let after = CursorSnapshot::caret(off + p.len(), doc.generation());
        doc.apply_delta(&delta, InputKind::ImeCommit, i as u64 * 1000, before, after)
            .unwrap();
        off += p.len();
    }
    let joined = pieces.concat();
    assert_eq!(doc.text(), joined);
    // Each IME commit is its own undo step; undoing all restores empty.
    while doc.can_undo() {
        doc.undo().unwrap();
    }
    assert_eq!(doc.text(), "");
}

#[test]
fn non_boundary_delta_never_applies() {
    let mut doc = DocumentState::loaded("héllo😀", LineEnding::Lf, None);
    let before = doc.text().to_string();
    // Byte 2 is inside 'é'; this must be rejected and leave text untouched.
    let bad = TextDelta::new(1..2, "x");
    let c = CursorSnapshot::caret(1, doc.generation());
    assert!(
        doc.apply_delta(&bad, InputKind::Typing, 0, c.clone(), c)
            .is_err()
    );
    assert_eq!(doc.text(), before);
}

/// Performance smoke: append and middle-insert latency on large documents.
///
/// plan_ref: docs/plan/10_performance_reliability.md
///
/// This is a smoke gate, not a precise benchmark: it asserts generous upper bounds
/// to catch catastrophic regressions without being flaky in CI.
#[test]
fn perf_smoke_apply_latency() {
    use std::time::Instant;

    let target_bytes = 1024 * 1024; // 1 MiB
    let mut body = String::new();
    let line = "The quick brown fox jumps over the lazy dog. 这是中文示例，含标点。\n";
    while body.len() < target_bytes {
        body.push_str(line);
    }
    let mut doc = DocumentState::loaded(&body, LineEnding::Lf, None);

    // Append at the end (amortized cheap).
    let iters = 200;
    let start = Instant::now();
    for i in 0..iters {
        let off = doc.text().len();
        let delta = TextDelta::insert(off, "x");
        let before = CursorSnapshot::caret(off, doc.generation());
        let after = CursorSnapshot::caret(off + 1, doc.generation());
        doc.apply_delta(&delta, InputKind::Typing, i, before, after)
            .unwrap();
    }
    let append_ns = start.elapsed().as_nanos() as f64 / iters as f64;

    // Middle insert (O(n) shift) on the 1 MiB document.
    let mid = doc.text().len() / 2;
    let iters_mid = 50;
    let start = Instant::now();
    for i in 0..iters_mid {
        let delta = TextDelta::insert(mid, "y");
        let before = CursorSnapshot::caret(mid, doc.generation());
        let after = CursorSnapshot::caret(mid + 1, doc.generation());
        doc.apply_delta(&delta, InputKind::Typing, 10_000 + i, before, after)
            .unwrap();
    }
    let mid_ns = start.elapsed().as_nanos() as f64 / iters_mid as f64;

    println!(
        "perf_smoke: doc={}B append≈{:.0}ns/op middle_insert≈{:.0}ns/op",
        doc.text().len(),
        append_ns,
        mid_ns
    );
    // Generous gates: append well under 1 ms; middle insert on 1 MiB under 50 ms.
    assert!(append_ns < 1_000_000.0, "append too slow: {append_ns}ns");
    assert!(mid_ns < 50_000_000.0, "middle insert too slow: {mid_ns}ns");
}
