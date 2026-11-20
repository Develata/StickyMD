//! One pass of the `$` math action as pure text: equals-line join, then
//! delimiter conversion of the joined text.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#semantic-math-delimiter-conversion
//!
//! The two steps stay independent modules; this only composes them and their
//! position maps. Callers own the generation check, the transaction and the
//! selection.

use std::ops::Range;

use stickymd_core::DocumentSnapshot;

use crate::math_text::{EqualsLineConversion, join_math_equals_lines};
use crate::preview::{
    MAX_PREVIEW_SOURCE_BYTES, MathDelimiterConversion, PreviewParseError, SemanticConversionError,
    convert_latex_math_delimiters,
};

/// The result of one pass that changed the text.
#[derive(Debug)]
pub enum MathActionPass {
    /// Only step 1 changed the text.
    Joined(EqualsLineConversion),
    /// Step 2 changed the text, after step 1 if it did.
    Converted(Option<EqualsLineConversion>, MathDelimiterConversion),
}

impl MathActionPass {
    /// How many places the pass changed, for the notice that reports it: the formulas whose
    /// delimiters it converted, or the equals lines it joined when that is all it did.
    pub fn change_count(&self) -> usize {
        match self {
            Self::Joined(joined) => joined.joined_lines(),
            Self::Converted(_, converted) => converted.replacement_count(),
        }
    }

    pub fn text(&self) -> &str {
        match self {
            Self::Joined(joined) => joined.text(),
            Self::Converted(_, converted) => converted.text(),
        }
    }

    pub fn into_text(self) -> String {
        match self {
            Self::Joined(joined) => joined.into_text(),
            Self::Converted(_, converted) => converted.into_text(),
        }
    }

    /// Maps a byte position of the pass input through both steps.
    pub fn map_position(&self, position: usize) -> usize {
        match self {
            Self::Joined(joined) => joined.map_position(position),
            Self::Converted(joined, converted) => converted.map_position(
                joined
                    .as_ref()
                    .map_or(position, |joined| joined.map_position(position)),
            ),
        }
    }
}

/// Runs step 1 on `snapshot` and step 2 on its result; a scope is mapped
/// between them. Returns None when neither step changes anything.
pub fn run_math_action(
    snapshot: &DocumentSnapshot,
    scope: Option<Range<usize>>,
) -> Result<Option<MathActionPass>, SemanticConversionError> {
    // Step 2 refuses such a source before it parses; step 1 must not parse
    // it first.
    if snapshot.text.len() > MAX_PREVIEW_SOURCE_BYTES {
        return Err(PreviewParseError::SourceTooLarge {
            actual: snapshot.text.len(),
            limit: MAX_PREVIEW_SOURCE_BYTES,
        }
        .into());
    }
    let joined = join_math_equals_lines(&snapshot.text, scope.clone());
    let joined_snapshot = joined.as_ref().map(|joined| DocumentSnapshot {
        text: joined.text().into(),
        generation: snapshot.generation,
        line_ending: snapshot.line_ending,
    });
    let source = joined_snapshot.as_ref().unwrap_or(snapshot);
    let scope = scope.map(|range| match &joined {
        Some(joined) => joined.map_position(range.start)..joined.map_position(range.end),
        None => range,
    });
    Ok(match convert_latex_math_delimiters(source, scope)? {
        Some(converted) => Some(MathActionPass::Converted(joined, converted)),
        None => joined.map(MathActionPass::Joined),
    })
}

#[cfg(test)]
mod tests {
    /// The notice counts one place per joined equals line when that is all a pass did, and one
    /// per converted formula otherwise.
    #[test]
    fn a_pass_counts_joined_lines_or_converted_formulas() {
        let pass = |text: &str| {
            let snapshot = stickymd_core::DocumentSnapshot {
                text: std::sync::Arc::from(text),
                generation: stickymd_core::Generation::initial(),
                line_ending: stickymd_core::LineEnding::Lf,
            };
            super::run_math_action(&snapshot, None)
                .unwrap()
                .expect("the pass changes the text")
        };
        assert_eq!(pass("$$a\n=\nb$$").change_count(), 1);
        assert_eq!(pass("$$a\n=\nb\n=\nc$$").change_count(), 2);
        assert_eq!(pass("$$a\n=\nb$$\n\n$$c\n=\nd$$").change_count(), 2);
        assert_eq!(pass("\\(x\\) and \\(y\\)").change_count(), 2);
    }

    use std::time::{Duration, Instant};

    use comrak::{Arena, parse_document};
    use stickymd_core::{DocumentSnapshot, Generation, LineEnding};

    use super::run_math_action;
    use crate::preview::markdown_options;

    /// 1000 units that each need both steps, leave a bare `=` line outside a
    /// formula (so the second pass parses in full) and hold a paragraph that
    /// starts with a link reference definition (so both steps look for
    /// shifted blocks), padded to 1 MiB with `filler`.
    fn workload(filler: &str) -> String {
        let unit = "\\[\nx^2\n=\ny^2\n\\]\n\nHeading\n=\ntext\n\n[r]: u\nt \\(q\\)\n\n";
        let mut source = unit.repeat(1_000);
        source.push_str(&filler.repeat((1024 * 1024 - source.len()) / filler.len()));
        while source.len() < 1024 * 1024 {
            source.push('x');
        }
        source
    }

    fn snapshot(text: &str) -> DocumentSnapshot {
        DocumentSnapshot {
            text: text.into(),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        }
    }

    /// What `convert_math_delimiters` runs, minus the commit: one pass, then
    /// the second pass that checks the result.
    fn button(input: &DocumentSnapshot) -> Duration {
        let started = Instant::now();
        let pass = run_math_action(input, None).unwrap().unwrap();
        let again = run_math_action(&snapshot(pass.text()), None).unwrap();
        let elapsed = started.elapsed();
        assert!(again.is_none());
        assert_eq!(pass.text().matches("$$\nx^2=y^2\n$$").count(), 1_000);
        assert_eq!(pass.text().matches("Heading\n=\ntext").count(), 1_000);
        assert_eq!(pass.text().matches("t \\(q\\)").count(), 1_000);
        elapsed
    }

    fn summary(samples: &mut [Duration]) -> (Duration, Duration, Duration) {
        samples.sort_unstable();
        // Nearest-rank p95: the ceil(0.95 n)-th smallest sample.
        let p95_rank = (samples.len() * 95).div_ceil(100);
        (
            samples[samples.len() / 2],
            samples[p95_rank - 1],
            samples[samples.len() - 1],
        )
    }

    #[test]
    fn phase11b_math_action_refuses_an_oversized_source_before_joining() {
        // One byte over the limit, and back under it once the join removes
        // two newlines: only a check before step 1 refuses it.
        let mut text = "x".repeat(super::MAX_PREVIEW_SOURCE_BYTES - 10);
        text.push_str("\n\n$$a\n=\nb$$");
        assert_eq!(text.len(), super::MAX_PREVIEW_SOURCE_BYTES + 1);
        let result = run_math_action(&snapshot(&text), None);
        assert!(
            matches!(
                result,
                Err(super::SemanticConversionError::Parse(
                    super::PreviewParseError::SourceTooLarge { .. }
                ))
            ),
            "{result:?}"
        );
    }

    /// One Comrak parse of `source` with the Preview's options.
    fn parse_once(source: &str) -> Duration {
        let started = Instant::now();
        let arena = Arena::new();
        std::hint::black_box(parse_document(&arena, source, &markdown_options()));
        started.elapsed()
    }

    /// Plan 07's two checks, after three untimed rounds, over 100 timed
    /// clicks that each follow a timed parse of the same document (so a stall
    /// slows both alike):
    ///
    /// - regression: the click median is at most 10 parse medians. The ratio
    ///   removes the host's speed, so a fast machine catches waste too;
    /// - fluency: the nearest-rank p95 (the 95th sample) stays under 100 ms.
    ///   Up to five samples may exceed it, so a stall of a shared CI host that
    ///   slows two or three clicks does not fail it.
    ///
    /// Samples print in run order to show such stalls.
    #[test]
    #[ignore = "Release-only Phase 11-B math button receipt"]
    fn phase11b_performance_math_action_one_mib() {
        let source = workload("plain text filler ");
        let input = snapshot(&source);
        for _ in 0..3 {
            parse_once(&source);
            button(&input);
        }
        let mut parses = Vec::with_capacity(100);
        let mut samples = Vec::with_capacity(100);
        for _ in 0..100 {
            parses.push(parse_once(&source));
            samples.push(button(&input));
        }
        let in_run_order: Vec<_> = samples
            .iter()
            .map(|sample| format!("{:.1}", sample.as_secs_f64() * 1e3))
            .collect();
        eprintln!(
            "phase11b math_action_1mib_1000_math samples_ms=[{}]",
            in_run_order.join(",")
        );
        let (parse, _, _) = summary(&mut parses);
        let (median, p95, max) = summary(&mut samples);
        eprintln!("phase11b math_action_1mib_1000_math median={median:?} p95={p95:?} max={max:?}");
        eprintln!(
            "phase11b math_action_1mib_1000_math parse_median={parse:?} median_over_parse={:.2}",
            median.as_secs_f64() / parse.as_secs_f64()
        );
        assert!(median <= parse * 10, "median={median:?} parse={parse:?}");
        assert!(p95 < Duration::from_millis(100), "p95={p95:?}");
    }

    #[test]
    #[ignore = "Release-only Phase 11-B math button receipt"]
    fn phase11b_performance_math_action_stays_linear_in_shifted_paragraphs() {
        // 23,717 paragraphs that start with a definition, under the Preview's
        // node limit. Parsing so many blocks takes far longer than one large
        // paragraph, so this guards the shape, not the 100 ms bound: the
        // button parses four times, while a per-node search over past
        // shifted paragraphs would cost hundreds of parses.
        let source = workload("[r]: u\nplain text filler plain text filler\n\n");
        let input = snapshot(&source);
        let mut parses: Vec<_> = (0..9).map(|_| parse_once(&source)).collect();
        let mut samples: Vec<_> = (0..9).map(|_| button(&input)).collect();
        let (parse, _, _) = summary(&mut parses);
        let (median, p95, max) = summary(&mut samples);
        eprintln!(
            "phase11b math_action_1mib_shifted_paragraphs parse_median={parse:?} \
             median={median:?} p95={p95:?} max={max:?}"
        );
        assert!(median < parse * 12, "median={median:?} parse={parse:?}");
    }
}
