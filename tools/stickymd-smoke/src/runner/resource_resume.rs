//! Per-command diagnostic cache ownership and delegation of native progress.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use crate::{
    evidence::EvidenceResult,
    qualification::resource_diagnostics::{Store, priority},
    resource_plan::{diagnostic::Unit, progress::Observer},
};
use std::path::Path;

#[cfg(test)]
#[path = "resource_resume_tests.rs"]
mod tests;

pub(super) struct Resume {
    enabled: bool,
    store: Option<Store>,
    planned: bool,
    batch_candidates: Vec<Unit>,
    record_failures: bool,
    priority: priority::Selection,
}
impl Resume {
    pub(super) fn new(enabled: bool, priority: priority::Selection) -> Self {
        Self {
            enabled,
            store: None,
            planned: false,
            batch_candidates: Vec::new(),
            record_failures: enabled,
            priority,
        }
    }
    pub(super) fn plan(
        &mut self,
        root: &Path,
        groups: &[crate::cli::ResourceModule],
    ) -> Result<(), String> {
        if self.planned || !self.enabled {
            return Ok(());
        }
        self.planned = true;
        use crate::qualification::resource_diagnostics::plan;
        let mut units = plan::units(
            groups,
            std::env::var("STICKYMD_SMOKE_RESOURCE_CASE")
                .ok()
                .as_deref(),
        )?;
        plan::prioritize(&mut units, self.priority.first);
        let (store, plan) = plan::prepare(root, &units, self.priority.first)?;
        plan.log();
        self.batch_candidates = plan.cached_units();
        self.enabled = store.is_some();
        self.store = store;
        Ok(())
    }
    pub(super) fn observe<'a>(
        &'a mut self,
        root: &'a Path,
        group: crate::cli::ResourceModule,
        inner: &'a mut dyn Observer,
    ) -> Resuming<'a> {
        Resuming {
            resume: self,
            root,
            inner,
            group,
            active: matches!(
                group,
                crate::cli::ResourceModule::Window | crate::cli::ResourceModule::Zoom
            )
            .then_some(Unit::Group(group)),
        }
    }
}

pub(super) struct Resuming<'a> {
    resume: &'a mut Resume,
    root: &'a Path,
    inner: &'a mut dyn Observer,
    group: crate::cli::ResourceModule,
    active: Option<Unit>,
}

impl Resuming<'_> {
    pub(super) fn failed(&mut self, original: String) -> String {
        if self.resume.record_failures
            && let Some(unit) = self.active
            && let Err(error) = priority::failed(self.root, unit)
        {
            return format!("{original}; failure hint write also failed: {error}");
        }
        original
    }
}

impl Observer for Resuming<'_> {
    fn order_cases(&self, cases: &mut [crate::resource_plan::ResourceCase]) {
        cases.sort_by_key(|case| Some(Unit::Case(*case)) != self.resume.priority.first);
    }
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
        if Some(case) == self.resume.priority.first {
            self.verify()?;
            eprintln!(
                "RESOURCE_RESUME unit={} status=FORCE_RUN reason=LAST_FAILURE",
                case.key()
            );
            return Ok(None);
        }
        if self.resume.enabled && self.resume.store.is_none() {
            self.verify()?;
        }
        // Store::load checks fresh identity on both sides of the cache read.
        // Same-command aliases bypass this path and explicitly call verify instead.
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
            .map_or(Ok(()), |store| store.save(self.root, case, result, elapsed))?;
        if self.resume.record_failures {
            self.resume.priority.complete(self.root, case, result)?;
        }
        Ok(())
    }
    fn load_all(
        &mut self,
        cases: &[crate::resource_plan::ResourceCase],
    ) -> Result<Option<Vec<EvidenceResult>>, String> {
        if cases
            .iter()
            .any(|case| Some(Unit::Case(*case)) == self.resume.priority.first)
            || !cases
                .iter()
                .all(|c| self.resume.batch_candidates.contains(&Unit::Case(*c)))
        {
            return Ok(None);
        }
        let Some(store) = &self.resume.store else {
            return Ok(None);
        };
        let results = store.load_batch(self.root, cases)?;
        if results.is_some() {
            for case in cases {
                self.inner.waited(case.minimum_wait_seconds());
            }
            self.inner
                .stage("diagnostic-cache-batch", 0, "diagnostic-batch-reused", 0)?;
        }
        Ok(results)
    }
    fn stage(&mut self, cohort: &str, run: usize, stage: &str, seconds: u64) -> Result<(), String> {
        if stage == "case-start"
            && !matches!(
                self.group,
                crate::cli::ResourceModule::Window | crate::cli::ResourceModule::Zoom
            )
        {
            self.active = Unit::from_key(cohort).filter(|unit| unit.group() == Some(self.group));
        }
        self.inner.stage(cohort, run, stage, seconds)?;
        if matches!(stage, "case-finished" | "diagnostic-reused" | "shared")
            && matches!(self.active, Some(Unit::Case(_)))
        {
            self.active = None;
        }
        Ok(())
    }
    fn waited(&mut self, seconds: u64) {
        self.inner.waited(seconds);
    }
    fn checkpoint(&mut self, result: EvidenceResult) -> Result<(), String> {
        self.inner.checkpoint(result)
    }
}
