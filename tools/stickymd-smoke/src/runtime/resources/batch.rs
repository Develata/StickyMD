//! Transient batch results never cross a fresh measurement or a resource-group boundary.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::{Cache, EvidenceMeasurement, Observer, Output, ResourceCase};
use crate::evidence::EvidenceResult;
use std::{collections::VecDeque, time::Instant};

pub(super) fn prepare(
    cache: &Cache,
    cases: &[ResourceCase],
    output: &mut Output,
    observer: &mut dyn Observer,
) -> Result<VecDeque<EvidenceResult>, String> {
    let pending: Vec<_> = cases
        .iter()
        .copied()
        .filter(|c| !cache.cohorts.contains(*c))
        .collect();
    if pending.len() < 2 {
        return Ok(VecDeque::new());
    }
    let started = Instant::now();
    let Some(results) = observer.load_all(&pending)? else {
        return Ok(VecDeque::new());
    };
    if results.len() != pending.len() || results.iter().zip(&pending).any(|(r, c)| r.id != c.label)
    {
        return Err("diagnostic batch did not return every pending case in order".into());
    }
    output.measurements.push(EvidenceMeasurement {
        name: "cache_batch.execution_seconds".into(),
        unit: "seconds".into(),
        value: started.elapsed().as_secs_f64(),
    });
    output.measurements.push(EvidenceMeasurement {
        name: "cache_batch.case_count".into(),
        unit: "count".into(),
        value: results.len() as f64,
    });
    Ok(results.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Reader {
        expected: Vec<ResourceCase>,
        complete: bool,
    }
    impl Observer for Reader {
        fn load_all(
            &mut self,
            cases: &[ResourceCase],
        ) -> Result<Option<Vec<EvidenceResult>>, String> {
            assert_eq!(cases, self.expected);
            Ok(self.complete.then(|| {
                cases
                    .iter()
                    .map(|c| {
                        let mut result = crate::resource_plan::tests::valid_resource_result(
                            crate::cli::ResourceModule::Math,
                        );
                        result.id = c.label.into();
                        result
                    })
                    .collect()
            }))
        }
        fn stage(&mut self, _: &str, _: usize, _: &str, _: u64) -> Result<(), String> {
            Ok(())
        }
        fn waited(&mut self, _: u64) {}
        fn checkpoint(&mut self, _: EvidenceResult) -> Result<(), String> {
            Ok(())
        }
    }
    #[test]
    fn batch_only_requests_pending_cases_and_does_not_consume_partial_hits() {
        let cases = crate::cli::ResourceModule::Math.cases();
        let mut cache = Cache::default();
        for &case in crate::cli::ResourceModule::SourcePreview.cases() {
            cache
                .cohorts
                .measure(case, || Ok(Output::default()))
                .unwrap();
        }
        let mut reader = Reader {
            expected: vec![cases[1], cases[2], cases[5]],
            complete: false,
        };
        let mut output = Output::default();
        assert!(
            prepare(&cache, cases, &mut output, &mut reader)
                .unwrap()
                .is_empty()
        );
        assert!(output.measurements.is_empty());
        reader.complete = true;
        let values = prepare(&cache, cases, &mut output, &mut reader).unwrap();
        assert_eq!(
            values.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            reader.expected.iter().map(|c| c.label).collect::<Vec<_>>()
        );
        assert_eq!(output.measurements.last().unwrap().value, 3.0);
    }
}
