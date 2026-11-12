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
    MathDelimiterConversion, SemanticConversionError, convert_latex_math_delimiters,
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
        (
            samples[samples.len() / 2],
            samples[samples.len() * 95 / 100],
            samples[samples.len() - 1],
        )
    }

    #[test]
    #[ignore = "Release-only Phase 11-B math button receipt"]
    fn phase11b_performance_math_action_one_mib() {
        let input = snapshot(&workload("plain text filler "));
        let mut samples: Vec<_> = (0..25).map(|_| button(&input)).collect();
        let (median, p95, max) = summary(&mut samples);
        eprintln!("phase11b math_action_1mib_1000_math median={median:?} p95={p95:?} max={max:?}");
        assert!(p95 < Duration::from_millis(50), "p95={p95:?}");
    }

    #[test]
    #[ignore = "Release-only Phase 11-B math button receipt"]
    fn phase11b_performance_math_action_stays_linear_in_shifted_paragraphs() {
        // 23,717 paragraphs that start with a definition, under the Preview's
        // node limit. Parsing so many blocks takes far longer than one large
        // paragraph, so this guards the shape, not the 50 ms budget: the
        // button parses four times, while a per-node search over past
        // shifted paragraphs would cost hundreds of parses.
        let source = workload("[r]: u\nplain text filler plain text filler\n\n");
        let input = snapshot(&source);
        let mut parses: Vec<_> = (0..9)
            .map(|_| {
                let started = Instant::now();
                let arena = Arena::new();
                parse_document(&arena, &source, &markdown_options());
                started.elapsed()
            })
            .collect();
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
