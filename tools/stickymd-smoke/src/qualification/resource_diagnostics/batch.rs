//! All-or-nothing reads bracket a fully cached set; partial hits grant no reuse authority.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::{Identity, Store, identity};
use crate::{
    evidence::EvidenceResult,
    resource_plan::{GROUPS, ResourceCase},
};
use std::path::Path;

impl Store {
    pub(crate) fn load_batch(
        &self,
        root: &Path,
        cases: &[ResourceCase],
    ) -> Result<Option<Vec<EvidenceResult>>, String> {
        self.load_batch_using(root, cases, || identity::capture(root, &self.host))
    }

    pub(super) fn load_batch_using(
        &self,
        root: &Path,
        cases: &[ResourceCase],
        mut fresh: impl FnMut() -> Result<Identity, String>,
    ) -> Result<Option<Vec<EvidenceResult>>, String> {
        if cases.len() < 2
            || !GROUPS
                .iter()
                .any(|g| cases.iter().all(|c| g.cases().contains(c)))
            || cases
                .iter()
                .enumerate()
                .any(|(i, c)| cases[..i].contains(c))
        {
            return Err("diagnostic batch requires distinct registered cases in one group".into());
        }
        self.matches(fresh()?)?;
        let results = cases
            .iter()
            .map(|case| self.inspect(root, (*case).into()).map(|v| v.result))
            .collect::<Result<Vec<_>, _>>()?;
        self.matches(fresh()?)?;
        let complete: Option<Vec<_>> = results.into_iter().collect();
        eprintln!(
            "RESOURCE_RESUME_BATCH status={} cases={}",
            if complete.is_some() {
                "DIAGNOSTIC_REUSED"
            } else {
                "MISS"
            },
            cases.len()
        );
        Ok(complete)
    }
}
