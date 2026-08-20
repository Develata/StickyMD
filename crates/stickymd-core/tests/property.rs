//! Deterministic property-style tests for the canonical document model.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#property-tests

use stickymd_core::{
    CursorSnapshot, DocumentError, DocumentState, EditKind, EditMeta, EditRequest, LineEnding,
};

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

const CORPUS: &[char] = &[
    'a', 'b', 'c', 'Z', '9', ' ', '\n', '#', 'é', 'ñ', 'ß', 'Ω', 'Ж', 'あ', 'ア', '한', '你', '好',
    '世', '界', '中', '文', '草', '稿', '😀', '🎉', '🚀', '\u{0301}', '\u{0327}',
];

fn char_boundaries(text: &str) -> Vec<usize> {
    text.char_indices()
        .map(|(index, _)| index)
        .chain([text.len()])
        .collect()
}

fn random_insert(rng: &mut Rng) -> String {
    let count = 1 + rng.below(3);
    (0..count)
        .map(|_| CORPUS[rng.below(CORPUS.len())])
        .collect()
}

fn edit(
    doc: &mut DocumentState,
    range: std::ops::Range<usize>,
    inserted: impl Into<String>,
    kind: EditKind,
    timestamp_ms: u64,
    before: CursorSnapshot,
    after: CursorSnapshot,
) {
    let request = EditRequest::new(
        doc.generation(),
        range,
        inserted,
        before,
        after,
        EditMeta::new(kind, timestamp_ms),
    );
    doc.edit(request).expect("generated edit must be valid");
}

fn fuzz_roundtrip(seed: u64, operations: usize, time_step_ms: u64) {
    let mut rng = Rng(seed);
    let mut doc = DocumentState::empty(LineEnding::Lf);
    let original = doc.text().to_owned();
    let mut now_ms = 0u64;

    for _ in 0..operations {
        now_ms += time_step_ms;
        let boundaries = char_boundaries(doc.text());
        let delete = !doc.text().is_empty() && rng.below(3) == 0;

        if delete {
            let end_position = 1 + rng.below(boundaries.len() - 1);
            let start = boundaries[end_position - 1];
            let end = boundaries[end_position];
            edit(
                &mut doc,
                start..end,
                "",
                EditKind::Backspace,
                now_ms,
                CursorSnapshot::caret(end),
                CursorSnapshot::caret(start),
            );
        } else {
            let start = boundaries[rng.below(boundaries.len())];
            let inserted = random_insert(&mut rng);
            let end = start + inserted.len();
            edit(
                &mut doc,
                start..start,
                inserted,
                EditKind::Typing,
                now_ms,
                CursorSnapshot::caret(start),
                CursorSnapshot::caret(end),
            );
        }

        assert!(std::str::from_utf8(doc.text().as_bytes()).is_ok());
    }

    let final_text = doc.text().to_owned();
    let mut undo_count = 0;
    while doc.can_undo() {
        doc.undo().expect("retained undo entry must apply");
        undo_count += 1;
        assert!(std::str::from_utf8(doc.text().as_bytes()).is_ok());
    }
    assert_eq!(doc.text(), original);

    let mut redo_count = 0;
    while doc.can_redo() {
        doc.redo().expect("retained redo entry must apply");
        redo_count += 1;
    }
    assert_eq!(doc.text(), final_text);
    assert_eq!(undo_count, redo_count);
}

#[test]
fn deterministic_unicode_edits_roundtrip_without_grouping() {
    for seed in 1..=8 {
        fuzz_roundtrip(seed, 120, 1_000);
    }
}

#[test]
fn deterministic_unicode_edits_roundtrip_with_grouping() {
    for seed in 1..=8 {
        fuzz_roundtrip(seed, 150, 100);
    }
}

#[test]
fn cjk_emoji_and_combining_sequences_remain_byte_exact() {
    let pieces = ["你好", "😀🎉", "e\u{0301}", "世界🚀", "ñßΩ"];
    let mut doc = DocumentState::empty(LineEnding::Lf);
    let mut offset = 0;

    for (index, piece) in pieces.iter().enumerate() {
        edit(
            &mut doc,
            offset..offset,
            *piece,
            EditKind::ImeCommit,
            index as u64 * 1_000,
            CursorSnapshot::caret(offset),
            CursorSnapshot::caret(offset + piece.len()),
        );
        offset += piece.len();
    }

    assert_eq!(doc.text(), pieces.concat());
    while doc.can_undo() {
        doc.undo()
            .expect("IME commit must be independently undoable");
    }
    assert!(doc.text().is_empty());
}

#[test]
fn non_boundary_edit_is_rejected_without_mutation() {
    let mut doc = DocumentState::loaded("héllo😀", LineEnding::Lf, None);
    let before = doc.snapshot();
    let request = EditRequest::new(
        doc.generation(),
        1..2,
        "x",
        CursorSnapshot::caret(1),
        CursorSnapshot::caret(2),
        EditMeta::new(EditKind::Typing, 0),
    );

    assert_eq!(doc.edit(request), Err(DocumentError::InvalidCharBoundary));
    assert_eq!(doc.snapshot(), before);
    assert!(!doc.can_undo());
}

#[test]
fn one_mebibyte_common_edits_stay_below_smoke_threshold() {
    use std::time::Instant;

    let mut body = String::new();
    let line = "The quick brown fox jumps over the lazy dog. 这是中文示例，含标点。\n";
    while body.len() < 1024 * 1024 {
        body.push_str(line);
    }
    let mut doc = DocumentState::loaded(&body, LineEnding::Lf, None);

    let iterations = 200;
    let started = Instant::now();
    for timestamp_ms in 0..iterations {
        let offset = doc.text().len();
        edit(
            &mut doc,
            offset..offset,
            "x",
            EditKind::Typing,
            timestamp_ms,
            CursorSnapshot::caret(offset),
            CursorSnapshot::caret(offset + 1),
        );
    }
    let append_ns = started.elapsed().as_nanos() as f64 / iterations as f64;

    let iterations = 50;
    let started = Instant::now();
    for timestamp_ms in 0..iterations {
        let middle = doc.text().len() / 2;
        edit(
            &mut doc,
            middle..middle,
            "y",
            EditKind::Typing,
            10_000 + timestamp_ms,
            CursorSnapshot::caret(middle),
            CursorSnapshot::caret(middle + 1),
        );
    }
    let middle_ns = started.elapsed().as_nanos() as f64 / iterations as f64;

    eprintln!(
        "perf_smoke: doc={}B append≈{append_ns:.0}ns/op middle_insert≈{middle_ns:.0}ns/op",
        doc.text().len()
    );
    assert!(append_ns < 1_000_000.0);
    assert!(middle_ns < 50_000_000.0);
}
