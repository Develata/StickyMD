//! The shared application state of the running note. The shell keeps only view and native state
//! and reaches the document, its coordinators and their workers through here.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

mod access;
mod assets;
mod clock;
#[cfg(test)]
mod decision_tests;
mod document;
mod effects;
mod events;
mod io;
mod note;
mod preferences;
mod preview;
mod search;
#[cfg(test)]
mod tests;
mod turns;
mod view;
mod window;

pub use clock::{InstantClock, MonotonicClock};
pub use document::ProjectionChange;
pub use effects::{Frame, ShellEffect, ShellEffects};
pub use preferences::ZoomInput;
pub use search::SearchOpenStep;
pub use turns::SessionTurn;
pub use view::PreparedViewChange;

use stickymd_core::{DocumentState, PreserveExpectation, RecoveryCandidate};

use crate::config::{ConfigCoordinator, RuntimeConfig, ViewMode};
use crate::execution::{PersistenceWorker, PreviewWorker};
use crate::flow::window::reducer::WindowShellCoordinator;
use crate::flow::window::state::{VisibilityState, WindowGuardSnapshot};
use crate::flow::{
    AssetFlow, ClipboardPort, EditorCoordinator, PersistenceCoordinator, PreviewCoordinator,
    PreviewVisibility, RecoveryCoordinator,
};
use crate::interaction::rules::{GuardFacts, guard_snapshot, mutation_allowed};
use crate::interaction::{EditorSession, ParkedSearch, SearchSession};

/// The clipboard a host supplies.
pub type SessionClipboard = Box<dyn ClipboardPort>;

/// What only the shell knows that a window guard reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct ViewFacts {
    pub move_resize_active: bool,
    pub scrollbar_dragging: bool,
    pub opacity_popup_open: bool,
}

/// What a session starts from.
pub struct SessionParts {
    pub document: DocumentState,
    pub clipboard: SessionClipboard,
    pub config: RuntimeConfig,
    pub config_persistence_allowed: bool,
    pub recovery: Option<RecoveryCandidate>,
    /// The unusable canonical observation a restore must preserve first (plan 05 recovery 5).
    pub recovery_canonical_to_preserve: Option<PreserveExpectation>,
    pub worker: PersistenceWorker,
    pub clock: Box<dyn MonotonicClock>,
}

/// The session's state. The shell edits `editor` and `search` (interaction state) directly; the
/// rest changes only through the session's methods and is read through its accessors
/// (`access.rs`). Those fields are public only to test builds (`cfg(test)` here, feature `testing`
/// for the crates above), which set up and inspect state the product never reaches directly.
/// The order is the drop order: the Preview worker stops before the persistence worker.
pub struct AppSession {
    #[cfg(any(test, feature = "testing"))]
    pub preview_worker: Option<PreviewWorker>,
    #[cfg(not(any(test, feature = "testing")))]
    preview_worker: Option<PreviewWorker>,
    #[cfg(any(test, feature = "testing"))]
    pub preview_flow: PreviewCoordinator,
    #[cfg(not(any(test, feature = "testing")))]
    preview_flow: PreviewCoordinator,
    #[cfg(any(test, feature = "testing"))]
    pub coordinator: EditorCoordinator<SessionClipboard>,
    #[cfg(not(any(test, feature = "testing")))]
    coordinator: EditorCoordinator<SessionClipboard>,
    #[cfg(any(test, feature = "testing"))]
    pub persistence: PersistenceCoordinator,
    #[cfg(not(any(test, feature = "testing")))]
    persistence: PersistenceCoordinator,
    #[cfg(any(test, feature = "testing"))]
    pub config: ConfigCoordinator,
    #[cfg(not(any(test, feature = "testing")))]
    config: ConfigCoordinator,
    #[cfg(any(test, feature = "testing"))]
    pub config_persistence_allowed: bool,
    #[cfg(not(any(test, feature = "testing")))]
    config_persistence_allowed: bool,
    #[cfg(any(test, feature = "testing"))]
    pub recovery: RecoveryCoordinator,
    #[cfg(not(any(test, feature = "testing")))]
    recovery: RecoveryCoordinator,
    #[cfg(any(test, feature = "testing"))]
    pub worker: PersistenceWorker,
    #[cfg(not(any(test, feature = "testing")))]
    worker: PersistenceWorker,
    #[cfg(any(test, feature = "testing"))]
    pub assets: AssetFlow,
    #[cfg(not(any(test, feature = "testing")))]
    assets: AssetFlow,
    #[cfg(any(test, feature = "testing"))]
    pub export_in_flight: bool,
    #[cfg(not(any(test, feature = "testing")))]
    export_in_flight: bool,
    pub editor: EditorSession,
    pub search: SearchSession,
    /// The open Search while the view has no Source pane (`search.rs::park_search`).
    parked_search: Option<ParkedSearch>,
    #[cfg(any(test, feature = "testing"))]
    pub window_flow: Option<WindowShellCoordinator>,
    #[cfg(not(any(test, feature = "testing")))]
    window_flow: Option<WindowShellCoordinator>,
    #[cfg(any(test, feature = "testing"))]
    pub zoom_config_deadline: Option<u64>,
    #[cfg(not(any(test, feature = "testing")))]
    zoom_config_deadline: Option<u64>,
    clock: Box<dyn MonotonicClock>,
}

impl AppSession {
    pub fn new(parts: SessionParts) -> Self {
        let mut session = Self {
            preview_worker: None,
            preview_flow: PreviewCoordinator::default(),
            coordinator: EditorCoordinator::new(parts.document, parts.clipboard),
            persistence: PersistenceCoordinator::default(),
            config: ConfigCoordinator::loaded(parts.config),
            config_persistence_allowed: parts.config_persistence_allowed,
            recovery: RecoveryCoordinator::new(
                parts.recovery,
                parts.recovery_canonical_to_preserve,
            ),
            worker: parts.worker,
            assets: AssetFlow::default(),
            export_in_flight: false,
            editor: EditorSession::default(),
            search: SearchSession::default(),
            parked_search: None,
            window_flow: None,
            zoom_config_deadline: None,
            clock: parts.clock,
        };
        if session.recovery.is_pending() {
            session.persistence.set_recovery_pending(true);
        }
        session
    }

    pub fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }

    /// Whether input may change the note (`rules::mutation_allowed` says what to pass).
    pub fn mutation_allowed(&self, preview_focused: bool, shell_input_enabled: bool) -> bool {
        mutation_allowed(
            self.recovery.is_pending(),
            preview_focused,
            shell_input_enabled,
            self.window_accepts_mutation(),
            self.assets.reconcile_pending,
        )
    }

    /// Whether the window flow takes edits now; without a flow there is no window to edit in.
    pub fn window_accepts_mutation(&self) -> bool {
        self.window_flow
            .as_ref()
            .is_some_and(|flow| flow.state().accepts_editor_mutation())
    }

    pub fn hidden_to_tray(&self) -> bool {
        self.window_flow.as_ref().is_some_and(|flow| {
            matches!(
                flow.state().visibility(),
                VisibilityState::HiddenToTray { .. }
            )
        })
    }

    /// A waiting recovery choice owns the note file: no watcher until it is resolved.
    pub fn watcher_allowed(&self) -> bool {
        !self.recovery.is_pending()
    }

    pub fn preview_visibility(&self) -> PreviewVisibility {
        match self.config.current().view_mode {
            ViewMode::Source => PreviewVisibility::Hidden,
            ViewMode::Split => PreviewVisibility::Split,
            ViewMode::Preview => PreviewVisibility::Preview,
        }
    }

    /// The first time anything in the session wants to run again.
    pub fn next_deadline(&self) -> Option<u64> {
        [
            self.persistence.autosave_deadline(),
            self.persistence.external_deadline(),
            self.preview_flow.deadline(),
            self.window_flow
                .as_ref()
                .and_then(|flow| flow.state().next_deadline_ms()),
            self.zoom_config_deadline,
        ]
        .into_iter()
        .flatten()
        .min()
    }

    pub fn guards(&self, view: ViewFacts) -> WindowGuardSnapshot {
        guard_snapshot(GuardFacts {
            window_focused: self.editor.focused,
            editor_composing: self.editor.is_composing(),
            search_composing: self.search.is_composing(),
            move_resize_active: view.move_resize_active,
            scrollbar_dragging: view.scrollbar_dragging,
            opacity_popup_open: view.opacity_popup_open,
            search_open: self.search.open,
            export_in_flight: self.export_in_flight,
            recovery_pending: self.recovery.is_pending(),
            conflict: self.persistence.conflict().is_some(),
            paste_pending: self.assets.paste_pending,
            asset_sync_in_flight: self.assets.sync_in_flight,
            dirty: self.coordinator.view().dirty,
            durability_required: self.persistence.durability_required(),
            note_save_in_flight: self.persistence.note_save_in_flight(),
        })
    }
}
