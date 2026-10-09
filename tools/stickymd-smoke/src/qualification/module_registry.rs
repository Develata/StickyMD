//! The qualification module registry: stable module identities and their receipts.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::path::Path;

use super::path_identity::matches_receipt;
use crate::cli::ResourceModule;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ModuleId {
    Runtime,
    Performance,
    Resource(ResourceModule),
    G3,
    G4,
    G5,
}

pub(super) fn modules() -> impl Iterator<Item = ModuleId> {
    [ModuleId::Runtime, ModuleId::Performance]
        .into_iter()
        .chain(
            crate::resource_plan::GROUPS
                .into_iter()
                .map(ModuleId::Resource),
        )
        .chain([ModuleId::G3, ModuleId::G4, ModuleId::G5])
}

impl ModuleId {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Runtime => "runtime",
            Self::Performance => "performance",
            Self::Resource(group) => group.ledger_id(),
            Self::G3 => "g3",
            Self::G4 => "g4",
            Self::G5 => "g5",
        }
    }

    pub(super) const fn receipt(self) -> &'static str {
        match self {
            Self::Runtime => "dist/evidence/runtime-qualification.json",
            Self::Performance => "dist/evidence/performance-qualification.json",
            Self::Resource(group) => group.receipt(),
            Self::G3 => "dist/evidence/g3-exact-qualification.json",
            Self::G4 => "dist/evidence/g4-exact-qualification.json",
            Self::G5 => "dist/evidence/g5-exact-qualification.json",
        }
    }
}

pub(super) fn module_for_receipt(root: &Path, path: &Path) -> Option<ModuleId> {
    modules().find(|module| matches_receipt(root, path, module.receipt()))
}
