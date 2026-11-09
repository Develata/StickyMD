//! The session's own steps that must wait in the buffer. A step decides with what the shell has
//! already applied in front of it: a guard reads the drag a Preview change just ended, a config
//! flush reads the opacity an input-off effect just committed. So a list of decisions is not run
//! at once; each decision takes its turn among the effects, comes back through
//! `AppSession::resume` with the shell's facts as they are then, and whatever it asks for runs
//! before the next one.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

use crate::flow::window::geometry::WindowPlacement;
use crate::flow::window::state::{DockEdge, QuitBarrier, WindowIntent, WindowSaveReason};
use crate::flow::{AssetStep, NoteEffect, SaveTrigger};
use crate::ports::AssetReconcileMode;
use stickymd_core::{AssetEffect, Generation, Selection};

use super::{AppSession, ShellEffect, ShellEffects, ViewFacts};

/// A session step in the buffer. The shell hands it back and never looks inside.
#[derive(Debug)]
pub struct SessionTurn(Turn);

#[derive(Debug)]
pub(super) enum Turn {
    Note(NoteEffect),
    Asset(AssetStep),
    /// The document's change reaches the editor session and Search.
    DocumentAccepted {
        generation: Generation,
        selection: Selection,
    },
    AssetEffects {
        generation: Generation,
        effects: Vec<AssetEffect>,
    },
    /// A reload's references converge on disk.
    RuntimeAssetConvergence,
    QuitBarrier {
        barrier: QuitBarrier,
        succeeded: bool,
    },
    WindowSave(WindowSaveReason),
    SafeAssetGc,
    ConfigFlush,
    /// The shell has ended what it owns in the focus loss.
    FocusLoss,
    PreviewTick {
        now_ms: u64,
    },
    WindowTick {
        now_ms: u64,
    },
    CommitPlacement {
        placement: WindowPlacement,
        dock_edge: Option<DockEdge>,
    },
    /// The window took the user's always-on-top value (plan 04 window-action-receipts).
    CommitAlwaysOnTop(bool),
    /// Replace Current's change has reached Search; the next match starts at `end`.
    MatchReplaced {
        end: usize,
    },
}

impl ShellEffects {
    pub(super) fn turn(&mut self, turn: Turn) {
        self.push(ShellEffect::Resume(SessionTurn(turn)));
    }
}

impl AppSession {
    /// Takes a waiting step, with the shell's facts as of now.
    pub fn resume(&mut self, turn: SessionTurn, view: ViewFacts, out: &mut ShellEffects) {
        match turn.0 {
            Turn::Note(effect) => self.apply_note_effect(effect, view, out),
            Turn::Asset(step) => self.apply_asset_step(step, view, out),
            Turn::DocumentAccepted {
                generation,
                selection,
            } => {
                self.editor.accept_document_selection(selection);
                self.search
                    .refresh(self.coordinator.view().text, generation);
            }
            Turn::AssetEffects {
                generation,
                effects,
            } => self.submit_asset_sync(generation, effects, None, false, out),
            Turn::RuntimeAssetConvergence => self.submit_asset_sync(
                self.coordinator.view().generation,
                Vec::new(),
                Some(AssetReconcileMode::Runtime),
                false,
                out,
            ),
            Turn::QuitBarrier { barrier, succeeded } => self.dispatch_window_intent(
                WindowIntent::QuitBarrierCompleted {
                    barrier,
                    succeeded,
                    guards: self.guards(view),
                },
                out,
            ),
            Turn::WindowSave(reason) => self.request_save(
                match reason {
                    WindowSaveReason::HideToTray => SaveTrigger::HideToTray,
                    WindowSaveReason::Shutdown => SaveTrigger::Shutdown,
                },
                out,
            ),
            Turn::SafeAssetGc => self.submit_asset_sync(
                self.coordinator.view().generation,
                Vec::new(),
                Some(AssetReconcileMode::SafeBoundary),
                true,
                out,
            ),
            Turn::ConfigFlush => self.flush_config_for_quit(view, out),
            Turn::FocusLoss => self.end_focus(out),
            Turn::PreviewTick { now_ms } => self.tick_preview(now_ms, out),
            Turn::WindowTick { now_ms } => self.tick_window(now_ms, out),
            Turn::CommitPlacement {
                placement,
                dock_edge,
            } => self.commit_placement(&placement, dock_edge, out),
            Turn::CommitAlwaysOnTop(on) => self.commit_user_topmost(on, out),
            Turn::MatchReplaced { end } => self.after_match_replaced(end, out),
        }
    }
}
