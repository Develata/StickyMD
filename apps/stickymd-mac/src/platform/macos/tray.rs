//! Event-driven macOS menu-bar adapter.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

use crossbeam_channel::{Sender, bounded, select_biased};
use stickymd_app::ports::TrayCommand;
use thiserror::Error;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

const SHOW_HIDE_ID: &str = "stickymd.tray.show-hide";
const ALWAYS_ON_TOP_ID: &str = "stickymd.tray.always-on-top";
const QUIT_ID: &str = "stickymd.tray.quit";

static TRAY_INSTANCE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Vendor-neutral RGBA input for the tray icon.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrayIconRgba {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

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
        let always_on_top =
            CheckMenuItem::with_id(ALWAYS_ON_TOP_ID, "置顶", true, always_on_top, None);
        let quit = MenuItem::with_id(QUIT_ID, "退出", true, None);
        menu.append_items(&[&show_hide, &always_on_top, &quit])?;

        let tray_icon = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("StickyMD")
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
    match event.id().0.as_str() {
        SHOW_HIDE_ID => Some(TrayCommand::ToggleVisibility),
        ALWAYS_ON_TOP_ID => Some(TrayCommand::ToggleAlwaysOnTop),
        QUIT_ID => Some(TrayCommand::Quit),
        _ => None,
    }
}

/// The StickyMD icon, a sheet with three lines of ink.
pub fn tray_icon() -> TrayIconRgba {
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

const fn show_hide_label(visible: bool) -> &'static str {
    if visible { "隐藏" } else { "显示" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_hide_label_describes_the_action() {
        assert_eq!(show_hide_label(true), "隐藏");
        assert_eq!(show_hide_label(false), "显示");
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
        let icon = tray_icon();
        assert_eq!(
            icon.rgba.len(),
            icon.width as usize * icon.height as usize * 4
        );
        assert!(icon.rgba.chunks_exact(4).any(|pixel| pixel[3] == 255));
    }
}
