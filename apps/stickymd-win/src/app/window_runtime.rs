//! Applies typed window-domain effects to passive winit and Windows adapters.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

use winit::event_loop::ActiveEventLoop;
use winit::platform::windows::WindowExtWindows;
use winit::window::{ResizeDirection, Theme};

use super::StickyApp;
use super::window_geometry_runtime::{apply_frame, configured_monitor, monitor_geometries};
use crate::assets::AssetReconcileMode;
use crate::config::{DockEdge as ConfigDockEdge, ViewMode};
use crate::flow::window::geometry::WindowPlacement;
use crate::flow::window::reducer::WindowShellCoordinator;
use crate::flow::window::state::{
    DockEdge, QuitBarrier, StableVisibility, WindowEffect, WindowGuardSnapshot, WindowIntent,
    WindowSaveReason, effective_topmost,
};
use crate::flow::{SaveTrigger, WindowPreferenceEffect, coordinate_window_preference};
use crate::instruction::{WindowPlatformIntent, WindowPreferenceIntent, WindowResizeEdge};
use crate::platform::windows::tray::{TrayController, TrayIconRgba};
use crate::platform::windows::window_opacity::set_window_opacity;
use crate::platform::windows::window_topmost::set_window_topmost_no_activate;

impl StickyApp {
    pub(super) fn dispatch_window_platform_intent(&mut self, intent: WindowPlatformIntent) -> bool {
        let Some(window) = &self.window else {
            return false;
        };
        let result = match intent {
            WindowPlatformIntent::RequestDrag => window.drag_window(),
            WindowPlatformIntent::RequestResize(direction) => {
                window.drag_resize_window(winit_resize_direction(direction))
            }
        };
        match result {
            Ok(()) => {
                self.move_resize_active = true;
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
        let effect = match coordinate_window_preference(&mut self.config, intent) {
            Ok(effect) => effect,
            Err(error) => {
                self.diagnostic = Some(error.to_string());
                return;
            }
        };
        match effect {
            WindowPreferenceEffect::NoOp => {}
            WindowPreferenceEffect::ApplyTheme(theme) => {
                if let Some(window) = &self.window {
                    window.set_theme(match theme {
                        crate::config::ThemeMode::Light => Some(Theme::Light),
                        crate::config::ThemeMode::Dark => Some(Theme::Dark),
                        crate::config::ThemeMode::System => None,
                    });
                }
                self.source_paint_key = None;
                self.request_preview_relayout();
                self.submit_config_if_needed();
                self.request_redraw();
            }
            WindowPreferenceEffect::ApplyOpacity { opacity, persist } => {
                let opacity = self.controls.preview(opacity);
                if let Some(window) = &self.window
                    && let Err(error) = set_window_opacity(window.as_ref(), opacity)
                {
                    self.diagnostic = Some(format!("无法预览透明度：{error}"));
                }
                if persist {
                    self.submit_config_if_needed();
                }
                self.request_redraw();
            }
            WindowPreferenceEffect::ApplyAlwaysOnTop(topmost) => {
                let temporary = self
                    .window_flow
                    .as_ref()
                    .is_some_and(|flow| flow.state().temporary_sensor_topmost());
                self.apply_effective_topmost(temporary);
                if let Some(tray) = &self.tray {
                    tray.set_always_on_top(topmost);
                }
                self.submit_config_if_needed();
                self.request_redraw();
            }
            WindowPreferenceEffect::ApplyContentZoom(_) => {
                self.source_paint_key = None;
                self.configure_viewports();
                self.request_preview_relayout();
                self.update_ime_area();
                self.zoom_config_deadline = Some(self.timestamp_ms().saturating_add(250));
                self.request_redraw();
            }
        }
    }

    pub(super) fn apply_effective_topmost(&mut self, temporary: bool) {
        let Some(window) = &self.window else { return };
        let effective = effective_topmost(self.config.current().always_on_top, temporary);
        if let Err(error) = set_window_topmost_no_activate(window.as_ref(), effective) {
            self.diagnostic = Some(format!("无法应用窗口置顶状态：{error}"));
            self.request_redraw();
        }
    }

    pub(super) fn initialize_window_shell(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let Some(window) = self.window.as_ref().cloned() else {
            return false;
        };
        let monitors = monitor_geometries(&window);
        let Some(monitor) = configured_monitor(&monitors, &self.config.current().window.monitor_id)
        else {
            self.diagnostic = Some("Windows 未提供可用显示器；无法安全放置窗口。".into());
            return false;
        };
        self.startup_diagnostics.record("monitor_ready");
        let configured = &self.config.current().window;
        let placement = WindowPlacement::new(
            configured.width_dip as f64,
            configured.height_dip as f64,
            Some(monitor.identity.clone()),
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
        let proxy = self.proxy.clone();
        let tray = match TrayController::create(
            tray_icon(),
            true,
            self.config.current().always_on_top,
            move |event| {
                let _ = proxy.send_event(super::AppEvent::Tray(event));
            },
        ) {
            Ok(tray) => Some(tray),
            Err(error) => {
                let message = format!(
                    "无法创建系统托盘图标；StickyMD 将保持窗口可见，关闭窗口会安全保存并退出。\n\n{error}"
                );
                crate::platform::windows::message_box::show_error("StickyMD", &message);
                self.diagnostic = Some(message);
                None
            }
        };
        let tray_available = tray.is_some();
        if tray_available {
            if let Err(error) =
                crate::platform::windows::tool_window::apply_tool_window_identity(window.as_ref())
            {
                crate::platform::windows::message_box::show_error(
                    "StickyMD",
                    &format!(
                        "无法建立安全的工具窗口身份；StickyMD 不会显示无恢复入口的窗口。\n\n{error}"
                    ),
                );
                return false;
            }
        } else {
            // A tool-window-only lifecycle without a tray recovery path is
            // forbidden. Restore ordinary taskbar reachability before show.
            window.set_skip_taskbar(false);
        }
        self.window_flow = Some(WindowShellCoordinator::new(
            placement,
            visibility,
            monitor,
            tray_available,
        ));
        if let Some(frame) = self.window_flow.as_ref().map(|flow| flow.state().frame()) {
            apply_frame(&window, frame);
        }
        self.tray = tray;
        self.startup_diagnostics.record("tray_ready");
        self.dispatch_window_intent(
            Some(event_loop),
            WindowIntent::SplitModeChanged {
                split: self.config.current().view_mode == ViewMode::Split,
            },
        );
        window.set_visible(true);
        if let Err(error) = self.reassert_tool_window_identity() {
            crate::platform::windows::message_box::show_error(
                "StickyMD",
                &format!("窗口显示后无法保持安全的工具窗口身份；StickyMD 将停止启动。\n\n{error}"),
            );
            return false;
        }
        self.startup_diagnostics.record("window_visible");
        // Winit may refresh native extended styles while making the window
        // visible. Apply layered alpha and z-order after that transition so
        // the configured projection is the final native fact.
        if let Err(error) = set_window_opacity(window.as_ref(), self.config.current().opacity) {
            self.diagnostic = Some(format!("无法应用窗口透明度：{error}"));
            return false;
        }
        if let Err(error) = set_window_topmost_no_activate(
            window.as_ref(),
            effective_topmost(
                self.config.current().always_on_top,
                self.window_flow
                    .as_ref()
                    .is_some_and(|flow| flow.state().temporary_sensor_topmost()),
            ),
        ) {
            self.diagnostic = Some(format!("无法应用窗口置顶状态：{error}"));
            return false;
        }
        window.focus_window();
        self.refresh_window_guards(None);
        self.startup_diagnostics.record("shell_ready");
        true
    }

    pub(super) fn window_guards(&self) -> WindowGuardSnapshot {
        WindowGuardSnapshot {
            window_focused: self.session.focused,
            ime_composing: self.session.is_composing(),
            dragging: self.move_resize_active,
            popup_open: self.controls.opacity_popup_open || self.export_in_flight,
            conflict_or_recovery: self.recovery.is_pending()
                || self.persistence.conflict().is_some(),
            paste_pending: self.asset_paste_pending,
            asset_transaction_pending: self.asset_sync_in_flight,
            note_save_required: self.coordinator.view().dirty
                || self.persistence.durability_required(),
            note_save_in_flight: self.persistence.note_save_in_flight(),
        }
    }

    pub(super) fn refresh_window_guards(&mut self, event_loop: Option<&ActiveEventLoop>) {
        self.dispatch_window_intent(
            event_loop,
            WindowIntent::GuardsChanged {
                guards: self.window_guards(),
                now_ms: self.timestamp_ms(),
            },
        );
    }

    pub(super) fn dispatch_window_intent(
        &mut self,
        event_loop: Option<&ActiveEventLoop>,
        intent: WindowIntent,
    ) {
        let Some(flow) = &mut self.window_flow else {
            return;
        };
        let effects = flow.dispatch(intent);
        for effect in effects {
            self.apply_window_effect(event_loop, effect);
        }
    }

    fn apply_window_effect(&mut self, event_loop: Option<&ActiveEventLoop>, effect: WindowEffect) {
        match effect {
            WindowEffect::ApplyFrame(frame) => {
                if let Some(window) = &self.window {
                    apply_frame(window, frame);
                }
            }
            WindowEffect::SetTemporarySensorTopmost(temporary) => {
                self.apply_effective_topmost(temporary);
            }
            WindowEffect::SetVisible(visible) => {
                if !visible {
                    self.controls.opacity_popup_open = false;
                    self.controls.opacity_dragging = false;
                    self.controls.opacity_input_focused = false;
                    self.controls.opacity_preview = self.config.current().opacity;
                    self.controls.opacity_input = self.config.current().opacity.to_string();
                }
                if let Some(window) = &self.window {
                    window.set_visible(visible);
                    if visible {
                        window.set_minimized(false);
                    }
                }
                self.reassert_tool_window_identity_or_restore_taskbar();
                if let Some(tray) = &self.tray {
                    tray.set_window_visible(visible);
                }
                if visible {
                    self.configure_viewports();
                    let generation = self.coordinator.view().generation;
                    if let Some(action) = self
                        .preview_flow
                        .show(generation, self.preview_visibility())
                    {
                        self.submit_preview_action(action);
                    }
                }
            }
            WindowEffect::FocusWindow => {
                if let Some(window) = &self.window {
                    window.set_minimized(false);
                    window.focus_window();
                }
                self.reassert_tool_window_identity_or_restore_taskbar();
            }
            WindowEffect::CancelImePreedit => {
                self.session.cancel_preedit();
                self.sync_preedit();
            }
            WindowEffect::SetImeAllowed(allowed) => {
                if let Some(window) = &self.window {
                    window.set_ime_allowed(allowed && !self.preview_focused);
                }
            }
            WindowEffect::SetEditorInputEnabled(enabled) => {
                self.shell_input_enabled = enabled;
            }
            WindowEffect::RequestNoteSave(reason) => {
                self.request_immediate_save(match reason {
                    WindowSaveReason::HideToTray => SaveTrigger::HideToTray,
                    WindowSaveReason::Shutdown => SaveTrigger::Shutdown,
                });
            }
            WindowEffect::RequestSafeAssetGc => self.submit_asset_sync(
                self.coordinator.view().generation,
                Vec::new(),
                Some(AssetReconcileMode::SafeBoundary),
                true,
            ),
            WindowEffect::RequestConfigFlush => self.request_quit_config_flush(event_loop),
            WindowEffect::ExitProcess => {
                self.tray = None;
                if let Some(event_loop) = event_loop {
                    event_loop.exit();
                } else {
                    self.diagnostic = Some("退出屏障完成，但当前事件没有退出权限。".into());
                    self.shell_input_enabled = true;
                    self.request_redraw();
                }
            }
            WindowEffect::CommitPlacement {
                placement,
                dock_edge,
            } => self.commit_window_placement(&placement, dock_edge),
            WindowEffect::ReleaseHiddenCaches => {
                self.preview_frame = None;
                self.preview_selection = Default::default();
                self.preview_flow.release_projection();
                if let Some(worker) = &self.preview_worker {
                    worker.release_raster_caches();
                }
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
            WindowEffect::HideCancelled => {
                self.diagnostic = Some("保存失败，窗口保持显示，内存文本仍保留。".into());
                self.request_redraw();
            }
            WindowEffect::QuitBlocked => {
                self.diagnostic = Some("退出前必须先解决恢复或外部修改冲突。".into());
                self.request_redraw();
            }
            WindowEffect::QuitCancelled(barrier) => {
                self.diagnostic = Some(format!("退出已取消：{barrier:?} 未安全完成。"));
                self.request_redraw();
            }
            WindowEffect::QuitWarning(barrier) => {
                self.diagnostic = Some(format!("退出前的 {barrier:?} 未完成；证据已保留。"));
            }
            WindowEffect::RequestRedraw => self.request_redraw(),
            WindowEffect::WakeAt(_) => {}
        }
    }

    fn reassert_tool_window_identity(
        &self,
    ) -> Result<(), crate::platform::windows::tool_window::ToolWindowError> {
        if self.tray.is_none() {
            return Ok(());
        }
        let Some(window) = &self.window else {
            return Ok(());
        };
        crate::platform::windows::tool_window::apply_tool_window_identity(window.as_ref())
    }

    fn reassert_tool_window_identity_or_restore_taskbar(&mut self) {
        if let Err(error) = self.reassert_tool_window_identity() {
            if let Some(window) = &self.window {
                window.set_skip_taskbar(false);
            }
            self.diagnostic = Some(format!(
                "无法保持工具窗口身份；已恢复任务栏入口以避免窗口不可达：{error}"
            ));
            self.request_redraw();
        }
    }

    fn request_quit_config_flush(&mut self, event_loop: Option<&ActiveEventLoop>) {
        if !self.config_persistence_allowed {
            self.dispatch_window_intent(
                event_loop,
                WindowIntent::QuitBarrierCompleted {
                    barrier: QuitBarrier::Config,
                    succeeded: false,
                    guards: self.window_guards(),
                },
            );
            return;
        }
        self.submit_config_if_needed();
        if !self.config.is_dirty() && !self.config.is_saving() {
            self.dispatch_window_intent(
                event_loop,
                WindowIntent::QuitBarrierCompleted {
                    barrier: QuitBarrier::Config,
                    succeeded: true,
                    guards: self.window_guards(),
                },
            );
        }
    }

    fn commit_window_placement(
        &mut self,
        placement: &WindowPlacement,
        dock_edge: Option<DockEdge>,
    ) {
        let result = self.config.update(|config| {
            config.window.width_dip = placement.width_dip.round().clamp(220.0, 16_384.0) as u32;
            config.window.height_dip = placement.height_dip.round().clamp(120.0, 16_384.0) as u32;
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
            Err(error) => self.diagnostic = Some(error.to_string()),
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

fn tray_icon() -> TrayIconRgba {
    let width = 32;
    let height = 32;
    let mut rgba = vec![0; width * height * 4];
    for y in 2..30 {
        for x in 3..29 {
            let index = (y * width + x) * 4;
            let border = x == 3 || x == 28 || y == 2 || y == 29;
            let ink = (7..=24).contains(&x)
                && ((6..=9).contains(&y) || (14..=17).contains(&y) || (23..=26).contains(&y))
                && (x <= 12 || x >= 19 || (14..=17).contains(&y));
            let color = if border {
                [120, 96, 38, 255]
            } else if ink {
                [54, 48, 36, 255]
            } else {
                [250, 238, 188, 255]
            };
            rgba[index..index + 4].copy_from_slice(&color);
        }
    }
    TrayIconRgba {
        rgba,
        width: width as u32,
        height: height as u32,
    }
}

#[cfg(test)]
mod phase8_runtime_tests {
    use super::*;

    #[test]
    fn phase8_tray_icon_is_fully_owned_rgba() {
        let icon = tray_icon();
        assert_eq!(
            icon.rgba.len(),
            icon.width as usize * icon.height as usize * 4
        );
        assert!(icon.rgba.chunks_exact(4).any(|pixel| pixel[3] == 255));
    }
}
