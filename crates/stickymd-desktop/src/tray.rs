//! The tray menu every host builds: its three items, their labels and identifiers, the command each
//! identifier means, and the icon. A host owns the native icon and menu (`tray-icon`) and turns a
//! menu event into a command with [`command_for_menu_id`].
//!
//! plan_ref: docs/plan/09_windows_shell.md#tray-lifecycle

use stickymd_app::ports::TrayCommand;

pub const SHOW_HIDE_ID: &str = "stickymd.tray.show-hide";
pub const ALWAYS_ON_TOP_ID: &str = "stickymd.tray.always-on-top";
pub const QUIT_ID: &str = "stickymd.tray.quit";

pub const ALWAYS_ON_TOP_LABEL: &str = "置顶";
pub const QUIT_LABEL: &str = "退出";
pub const TOOLTIP: &str = "StickyMD";

/// What the show/hide item does now: hide a visible window, show a hidden one.
pub const fn show_hide_label(visible: bool) -> &'static str {
    if visible { "隐藏" } else { "显示" }
}

/// The command a menu item stands for; any other identifier is not ours.
pub fn command_for_menu_id(id: &str) -> Option<TrayCommand> {
    match id {
        SHOW_HIDE_ID => Some(TrayCommand::ToggleVisibility),
        ALWAYS_ON_TOP_ID => Some(TrayCommand::ToggleAlwaysOnTop),
        QUIT_ID => Some(TrayCommand::Quit),
        _ => None,
    }
}

/// Vendor-neutral RGBA input for the tray icon.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrayIconRgba {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_hide_label_describes_the_action() {
        assert_eq!(show_hide_label(true), "隐藏");
        assert_eq!(show_hide_label(false), "显示");
    }

    #[test]
    fn only_the_three_menu_items_translate_to_commands() {
        assert_eq!(
            command_for_menu_id(SHOW_HIDE_ID),
            Some(TrayCommand::ToggleVisibility)
        );
        assert_eq!(
            command_for_menu_id(ALWAYS_ON_TOP_ID),
            Some(TrayCommand::ToggleAlwaysOnTop)
        );
        assert_eq!(command_for_menu_id(QUIT_ID), Some(TrayCommand::Quit));
        assert_eq!(command_for_menu_id("other.menu.item"), None);
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
