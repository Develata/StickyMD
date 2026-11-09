//! Applies typed window-domain effects to the window and the host's window port.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

use winit::error::ExternalError;
use winit::window::{ResizeDirection, Theme};

use super::DesktopShell;
#[cfg(test)]
use super::shell_pins_tests::PressRoute;
use crate::host::Host;
use stickymd_app::flow::WindowPreferenceEffect;
use stickymd_app::flow::window::geometry::PhysicalRect;
use stickymd_app::flow::window::state::{TopmostRequest, WindowEffect, WindowIntent};
use stickymd_app::instruction::{WindowPlatformIntent, WindowPreferenceIntent, WindowResizeEdge};
use stickymd_app::ports::{FailureKind, TrayView, Visibility};
#[cfg(test)]
use stickymd_app::session::AppSession;

impl<H: Host> DesktopShell<H> {
    /// Starts the native gesture; `None` without a window to ask.
    fn start_window_gesture(
        &self,
        intent: WindowPlatformIntent,
    ) -> Option<Result<(), ExternalError>> {
        #[cfg(test)]
        if let Some(started) = self.headless_window_gesture {
            return Some(started.then_some(()).ok_or(ExternalError::Ignored));
        }
        let window = self.window.as_ref()?;
        Some(match intent {
            WindowPlatformIntent::RequestDrag => window.drag_window(),
            WindowPlatformIntent::RequestResize(direction) => {
                window.drag_resize_window(winit_resize_direction(direction))
            }
        })
    }

    pub(super) fn dispatch_window_platform_intent(&mut self, intent: WindowPlatformIntent) -> bool {
        press_probe!(self, PressRoute::Window(intent), true);
        let Some(result) = self.start_window_gesture(intent) else {
            return false;
        };
        match result {
            Ok(()) => {
                self.move_resize_active = true;
                self.dispatch_window_intent(WindowIntent::MoveResizeStarted);
                self.refresh_window_guards();
                true
            }
            Err(error) => {
                self.diagnostic = Some(format!("无法开始窗口拖动或缩放：{error}"));
                self.request_redraw();
                true
            }
        }
    }

    pub(super) fn dispatch_window_preference_intent(&mut self, intent: WindowPreferenceIntent) {
        press_probe!(self, PressRoute::Preference(intent));
        self.with_session(|app, _, out| app.dispatch_window_preference(intent, out));
    }

    /// Shows a preference the configuration took.
    pub(super) fn show_window_preference(&mut self, effect: WindowPreferenceEffect) {
        match effect {
            WindowPreferenceEffect::NoOp | WindowPreferenceEffect::ApplySplitScrollSync(_) => {}
            WindowPreferenceEffect::ApplyTheme(theme) => {
                if let Some(window) = &self.window {
                    window.set_theme(match theme {
                        stickymd_app::config::ThemeMode::Light => Some(Theme::Light),
                        stickymd_app::config::ThemeMode::Dark => Some(Theme::Dark),
                        stickymd_app::config::ThemeMode::System => None,
                    });
                }
                self.source_paint_key = None;
                self.request_preview_relayout();
            }
            WindowPreferenceEffect::ApplyOpacity { opacity, .. } => {
                let opacity = self.controls.preview(opacity);
                if let Err(error) = self.host.set_opacity(opacity) {
                    self.diagnostic = Some(format!("无法预览透明度：{error}"));
                }
            }
            // The window took it before the configuration did: only the tray is left to tell.
            WindowPreferenceEffect::ApplyAlwaysOnTop(topmost) => {
                self.update_tray(TrayView {
                    always_on_top: topmost,
                    ..self.tray_view
                });
            }
            WindowPreferenceEffect::ApplyContentZoom(_) => {
                self.source_paint_key = None;
                self.configure_viewports();
                self.request_preview_relayout();
                self.update_ime_area();
                self.app.defer_zoom_config();
            }
        }
    }

    pub(super) fn refresh_window_guards(&mut self) {
        self.dispatch_window_intent(WindowIntent::GuardsChanged {
            guards: self.window_guards(),
            now_ms: self.timestamp_ms(),
        });
    }

    pub(super) fn dispatch_window_intent(&mut self, intent: WindowIntent) {
        self.with_session(|app, _, out| app.dispatch_window_intent(intent, out));
    }

    /// One effect, on the way the reducer's effects take.
    #[cfg(test)]
    pub(super) fn apply_window_effect(&mut self, effect: WindowEffect) {
        self.with_session(|_, _, out| AppSession::route_window_effect(effect, out));
    }

    pub(super) fn run_window_effect(&mut self, effect: WindowEffect) {
        #[cfg(test)]
        self.applied_window_effects.push(format!("{effect:?}"));
        if self.quit_notice.on_window_effect(&effect) {
            self.update_window_title();
        }
        match effect {
            WindowEffect::ApplyFrame { id, target, last } => self.apply_frame(id, target, last),
            WindowEffect::ObservePlacement { id } => {
                // The tick that asked may also have replaced this placement (an auto-collapse
                // due at the same time), and then there is nothing to judge.
                if self.pending_placement() == Some(id) {
                    self.observe_placement(id);
                }
            }
            WindowEffect::PlacementFailed(kind) => {
                self.diagnostic = Some(format!("窗口未能到达目标位置（{kind:?}）。"));
                self.request_redraw();
            }
            WindowEffect::ApplyTopmost(request) => self.apply_topmost(request),
            WindowEffect::TopmostRejected { kind, .. } => {
                self.diagnostic = Some(format!("无法应用窗口置顶状态（{kind:?}）。"));
                self.request_redraw();
            }
            WindowEffect::HideWindow { id } => {
                self.scrollbars = Default::default();
                self.controls.opacity_popup_open = false;
                self.controls.opacity_dragging = false;
                self.controls.opacity_input_focused = false;
                self.controls.opacity_preview = self.app.config().current().opacity;
                self.controls.opacity_input = self.app.config().current().opacity.to_string();
                // The closed popup no longer holds the window open, whichever way the hide ends.
                self.refresh_window_guards();
                let outcome = self.hide_native();
                self.dispatch_window_intent(WindowIntent::ActionCompleted {
                    id,
                    outcome,
                    now_ms: self.timestamp_ms(),
                });
            }
            WindowEffect::ShowWindow { id } => {
                let outcome = self.show_native();
                self.dispatch_window_intent(WindowIntent::ActionCompleted {
                    id,
                    outcome,
                    now_ms: self.timestamp_ms(),
                });
            }
            WindowEffect::WindowShown => {
                self.update_tray(TrayView {
                    window_visible: true,
                    ..self.tray_view
                });
                self.configure_viewports();
                if let Some(action) = self.app.show_preview_action() {
                    self.submit_preview_action(action);
                }
            }
            WindowEffect::FocusWindow { id } => {
                let outcome = self.host.focus().and_then(Visibility::show_outcome);
                self.keep_window_identity();
                self.dispatch_window_intent(WindowIntent::ActionCompleted {
                    id,
                    outcome,
                    now_ms: self.timestamp_ms(),
                });
            }
            WindowEffect::CancelImePreedit => {
                self.app.editor.cancel_preedit();
                self.sync_preedit();
            }
            WindowEffect::SetImeAllowed(allowed) => {
                if allowed {
                    // Allowed means: back to whoever takes the keyboard (`sync_ime_allowed`).
                    self.sync_ime_allowed();
                } else if let Some(window) = &self.window {
                    window.set_ime_allowed(false);
                }
            }
            WindowEffect::SetEditorInputEnabled(enabled) => {
                self.shell_input_enabled = enabled;
                if !enabled {
                    self.scrollbars = Default::default();
                    if self.controls.opacity_input_focused {
                        // Hiding or quitting takes the keyboard from the field, which commits it
                        // (plan 09) before the hide clears the popup or the quit flushes config.
                        self.commit_opacity_input();
                    }
                }
                // Input going away turns the IME off; input coming back (a show, a cancelled hide
                // or quit) gives it to whoever takes the keyboard, which an opacity field edit may
                // have turned off. Before the window is focused that is off, until `Focused(true)`.
                self.sync_ime_allowed();
            }
            WindowEffect::ReleaseHiddenCaches => {
                self.preview_frame = None;
                self.preview_frame_viewport = None;
                self.preview_selection = Default::default();
                self.app.release_hidden_preview();
                self.source_frame = None;
                self.source_paint_key = None;
            }
            WindowEffect::ReportTrayUnavailable => {
                self.diagnostic = Some("系统托盘不可用；窗口未隐藏。".into());
                self.request_redraw();
            }
            WindowEffect::HideBlocked => {
                self.diagnostic = Some("恢复或外部冲突未解决，窗口不会隐藏。".into());
                self.request_redraw();
            }
            WindowEffect::HideRejected(kind) => {
                self.diagnostic = Some(format!("窗口未能隐藏，保持显示（{kind:?}）。"));
                self.request_redraw();
            }
            WindowEffect::ShowRejected(kind) => {
                self.diagnostic = Some(format!("窗口未能显示（{kind:?}）。"));
                self.request_redraw();
            }
            WindowEffect::HideCancelled => {
                self.diagnostic = Some("保存失败，窗口保持显示，内存文本仍保留。".into());
                self.request_redraw();
            }
            WindowEffect::QuitBlocked => {
                self.diagnostic = Some("退出前必须先解决恢复或外部修改冲突。".into());
                self.request_redraw();
                self.host.quit_abandoned();
            }
            WindowEffect::QuitTimedOut => {
                self.diagnostic = Some("退出等待超时，已取消；保存可能仍在进行。".to_owned());
                self.request_redraw();
                self.host.quit_abandoned();
                self.refresh_window_guards();
            }
            WindowEffect::QuitCancelled(barrier) => {
                self.diagnostic = Some(format!("退出已取消：{barrier:?} 未安全完成。"));
                self.request_redraw();
                self.host.quit_abandoned();
                // Quitting stopped the dock timers; a window that stays gets them back.
                self.refresh_window_guards();
            }
            WindowEffect::QuitWarning(barrier) => {
                self.diagnostic = Some(format!("退出前的 {barrier:?} 未完成；证据已保留。"));
            }
            WindowEffect::RequestRedraw => self.request_redraw(),
            WindowEffect::WakeAt(_) => {}
            WindowEffect::RequestNoteSave(_)
            | WindowEffect::RequestSafeAssetGc
            | WindowEffect::RequestConfigFlush
            | WindowEffect::CommitPlacement { .. }
            | WindowEffect::CommitAlwaysOnTop(_)
            | WindowEffect::ExitProcess => {
                debug_assert!(false, "`route_window_effect` keeps these from the shell");
            }
        }
    }

    /// Hides the window and reports what the host saw. Only a confirmed hide tells the tray the
    /// window is gone.
    fn hide_native(&mut self) -> Result<(), FailureKind> {
        let outcome = self.host.hide();
        self.keep_window_identity();
        if outcome.is_ok() {
            self.update_tray(TrayView {
                window_visible: false,
                ..self.tray_view
            });
        }
        outcome
    }

    /// Moves the window. A frame with a verdict is observed at once, because a window already at
    /// its target sends no move event; an animation frame is never observed.
    fn apply_frame(&mut self, id: u64, target: PhysicalRect, last: bool) {
        let result = self.host.apply_frame(target);
        if !last {
            return;
        }
        match result {
            Ok(()) => self.observe_placement(id),
            Err(kind) => self.dispatch_window_intent(WindowIntent::ActionCompleted {
                id,
                outcome: Err(kind),
                now_ms: self.timestamp_ms(),
            }),
        }
    }

    /// Reports where the window is, for placement `id`. No placement waits during a native drag
    /// or resize, so this never reads a window under the user's hand as a confirmation.
    fn observe_placement(&mut self, id: u64) {
        // A display change is still to be applied: the frame read now belongs to the old
        // display. The deferred recovery replaces this placement, or observes it afterwards.
        if self.display_recovery_due {
            return;
        }
        let observed = self.host.observe_frame();
        self.dispatch_window_intent(WindowIntent::PlacementObserved {
            id,
            observed,
            now_ms: self.timestamp_ms(),
        });
    }

    /// The window reported a change: judge the waiting placement, if one is. Without one the
    /// window is not read at all.
    pub(super) fn observe_pending_placement(&mut self) {
        if let Some(id) = self.pending_placement() {
            self.observe_placement(id);
        }
    }

    fn pending_placement(&self) -> Option<u64> {
        self.app
            .window_flow()
            .and_then(|flow| flow.state().pending_placement_id())
    }

    /// Asks for the effective bit and reports what it reads back. A host error's own words stay on
    /// the status line; the coordinator reports the outcome of every other failure.
    fn apply_topmost(&mut self, request: TopmostRequest) {
        let answer = self.host.set_topmost(request.desired_effective);
        let outcome = match &answer {
            Ok(fact) => fact.outcome(request.desired_effective),
            Err(failure) => Err(failure.kind()),
        };
        self.dispatch_window_intent(WindowIntent::ActionCompleted {
            id: request.id,
            outcome,
            now_ms: self.timestamp_ms(),
        });
        if let Err(failure) = answer {
            self.diagnostic = Some(format!("无法应用窗口置顶状态：{failure}"));
            self.request_redraw();
        }
    }

    /// Shows the window and reads back whether it is visible and not minimized. The tray is told
    /// only once the coordinator commits (`WindowShown`).
    fn show_native(&mut self) -> Result<(), FailureKind> {
        let outcome = self.host.show().and_then(Visibility::show_outcome);
        self.keep_window_identity();
        outcome
    }

    fn update_tray(&mut self, view: TrayView) {
        self.tray_view = view;
        self.host.update(view);
    }

    fn keep_window_identity(&mut self) {
        if let Err(error) = self.host.reassert_identity() {
            self.host.restore_ordinary_entry();
            self.diagnostic = Some(format!(
                "无法保持工具窗口身份；已恢复任务栏入口以避免窗口不可达：{error}"
            ));
            self.request_redraw();
        }
    }
}

fn winit_resize_direction(direction: WindowResizeEdge) -> ResizeDirection {
    match direction {
        WindowResizeEdge::North => ResizeDirection::North,
        WindowResizeEdge::NorthEast => ResizeDirection::NorthEast,
        WindowResizeEdge::East => ResizeDirection::East,
        WindowResizeEdge::SouthEast => ResizeDirection::SouthEast,
        WindowResizeEdge::South => ResizeDirection::South,
        WindowResizeEdge::SouthWest => ResizeDirection::SouthWest,
        WindowResizeEdge::West => ResizeDirection::West,
        WindowResizeEdge::NorthWest => ResizeDirection::NorthWest,
    }
}

#[cfg(test)]
mod phase8_runtime_tests {
    use super::*;
    use stickymd_app::flow::window::state::QuitBarrier;

    #[test]
    fn runtime_every_window_effect_preserves_notice_except_failed_note_quit() {
        for active in [false, true] {
            for effect in crate::title::testing::every_effect() {
                let mut app = crate::shell::headless_tests::fixture("saved text");
                if active {
                    app.apply_window_effect(WindowEffect::QuitCancelled(QuitBarrier::NoteSave));
                }
                let expected =
                    active || effect == WindowEffect::QuitCancelled(QuitBarrier::NoteSave);
                let description = format!("{effect:?}");
                app.apply_window_effect(effect);
                assert_eq!(
                    app.quit_notice.active(),
                    expected,
                    "active={active}, {description}"
                );
                assert_eq!(
                    app.quit_notice.settle_count, 0,
                    "effect must not settle a partial event pass"
                );
            }
        }
    }
}
