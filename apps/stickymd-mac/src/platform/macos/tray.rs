//! Event-driven macOS menu-bar adapter.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

use crossbeam_channel::{Sender, bounded, select_biased};
use stickymd_app::ports::TrayCommand;
use stickymd_desktop::tray::{
    ALWAYS_ON_TOP_ID, ALWAYS_ON_TOP_LABEL, QUIT_ID, QUIT_LABEL, SHOW_HIDE_ID, TOOLTIP,
    TrayIconRgba, command_for_menu_id, show_hide_label,
};
use thiserror::Error;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

static TRAY_INSTANCE_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Error)]
pub enum TrayAdapterError {
    #[error("only one StickyMD tray controller may be active per process")]
    InstanceAlreadyActive,
    #[error("invalid tray icon pixels: {0}")]
    Icon(#[from] tray_icon::BadIcon),
    #[error("failed to construct the three-item tray menu: {0}")]
    Menu(#[from] tray_icon::menu::Error),
    #[error("failed to create the menu-bar icon: {0}")]
    Tray(#[from] tray_icon::Error),
    #[error("failed to start the event-driven tray dispatcher: {0}")]
    Dispatcher(#[from] std::io::Error),
}

/// Owns the native icon and its exactly-three-item context menu.
///
/// Creation and mutation must happen on the UI thread. Event handlers only
/// translate vendor events to `TrayCommand`; they never mutate app state.
pub struct TrayController {
    tray_icon: Option<TrayIcon>,
    show_hide: MenuItem,
    always_on_top: CheckMenuItem,
    dispatcher_shutdown: Sender<()>,
    dispatcher: Option<JoinHandle<()>>,
}

impl TrayController {
    pub fn create<F>(
        icon: TrayIconRgba,
        window_visible: bool,
        always_on_top: bool,
        callback: F,
    ) -> Result<Self, TrayAdapterError>
    where
        F: Fn(TrayCommand) + Send + Sync + 'static,
    {
        if TRAY_INSTANCE_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(TrayAdapterError::InstanceAlreadyActive);
        }

        let result =
            Self::create_after_reservation(icon, window_visible, always_on_top, Arc::new(callback));
        if result.is_err() {
            TRAY_INSTANCE_ACTIVE.store(false, Ordering::Release);
        }
        result
    }

    fn create_after_reservation(
        icon: TrayIconRgba,
        window_visible: bool,
        always_on_top: bool,
        callback: Arc<dyn Fn(TrayCommand) + Send + Sync>,
    ) -> Result<Self, TrayAdapterError> {
        let icon = Icon::from_rgba(icon.rgba, icon.width, icon.height)?;
        let menu = Menu::new();
        let show_hide =
            MenuItem::with_id(SHOW_HIDE_ID, show_hide_label(window_visible), true, None);
        let always_on_top = CheckMenuItem::with_id(
            ALWAYS_ON_TOP_ID,
            ALWAYS_ON_TOP_LABEL,
            true,
            always_on_top,
            None,
        );
        let quit = MenuItem::with_id(QUIT_ID, QUIT_LABEL, true, None);
        menu.append_items(&[&show_hide, &always_on_top, &quit])?;

        let tray_icon = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip(TOOLTIP)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_menu_on_right_click(true)
            .build()?;

        // `tray-icon` exposes process-global receivers and write-once handler
        // slots. A joinable receiver thread avoids retaining an application
        // callback forever, permits safe controller recreation in tests, and
        // continuously drains ignored icon events without polling.
        let (dispatcher_shutdown, shutdown_receiver) = bounded(1);
        let dispatcher = thread::Builder::new()
            .name("stickymd-tray-events".into())
            .stack_size(256 * 1024)
            .spawn(move || {
                let menu_events = MenuEvent::receiver();
                let tray_events = TrayIconEvent::receiver();
                loop {
                    select_biased! {
                        recv(shutdown_receiver) -> _ => break,
                        recv(menu_events) -> event => {
                            let Ok(event) = event else { break };
                            if let Some(event) = translate_menu_event(&event) {
                                callback(event);
                            }
                        },
                        // Icon clicks/movement are intentionally not product
                        // commands in v1; consume them only to keep the
                        // library's unbounded global receiver drained.
                        recv(tray_events) -> event => {
                            if event.is_err() {
                                break;
                            }
                        },
                    }
                }
            })?;

        Ok(Self {
            tray_icon: Some(tray_icon),
            show_hide,
            always_on_top,
            dispatcher_shutdown,
            dispatcher: Some(dispatcher),
        })
    }

    pub fn set_window_visible(&self, visible: bool) {
        self.show_hide.set_text(show_hide_label(visible));
    }

    pub fn set_always_on_top(&self, always_on_top: bool) {
        self.always_on_top.set_checked(always_on_top);
    }
}

impl Drop for TrayController {
    fn drop(&mut self) {
        let _ = self.dispatcher_shutdown.try_send(());
        if let Some(dispatcher) = self.dispatcher.take() {
            let _ = dispatcher.join();
        }
        // Destroy the native icon before making another controller eligible.
        drop(self.tray_icon.take());
        TRAY_INSTANCE_ACTIVE.store(false, Ordering::Release);
    }
}

fn translate_menu_event(event: &MenuEvent) -> Option<TrayCommand> {
    command_for_menu_id(event.id().0.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use stickymd_desktop::testing::contracts;

    #[test]
    fn show_hide_label_describes_the_action() {
        contracts::tray_show_hide_label_describes_the_action();
    }

    #[test]
    fn only_the_three_menu_items_translate_to_platform_events() {
        let show_hide = MenuEvent {
            id: SHOW_HIDE_ID.into(),
        };
        let always_on_top = MenuEvent {
            id: ALWAYS_ON_TOP_ID.into(),
        };
        let quit = MenuEvent { id: QUIT_ID.into() };
        let unrelated = MenuEvent {
            id: "other.menu.item".into(),
        };

        assert_eq!(
            translate_menu_event(&show_hide),
            Some(TrayCommand::ToggleVisibility)
        );
        assert_eq!(
            translate_menu_event(&always_on_top),
            Some(TrayCommand::ToggleAlwaysOnTop)
        );
        assert_eq!(translate_menu_event(&quit), Some(TrayCommand::Quit));
        assert_eq!(translate_menu_event(&unrelated), None);
    }

    #[test]
    fn tray_icon_is_fully_owned_rgba() {
        contracts::tray_icon_is_fully_owned_rgba();
    }
}
