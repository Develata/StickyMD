//! Read a checked native popup item only while its owner is the tested process.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#physical-input-injection

use super::active_tray_popup_owner;

const MN_GETHMENU: u32 = 481;
const MF_BYPOSITION: u32 = 1024;
const MF_CHECKED: u32 = 8;
const SMTO_ABORTIFHUNG: u32 = 2;

#[link(name = "user32")]
unsafe extern "system" {
    fn FindWindowExW(parent: isize, after: isize, class: *const u16, title: *const u16) -> isize;
    fn GetWindowThreadProcessId(window: isize, process: *mut u32) -> u32;
    fn SendMessageTimeoutW(
        window: isize,
        message: u32,
        wparam: usize,
        lparam: isize,
        flags: u32,
        timeout: u32,
        result: *mut usize,
    ) -> isize;
    fn GetMenuItemCount(menu: isize) -> i32;
    fn GetMenuStringW(menu: isize, item: u32, buffer: *mut u16, capacity: i32, flags: u32) -> i32;
    fn GetMenuState(menu: isize, item: u32, flags: u32) -> u32;
}

pub(crate) fn tray_item_checked(process: u32, expected: &str) -> Result<bool, String> {
    if active_tray_popup_owner()? != Some(process) {
        return Err("native menu check state requires the target's active popup".to_owned());
    }
    let class: Vec<u16> = "#32768\0".encode_utf16().collect();
    let mut previous = 0;
    let mut matched = None;
    let mut exhausted = false;
    // A bounded read-only enumeration; no messages that open or invoke a menu.
    for _ in 0..64 {
        // SAFETY: the NUL-terminated class buffer lives through the call. HWNDs
        // are borrowed; null parent enumerates top-level native menu windows.
        let popup = unsafe { FindWindowExW(0, previous, class.as_ptr(), std::ptr::null()) };
        if popup == 0 {
            exhausted = true;
            break;
        }
        previous = popup;
        let mut owner = 0;
        // SAFETY: owner is writable DWORD storage; the API retains no pointer.
        if unsafe { GetWindowThreadProcessId(popup, &raw mut owner) } == 0 || owner != process {
            continue;
        }
        let mut menu = 0;
        // SAFETY: this system popup query returns a borrowed HMENU in result,
        // bounded by 250 ms; it sends no keyboard/mouse event or menu command.
        if unsafe {
            SendMessageTimeoutW(
                popup,
                MN_GETHMENU,
                0,
                0,
                SMTO_ABORTIFHUNG,
                250,
                &raw mut menu,
            )
        } == 0
            || menu == 0
        {
            return Err("cannot query the owned popup's HMENU".to_owned());
        }
        // SAFETY: a borrowed HMENU from the live popup; invalidation yields -1.
        let count = unsafe { GetMenuItemCount(menu as isize) };
        if !(1..=16).contains(&count) {
            return Err(format!("invalid native menu count {count}"));
        }
        for item in 0..count as u32 {
            let mut name = [0_u16; 128];
            // SAFETY: the buffer is writable for capacity UTF-16 units; no
            // pointer escapes and the menu handle remains borrowed.
            let len = unsafe {
                GetMenuStringW(
                    menu as isize,
                    item,
                    name.as_mut_ptr(),
                    name.len() as i32,
                    MF_BYPOSITION,
                )
            };
            if len <= 0 || len as usize >= name.len() - 1 {
                return Err("native menu item name unavailable or truncated".to_owned());
            }
            if String::from_utf16_lossy(&name[..len as usize]) != expected {
                continue;
            }
            // SAFETY: this is a read-only state query by position in the same
            // borrowed menu; u32::MAX reports an invalid handle/item.
            let state = unsafe { GetMenuState(menu as isize, item, MF_BYPOSITION) };
            if state == u32::MAX || matched.is_some() {
                return Err("native menu state invalid or target item duplicated".to_owned());
            }
            matched = Some(state & MF_CHECKED != 0);
        }
    }
    if !exhausted {
        return Err("native popup enumeration exceeded its bound".to_owned());
    }
    if active_tray_popup_owner()? != Some(process) {
        return Err("popup owner changed while reading native menu state".to_owned());
    }
    matched.ok_or_else(|| format!("owned native popup has no {expected:?} item"))
}
