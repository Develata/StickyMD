//! The shell's side of the event pass: focus, the settlement at its end, the exit it asked for
//! and the host's signals. The event loop's callbacks are `handler.rs`.
//!
//! plan_ref: docs/plan/03_system_architecture.md#interaction-shell

use super::DesktopShell;
use crate::host::{Host, HostSignal};

impl<H: Host> DesktopShell<H> {
    pub(super) fn start_watcher(&mut self) {
        if !self.app.watcher_allowed() {
            return;
        }
        if let Err(error) = self.host.start_watch(&self.sink) {
            self.diagnostic = Some(format!(
                "外部文件自动检测不可用；安全保存仍受指纹校验保护：{error}"
            ));
        }
    }

    pub(super) fn timestamp_ms(&self) -> u64 {
        self.app.now_ms()
    }

    pub(super) fn apply_focus_change(&mut self, focused: bool) {
        self.with_session(|app, _, out| app.on_focus_changed(focused, out));
        self.sync_ime_allowed();
        self.refresh_window_guards();
        self.after_presentation_change();
    }

    pub(super) fn end_view_focus(&mut self) {
        self.cancel_scrollbar_drag();
        self.scrollbars.hovered = None;
        if self.controls.opacity_input_focused {
            // The field loses focus with the window, which commits it (plan 09).
            self.commit_opacity_input();
        }
    }

    /// The end of an `ApplicationHandler` callback: an exit the effects of this callback asked
    /// for happens here, after the tray went with the effect.
    pub(super) fn finish_callback(&mut self, exit: impl FnOnce()) {
        if std::mem::take(&mut self.exit_pending) {
            exit();
        }
    }

    /// Complete the pass before deciding how long the native event loop can sleep.
    pub(super) fn settle_event_pass(&mut self, now_ms: u64) {
        self.with_session(|app, _, out| app.settle(now_ms, out));
        self.finish_event_pass();
    }

    pub(super) fn handle_host_signal(&mut self, signal: HostSignal) {
        match signal {
            HostSignal::NoteDirectoryChanged => {
                self.host.acknowledge_watch();
                self.app.note_directory_changed();
            }
            HostSignal::WatchFailed(error) => {
                self.host.stop_watch();
                self.diagnostic = Some(format!(
                    "外部文件自动检测已停止；安全保存仍受指纹校验保护：{error}"
                ));
                self.request_redraw();
            }
            HostSignal::ShowRequested => {
                self.dispatch_window_intent(
                    stickymd_app::flow::window::state::WindowIntent::ShowRequested {
                        reason: stickymd_app::flow::window::state::ShowReason::SecondInstance,
                        now_ms: self.timestamp_ms(),
                    },
                );
            }
            HostSignal::Tray(command) => self.handle_tray_command(command),
            HostSignal::DisplayTopologyChanged => self.recover_display_topology(),
        }
    }
}

#[cfg(test)]
mod runtime_tests;
