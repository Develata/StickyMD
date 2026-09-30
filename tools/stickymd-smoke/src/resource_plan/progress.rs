//! Resource execution events; their persistence belongs to the runner.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use crate::evidence::EvidenceResult;

// These events are emitted by the native Windows resource executor.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) trait Observer {
    fn order_cases(&self, _cases: &mut [super::ResourceCase]) {}
    fn verify(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn load(&mut self, _unit: super::diagnostic::Unit) -> Result<Option<EvidenceResult>, String> {
        Ok(None)
    }
    fn load_all(
        &mut self,
        _cases: &[super::ResourceCase],
    ) -> Result<Option<Vec<EvidenceResult>>, String> {
        Ok(None)
    }
    fn save(
        &mut self,
        _unit: super::diagnostic::Unit,
        _result: &EvidenceResult,
        _elapsed_seconds: f64,
    ) -> Result<(), String> {
        Ok(())
    }
    fn stage(&mut self, cohort: &str, run: usize, stage: &str, seconds: u64) -> Result<(), String>;
    fn waited(&mut self, seconds: u64);
    fn checkpoint(&mut self, result: EvidenceResult) -> Result<(), String>;
}

#[cfg(windows)]
pub(crate) struct Console {
    pub(crate) remaining: u64,
}

#[cfg(windows)]
impl Observer for Console {
    fn stage(&mut self, cohort: &str, run: usize, stage: &str, seconds: u64) -> Result<(), String> {
        eprintln!(
            "RESOURCE_PROGRESS cohort={cohort} run={run}/{} stage={stage} phase_fixed_seconds={seconds} remaining_fixed_wait_seconds={}",
            super::REPETITIONS,
            self.remaining
        );
        Ok(())
    }
    fn waited(&mut self, seconds: u64) {
        self.remaining = self.remaining.saturating_sub(seconds);
    }
    fn checkpoint(&mut self, _result: EvidenceResult) -> Result<(), String> {
        Ok(())
    }
}
