//! Minimal Windows window-message bridge for opt-in runtime smoke transitions.

use std::thread;
use std::time::Duration;

const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const MK_LBUTTON: usize = 0x0001;

#[derive(Default)]
struct WindowSearch {
    process_id: u32,
    window: isize,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(
        callback: Option<unsafe extern "system" fn(isize, isize) -> i32>,
        parameter: isize,
    ) -> i32;
    fn GetWindowThreadProcessId(window: isize, process_id: *mut u32) -> u32;
    fn IsWindowVisible(window: isize) -> i32;
    fn PostMessageW(window: isize, message: u32, wparam: usize, lparam: isize) -> i32;
}

/// Click StickyMD's Source toolbar control in the visible top-level window
/// owned by `process_id`. This bridge exists only in the development smoke
/// CLI; product runtime code does not expose a test command channel.
pub(crate) fn switch_to_source(process_id: u32) -> Result<(), String> {
    let mut search = WindowSearch {
        process_id,
        window: 0,
    };
    // SAFETY: `search` remains alive and exclusively borrowed until EnumWindows
    // returns; the callback only writes that value during this synchronous call.
    unsafe {
        EnumWindows(
            Some(find_process_window),
            (&raw mut search).cast::<()>() as isize,
        );
    }
    if search.window == 0 {
        return Err(format!(
            "cannot find a visible StickyMD window for process {process_id}"
        ));
    }

    // (16, 10) physical pixels lies inside the first toolbar control for all
    // supported scale factors: the hit test derives the same 34-DIP toolbar
    // scale and maps the first 38-DIP slot to Source mode.
    let position = mouse_lparam(16, 10);
    post_mouse(search.window, WM_MOUSEMOVE, 0, position)?;
    post_mouse(search.window, WM_LBUTTONDOWN, MK_LBUTTON, position)?;
    post_mouse(search.window, WM_LBUTTONUP, 0, position)?;
    // Let the application's message pump consume the ordered sequence before
    // the caller starts polling the durable config acknowledgement.
    thread::sleep(Duration::from_millis(50));
    Ok(())
}

unsafe extern "system" fn find_process_window(window: isize, parameter: isize) -> i32 {
    // SAFETY: EnumWindows invokes this callback synchronously with the pointer
    // to the live `WindowSearch` supplied by `switch_to_source`.
    let search = unsafe { &mut *(parameter as *mut WindowSearch) };
    let mut process_id = 0_u32;
    // SAFETY: `process_id` is a valid writable u32 and `window` is supplied by
    // EnumWindows for the duration of the callback.
    unsafe {
        GetWindowThreadProcessId(window, &raw mut process_id);
    }
    // SAFETY: `window` is the current EnumWindows handle and may be queried
    // during the callback.
    let visible = unsafe { IsWindowVisible(window) } != 0;
    if process_id == search.process_id && visible {
        search.window = window;
        0
    } else {
        1
    }
}

fn post_mouse(
    window: isize,
    message: u32,
    button_state: usize,
    position: isize,
) -> Result<(), String> {
    // SAFETY: `window` was returned by EnumWindows for the live child process;
    // PostMessageW copies these integer mouse-message values and retains no
    // pointers. Down/up are paired before the function returns.
    if unsafe { PostMessageW(window, message, button_state, position) } == 0 {
        Err(format!(
            "cannot post resource-smoke mouse message {message:#x}"
        ))
    } else {
        Ok(())
    }
}

fn mouse_lparam(x: u16, y: u16) -> isize {
    let x = isize::try_from(x).unwrap_or_default();
    let y = isize::try_from(y).unwrap_or_default();
    x | (y << 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_lparam_packs_client_coordinates() {
        let packed = mouse_lparam(16, 10);
        assert_eq!(packed & 0xffff, 16);
        assert_eq!((packed >> 16) & 0xffff, 10);
    }
}
