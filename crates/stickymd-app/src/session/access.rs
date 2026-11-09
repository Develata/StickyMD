//! What the shell may reach of the session's state: read-only views of the document, the
//! configuration and the flows; the two public interaction states (Search, the editor) lent
//! together with the note's text; and the clipboard read. Whatever changes the session's own state
//! goes through its methods, so the shell cannot step around the session's transactions.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

use crate::config::ConfigCoordinator;
use crate::flow::window::reducer::WindowShellCoordinator;
use crate::flow::{
    AssetFlow, EditorCoordinator, EditorFlowError, PersistenceCoordinator, RecoveryCoordinator,
};
use crate::interaction::{EditorSession, SearchSession};

use super::{AppSession, SessionClipboard};

impl AppSession {
    pub fn coordinator(&self) -> &EditorCoordinator<SessionClipboard> {
        &self.coordinator
    }

    pub fn config(&self) -> &ConfigCoordinator {
        &self.config
    }

    pub fn persistence(&self) -> &PersistenceCoordinator {
        &self.persistence
    }

    pub fn recovery(&self) -> &RecoveryCoordinator {
        &self.recovery
    }

    pub fn assets(&self) -> &AssetFlow {
        &self.assets
    }

    pub fn window_flow(&self) -> Option<&WindowShellCoordinator> {
        self.window_flow.as_ref()
    }

    /// Search with the note's text, for an edit of Search that rescans against it.
    pub fn search_over_text(&mut self) -> (&mut SearchSession, &str) {
        (&mut self.search, self.coordinator.view().text)
    }

    /// The editor session with the note's text, for a caret move over it.
    pub fn editor_over_text(&mut self) -> (&mut EditorSession, &str) {
        (&mut self.editor, self.coordinator.view().text)
    }

    /// The clipboard's text, for Search's paste: reading the clipboard port needs it mutably.
    pub fn read_clipboard_text(&mut self) -> Result<Option<String>, EditorFlowError> {
        self.coordinator.read_clipboard_text()
    }
}
