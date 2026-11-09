//! The window's startup once the window and its surface exist, and the one way a startup ends
//! early. What each failure means is `stickymd_app::flow::startup`.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-failure-paths

use super::DesktopShell;
use crate::host::{Host, StartupDetail};
use stickymd_app::config::ViewMode;
use stickymd_app::flow::startup::{StartupDegraded, StartupStop};
use stickymd_app::flow::window::state::{WindowIntent, effective_topmost};

impl<H: Host> DesktopShell<H> {
    /// Ends a startup that cannot go on. The event loop still delivers what it buffered after
    /// `exit`, so the failure is recorded first and every callback returns at once from then on:
    /// nothing typed behind the notice reaches the note. The tray goes before the notice, so that
    /// no icon offers commands while the notice waits for the user.
    pub(super) fn abort_startup(&mut self, stop: &StartupStop, exit: impl FnOnce()) {
        self.startup_failed = true;
        let message = stop.to_string();
        eprintln!("{message}");
        self.host.destroy();
        self.host.show_error("StickyMD", &message);
        exit();
    }

    /// Goes on without something the note can do without. The status line is one line, and each
    /// such failure gets its place on it. The recovery prompt lays out its actions by position,
    /// so it takes nothing more and the reason is told in a dialog instead of being lost.
    fn startup_degraded(&mut self, degraded: StartupDegraded) {
        let message = degraded.to_string();
        let status_line_taken = self.app.recovery().is_pending();
        if degraded.needs_dialog() || status_line_taken {
            self.host.show_error("StickyMD", &message);
        }
        if status_line_taken {
            return;
        }
        let line = message.replace("\n\n", " ");
        match &mut self.diagnostic {
            Some(earlier) => {
                earlier.push('；');
                earlier.push_str(&line);
            }
            None => self.diagnostic = Some(line),
        }
    }

    /// `reveal` and `focus` are the two steps only a real window can take.
    pub(super) fn initialize_window_shell(
        &mut self,
        reveal: impl FnOnce(),
        focus: impl FnOnce(),
    ) -> Result<(), StartupStop> {
        self.prepare_window_shell()?;
        self.host.detail(StartupDetail::WindowShowBegin);
        reveal();
        self.host.detail(StartupDetail::WindowShowEnd);
        self.settle_shown_window()?;
        focus();
        self.host.milestone("focus_ready");
        self.refresh_window_guards();
        self.host.milestone("guards_ready");
        self.host.milestone("shell_ready");
        Ok(())
    }

    /// Everything before the window is shown.
    fn prepare_window_shell(&mut self) -> Result<(), StartupStop> {
        let monitors = self
            .host
            .snapshot()
            .map(|snapshot| snapshot.displays)
            .unwrap_or_default();
        let Some(monitor) = self.app.startup_display(&monitors) else {
            return Err(StartupStop::NoDisplay);
        };
        self.host.milestone("monitor_ready");
        let tray_available = match self.host.create(self.tray_view, self.sink.clone()) {
            Ok(()) => true,
            Err(error) => {
                self.startup_degraded(StartupDegraded::Tray(error));
                false
            }
        };
        if tray_available {
            self.host
                .reassert_identity()
                .map_err(|cause| StartupStop::ToolIdentity {
                    shown: false,
                    cause,
                })?;
        } else {
            // A tool-window-only lifecycle without a tray recovery path is
            // forbidden. Restore ordinary taskbar reachability before show.
            self.host.restore_ordinary_entry();
        }
        let frame = self.app.start_window_flow(monitor, tray_available);
        let _ = self.host.apply_frame(frame);
        self.host.milestone("tray_ready");
        self.host.detail(StartupDetail::SplitModeBegin);
        self.dispatch_window_intent(WindowIntent::SplitModeChanged {
            split: self.app.config().current().view_mode == ViewMode::Split,
        });
        self.host.detail(StartupDetail::SplitModeEnd);
        Ok(())
    }

    /// Everything after the window is shown that can still fail.
    fn settle_shown_window(&mut self) -> Result<(), StartupStop> {
        self.host.detail(StartupDetail::ToolIdentityBegin);
        self.host
            .reassert_identity()
            .map_err(|cause| StartupStop::ToolIdentity { shown: true, cause })?;
        self.host.detail(StartupDetail::ToolIdentityEnd);
        self.host.milestone("window_visible");
        // Winit may refresh native extended styles while making the window
        // visible. Apply layered alpha and z-order after that transition so
        // the configured projection is the final native fact.
        if let Err(error) = self.host.set_opacity(self.app.config().current().opacity) {
            self.startup_degraded(StartupDegraded::Opacity(error));
        }
        self.host.milestone("opacity_ready");
        self.apply_startup_topmost();
        self.host.milestone("topmost_ready");
        Ok(())
    }

    /// Applies the configured value with no receipt: a read-back of any kind is not a failure.
    pub(super) fn apply_startup_topmost(&mut self) {
        let effective = effective_topmost(
            self.app.config().current().always_on_top,
            self.app
                .window_flow()
                .as_ref()
                .is_some_and(|flow| flow.state().temporary_sensor_topmost()),
        );
        if let Err(error) = self.host.set_topmost(effective) {
            self.app.startup_topmost_failed();
            self.startup_degraded(StartupDegraded::Topmost(error));
        }
    }
}
