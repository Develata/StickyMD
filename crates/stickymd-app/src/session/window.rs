//! The window flow of the session: its intents and what their effects mean for the rest of it.
//!
//! plan_ref: docs/plan/09_windows_shell.md#tray-lifecycle

use crate::config::DockEdge as ConfigDockEdge;
use crate::flow::window::geometry::{MonitorGeometry, PhysicalRect, WindowPlacement};
use crate::flow::window::reducer::WindowShellCoordinator;
use crate::flow::window::state::{
    DockEdge, QuitBarrier, ShowReason, StableVisibility, WindowEffect, WindowGuardSnapshot,
    WindowIntent,
};
use crate::interaction::rules::{clamp_placement, window_display};

use super::turns::Turn;
use super::{AppSession, ShellEffect, ShellEffects, ViewFacts};

impl AppSession {
    /// The display the window opens on: the configured one, else the primary, else any.
    pub fn startup_display(&self, displays: &[MonitorGeometry]) -> Option<MonitorGeometry> {
        window_display(displays, &self.config.current().window.monitor_id)
    }

    /// Starts the window flow at the configured size, place and dock edge on `display`, and
    /// returns the frame the window takes first. Without a tray the flow never hides the window.
    pub fn start_window_flow(
        &mut self,
        display: MonitorGeometry,
        tray_available: bool,
    ) -> PhysicalRect {
        let configured = &self.config.current().window;
        let placement = WindowPlacement::new(
            configured.width_dip as f64,
            configured.height_dip as f64,
            Some(display.identity.clone()),
            configured.dock_offset_ratio as f64,
            configured.floating_x_ratio as f64,
            configured.floating_y_ratio as f64,
        );
        let visibility = match configured.dock_edge {
            ConfigDockEdge::None => StableVisibility::Floating,
            ConfigDockEdge::Left => StableVisibility::DockedExpanded(DockEdge::Left),
            ConfigDockEdge::Right => StableVisibility::DockedExpanded(DockEdge::Right),
            ConfigDockEdge::Top => StableVisibility::DockedExpanded(DockEdge::Top),
        };
        let always_on_top = self.config.current().always_on_top;
        let flow = WindowShellCoordinator::new(placement, visibility, display, tray_available)
            .with_user_topmost(always_on_top);
        let frame = flow.state().frame();
        self.window_flow = Some(flow);
        frame
    }

    pub fn dispatch_window_intent(&mut self, intent: WindowIntent, out: &mut ShellEffects) {
        let Some(flow) = &mut self.window_flow else {
            return;
        };
        for effect in flow.dispatch(intent) {
            Self::route_window_effect(effect, out);
        }
    }

    /// The tray's show/hide command. A window hidden to the tray is shown; any other is toggled
    /// by the flow, which also takes `guards` as the guards it holds from now on.
    pub fn toggle_visibility_from_tray(
        &mut self,
        now_ms: u64,
        guards: WindowGuardSnapshot,
        out: &mut ShellEffects,
    ) {
        let intent = if self.hidden_to_tray() {
            WindowIntent::ShowRequested {
                reason: ShowReason::Tray,
                now_ms,
            }
        } else {
            WindowIntent::TrayToggleRequested { now_ms, guards }
        };
        self.dispatch_window_intent(intent, out);
    }

    /// Hands an effect to whoever carries it out: the note, the assets and the configuration are
    /// the session's, and they wait their turn among the shell's.
    pub fn route_window_effect(effect: WindowEffect, out: &mut ShellEffects) {
        match effect {
            WindowEffect::ExitProcess => out.push(ShellEffect::Exit),
            WindowEffect::RequestNoteSave(reason) => out.turn(Turn::WindowSave(reason)),
            WindowEffect::RequestSafeAssetGc => out.turn(Turn::SafeAssetGc),
            WindowEffect::RequestConfigFlush => out.turn(Turn::ConfigFlush),
            WindowEffect::CommitPlacement {
                placement,
                dock_edge,
            } => out.turn(Turn::CommitPlacement {
                placement,
                dock_edge,
            }),
            WindowEffect::CommitAlwaysOnTop(on) => out.turn(Turn::CommitAlwaysOnTop(on)),
            effect => out.push(ShellEffect::Window(effect)),
        }
    }

    pub(super) fn flush_config_for_quit(&mut self, view: ViewFacts, out: &mut ShellEffects) {
        if !self.config_persistence_allowed {
            self.dispatch_window_intent(
                WindowIntent::QuitBarrierCompleted {
                    barrier: QuitBarrier::Config,
                    succeeded: false,
                    guards: self.guards(view),
                },
                out,
            );
            return;
        }
        self.submit_config_if_needed();
        if !self.config.is_dirty() && !self.config.is_saving() {
            self.dispatch_window_intent(
                WindowIntent::QuitBarrierCompleted {
                    barrier: QuitBarrier::Config,
                    succeeded: true,
                    guards: self.guards(view),
                },
                out,
            );
        }
    }

    pub(super) fn commit_placement(
        &mut self,
        placement: &WindowPlacement,
        dock_edge: Option<DockEdge>,
        out: &mut ShellEffects,
    ) {
        let result = self.config.update(|config| {
            (config.window.width_dip, config.window.height_dip) =
                clamp_placement(placement.width_dip, placement.height_dip);
            config.window.monitor_id = placement
                .monitor_identity
                .as_ref()
                .map_or_else(String::new, |identity| identity.as_str().to_owned());
            config.window.dock_edge = match dock_edge {
                None => ConfigDockEdge::None,
                Some(DockEdge::Left) => ConfigDockEdge::Left,
                Some(DockEdge::Right) => ConfigDockEdge::Right,
                Some(DockEdge::Top) => ConfigDockEdge::Top,
            };
            config.window.dock_offset_ratio = placement.dock_offset_ratio as f32;
            config.window.floating_x_ratio = placement.floating_x_ratio as f32;
            config.window.floating_y_ratio = placement.floating_y_ratio as f32;
        });
        match result {
            Ok(true) => self.submit_config_if_needed(),
            Ok(false) => {}
            Err(error) => out.push(ShellEffect::Diagnostic(error.to_string())),
        }
    }
}
