//! Deterministic autosave scheduling state.
//!
//! plan_ref: docs/plan/05_document_persistence.md#autosave-and-save-queue

use stickymd_core::Generation;

pub const AUTOSAVE_DEBOUNCE_MS: u64 = 650;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveTrigger {
    Debounce,
    Manual,
    FocusLoss,
    Shutdown,
    RecreateMissing,
    KeepLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutosaveAction {
    pub generation: Generation,
    pub trigger: SaveTrigger,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AutosaveScheduler {
    deadline_ms: Option<u64>,
    scheduled_generation: Option<Generation>,
    paused: bool,
}

impl AutosaveScheduler {
    pub fn on_document_changed(&mut self, now_ms: u64, generation: Generation) {
        self.scheduled_generation = Some(generation);
        self.deadline_ms = now_ms.checked_add(AUTOSAVE_DEBOUNCE_MS);
    }

    pub fn request_now(
        &mut self,
        generation: Generation,
        trigger: SaveTrigger,
    ) -> Option<AutosaveAction> {
        if self.paused {
            return None;
        }
        self.deadline_ms = None;
        self.scheduled_generation = None;
        Some(AutosaveAction {
            generation,
            trigger,
        })
    }

    pub fn tick(&mut self, now_ms: u64) -> Option<AutosaveAction> {
        if self.paused || self.deadline_ms.is_none_or(|deadline| now_ms < deadline) {
            return None;
        }
        let generation = self.scheduled_generation.take()?;
        self.deadline_ms = None;
        Some(AutosaveAction {
            generation,
            trigger: SaveTrigger::Debounce,
        })
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        if paused {
            self.deadline_ms = None;
            self.scheduled_generation = None;
        }
    }

    pub fn next_deadline_ms(&self) -> Option<u64> {
        if self.paused { None } else { self.deadline_ms }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generation(value: u64) -> Generation {
        let mut generation = Generation::initial();
        for _ in 0..value {
            generation = generation.checked_next().unwrap();
        }
        generation
    }

    #[test]
    fn edit_debounce_resets_and_fires_only_at_650ms() {
        let mut scheduler = AutosaveScheduler::default();
        scheduler.on_document_changed(0, generation(1));
        assert_eq!(scheduler.tick(649), None);
        scheduler.on_document_changed(400, generation(2));
        assert_eq!(scheduler.tick(1049), None);
        assert_eq!(
            scheduler.tick(1050),
            Some(AutosaveAction {
                generation: generation(2),
                trigger: SaveTrigger::Debounce,
            })
        );
    }

    #[test]
    fn one_thousand_continuous_edits_coalesce_to_one_autosave() {
        let mut scheduler = AutosaveScheduler::default();
        let mut latest = Generation::initial();
        for now_ms in 0..1000 {
            latest = latest.checked_next().unwrap();
            scheduler.on_document_changed(now_ms, latest);
        }
        assert_eq!(scheduler.tick(1648), None);
        assert_eq!(
            scheduler.tick(1649),
            Some(AutosaveAction {
                generation: latest,
                trigger: SaveTrigger::Debounce,
            })
        );
        assert_eq!(scheduler.tick(10_000), None);
    }

    #[test]
    fn immediate_triggers_bypass_and_cancel_debounce() {
        let mut scheduler = AutosaveScheduler::default();
        scheduler.on_document_changed(0, generation(1));
        assert_eq!(
            scheduler.request_now(generation(1), SaveTrigger::Manual),
            Some(AutosaveAction {
                generation: generation(1),
                trigger: SaveTrigger::Manual,
            })
        );
        assert_eq!(scheduler.tick(1000), None);

        for trigger in [SaveTrigger::FocusLoss, SaveTrigger::Shutdown] {
            scheduler.on_document_changed(2000, generation(2));
            assert_eq!(
                scheduler.request_now(generation(2), trigger),
                Some(AutosaveAction {
                    generation: generation(2),
                    trigger,
                })
            );
        }
    }

    #[test]
    fn conflict_pause_discards_normal_autosave() {
        let mut scheduler = AutosaveScheduler::default();
        scheduler.on_document_changed(0, generation(1));
        scheduler.set_paused(true);
        assert_eq!(scheduler.tick(1000), None);
        assert_eq!(
            scheduler.request_now(generation(1), SaveTrigger::FocusLoss),
            None
        );
    }
}
