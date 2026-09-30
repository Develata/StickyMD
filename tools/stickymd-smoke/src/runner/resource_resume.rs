//! Per-command diagnostic cache ownership and delegation of native progress.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use crate::{
    evidence::EvidenceResult,
    qualification::resource_diagnostics::Store,
    resource_plan::{diagnostic::Unit, progress::Observer},
};
use std::path::Path;

pub(super) struct Resume {
    enabled: bool,
    store: Option<Store>,
}
impl Resume {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            store: None,
        }
    }
    pub(super) fn observe<'a>(
        &'a mut self,
        root: &'a Path,
        inner: &'a mut dyn Observer,
    ) -> Resuming<'a> {
        Resuming {
            resume: self,
            root,
            inner,
        }
    }
}

pub(super) struct Resuming<'a> {
    resume: &'a mut Resume,
    root: &'a Path,
    inner: &'a mut dyn Observer,
}

impl Observer for Resuming<'_> {
    fn verify(&mut self) -> Result<(), String> {
        if !self.resume.enabled {
            return Ok(());
        }
        if let Some(store) = &self.resume.store {
            return store.verify(self.root);
        }
        match Store::open(self.root) {
            Ok(store) => {
                self.resume.store = Some(store);
            }
            Err(error) => {
                eprintln!("RESOURCE_RESUME status=DISABLED reason={error}");
                self.resume.enabled = false;
            }
        }
        Ok(())
    }
    fn load(&mut self, case: Unit) -> Result<Option<EvidenceResult>, String> {
        let result = self
            .resume
            .store
            .as_ref()
            .map_or(Ok(None), |store| store.load(self.root, case))?;
        if result.is_some() {
            self.inner.waited(case.minimum_wait_seconds());
            self.inner.stage(
                case.key(),
                crate::resource_plan::REPETITIONS,
                "diagnostic-reused",
                0,
            )?;
        }
        Ok(result)
    }
    fn save(&mut self, case: Unit, result: &EvidenceResult, elapsed: f64) -> Result<(), String> {
        self.resume
            .store
            .as_ref()
            .map_or(Ok(()), |store| store.save(self.root, case, result, elapsed))
    }
    fn stage(&mut self, cohort: &str, run: usize, stage: &str, seconds: u64) -> Result<(), String> {
        self.inner.stage(cohort, run, stage, seconds)
    }
    fn waited(&mut self, seconds: u64) {
        self.inner.waited(seconds);
    }
    fn checkpoint(&mut self, result: EvidenceResult) -> Result<(), String> {
        self.inner.checkpoint(result)
    }
}
