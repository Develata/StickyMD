//! Which processes can own a StickyMD tray icon, read without the program's file name.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#physical-input-injection

/// The window class `tray-icon` registers for each icon's owner window. It creates that window,
/// a hidden top-level one, before it registers the icon, and keeps it when registration fails
/// (`tray-icon` 0.24.2 per `Cargo.lock`, `src/platform_impl/windows/mod.rs` `TrayIcon::new`).
/// Recheck on a `tray-icon` upgrade; a renamed class makes the tray exit refuse, since its
/// target then owns no such window.
const TRAY_OWNER_CLASS: &str = "tray_icon_app";
const MAX_OWNER_WINDOWS: usize = 256;
/// Longer than the class name, so a longer class with the same prefix never matches.
const CLASS_CAPACITY: usize = 64;

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(
        callback: Option<unsafe extern "system" fn(isize, isize) -> i32>,
        parameter: isize,
    ) -> i32;
    fn GetClassNameW(window: isize, name: *mut u16, capacity: i32) -> i32;
    fn GetWindowThreadProcessId(window: isize, process: *mut u32) -> u32;
    fn GetWindowTextLengthW(window: isize) -> i32;
    fn GetWindowTextW(window: isize, text: *mut u16, capacity: i32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetLastError(code: u32);
}

struct OwnerSearch {
    class: Vec<u16>,
    pids: Vec<u32>,
    /// Why the enumeration cannot be trusted; it stops at the first failure.
    failure: Option<String>,
}

/// The process of every `tray-icon` owner window on the desktop, hidden ones included: a
/// StickyMD tray icon, whatever its program is named and wherever the shell shows the icon,
/// belongs to one of them. Only a complete enumeration is returned; any failure, of the
/// enumeration or of a window's query, is an error rather than a partial list. The list is not
/// an atomic snapshot: a window created during or after the enumeration is the same residual as
/// a copy started before the click, refused at its popup by the caller.
pub(crate) fn tray_icon_owner_pids() -> Result<Vec<u32>, String> {
    let mut search = OwnerSearch {
        class: TRAY_OWNER_CLASS.encode_utf16().collect(),
        pids: Vec::new(),
        failure: None,
    };
    // SAFETY: `search` stays alive and exclusively borrowed until EnumWindows returns; the
    // callback only uses it during this synchronous call.
    let completed =
        unsafe { EnumWindows(Some(collect_owner), (&raw mut search).cast::<()>() as isize) } != 0;
    let enumeration_error = (!completed).then(|| std::io::Error::last_os_error().to_string());
    complete(search, enumeration_error)
}

/// The owners of a finished enumeration, only when nothing failed: a list gathered before a
/// failure (even one that already holds the target) proves nothing about the rest.
fn complete(search: OwnerSearch, enumeration_error: Option<String>) -> Result<Vec<u32>, String> {
    if let Some(failure) = search.failure {
        return Err(format!("tray ownership unproven: {failure}"));
    }
    if let Some(error) = enumeration_error {
        return Err(format!(
            "tray ownership unproven: top-level window enumeration failed: {error}"
        ));
    }
    let mut pids = search.pids;
    pids.sort_unstable();
    pids.dedup();
    Ok(pids)
}

unsafe extern "system" fn collect_owner(window: isize, parameter: isize) -> i32 {
    // SAFETY: EnumWindows invokes this callback synchronously with the pointer to the live
    // `OwnerSearch` supplied by `tray_icon_owner_pids`.
    let search = unsafe { &mut *(parameter as *mut OwnerSearch) };
    let mut class = [0_u16; CLASS_CAPACITY];
    // SAFETY: `class` is writable for its capacity; `window` is the current EnumWindows handle.
    let length = unsafe { GetClassNameW(window, class.as_mut_ptr(), class.len() as i32) };
    if length <= 0 {
        // A window destroyed mid-enumeration also lands here; whether it was an owner is unknown.
        search.failure = Some(format!(
            "cannot read the class of window {window:#x}: {}",
            std::io::Error::last_os_error()
        ));
        return 0;
    }
    if class[..length as usize] != search.class[..] {
        return 1;
    }
    let mut pid = 0;
    // SAFETY: `pid` is writable DWORD storage; the API retains no pointer.
    if unsafe { GetWindowThreadProcessId(window, &raw mut pid) } == 0 {
        search.failure = Some(format!(
            "cannot read the process of tray owner window {window:#x}"
        ));
        return 0;
    }
    if search.pids.len() == MAX_OWNER_WINDOWS {
        search.failure = Some(format!(
            "more than {MAX_OWNER_WINDOWS} {TRAY_OWNER_CLASS} windows"
        ));
        return 0;
    }
    search.pids.push(pid);
    1
}

/// Every StickyMD window title starts with this (`apps/stickymd-win/src/app/title.rs`), and the
/// window is created with it before the tray icon.
const STICKYMD_TITLE_PREFIX: &str = "StickyMD";

struct TitleSearch {
    pid: u32,
    found: bool,
    failure: Option<String>,
}

/// Whether `pid` has a top-level window, hidden ones included, titled like StickyMD: evidence
/// from the running process itself, which a renamed copy keeps even when the file at its image
/// path has since been moved or replaced. A title that cannot be read is an error.
pub(crate) fn has_stickymd_window(pid: u32) -> Result<bool, String> {
    let mut search = TitleSearch {
        pid,
        found: false,
        failure: None,
    };
    // SAFETY: `search` stays alive and exclusively borrowed until EnumWindows returns; the
    // callback only uses it during this synchronous call.
    let completed =
        unsafe { EnumWindows(Some(find_titled), (&raw mut search).cast::<()>() as isize) } != 0;
    if let Some(failure) = search.failure {
        return Err(format!("cannot tell whether {pid} is StickyMD: {failure}"));
    }
    if search.found {
        return Ok(true);
    }
    if !completed {
        return Err(format!(
            "cannot tell whether {pid} is StickyMD: top-level window enumeration failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(false)
}

unsafe extern "system" fn find_titled(window: isize, parameter: isize) -> i32 {
    // SAFETY: EnumWindows invokes this callback synchronously with the pointer to the live
    // `TitleSearch` supplied by `has_stickymd_window`.
    let search = unsafe { &mut *(parameter as *mut TitleSearch) };
    let mut pid = 0;
    // SAFETY: `pid` is writable DWORD storage; the API retains no pointer.
    if unsafe { GetWindowThreadProcessId(window, &raw mut pid) } == 0 {
        // A window destroyed mid-enumeration; whose it was is unknown.
        search.failure = Some(format!("cannot read the process of window {window:#x}"));
        return 0;
    }
    if pid != search.pid {
        return 1;
    }
    match window_title(window) {
        Ok(title) if title.starts_with(STICKYMD_TITLE_PREFIX) => {
            search.found = true;
            0
        }
        Ok(_) => 1,
        Err(failure) => {
            search.failure = Some(failure);
            0
        }
    }
}

/// The title of `window`, telling an empty title from a failed read by the last error.
fn window_title(window: isize) -> Result<String, String> {
    let failed = |what: &str| {
        let error = std::io::Error::last_os_error();
        (error.raw_os_error() != Some(0))
            .then(|| format!("cannot read the {what} of window {window:#x}: {error}"))
    };
    // SAFETY: SetLastError only sets this thread's error value. GetWindowTextLengthW queries the
    // current EnumWindows handle and retains nothing.
    let length = unsafe {
        SetLastError(0);
        GetWindowTextLengthW(window)
    };
    if length <= 0 {
        return failed("title length").map_or(Ok(String::new()), Err);
    }
    let mut buffer = vec![0_u16; length as usize + 1];
    // SAFETY: `buffer` is writable for `length + 1` UTF-16 units, the capacity passed, and
    // stays alive while the copy is decoded.
    let copied = unsafe {
        SetLastError(0);
        GetWindowTextW(window, buffer.as_mut_ptr(), length + 1)
    };
    if copied <= 0 {
        return failed("title").map_or(Ok(String::new()), Err);
    }
    Ok(String::from_utf16_lossy(&buffer[..copied as usize]))
}

#[cfg(test)]
mod tests {
    use super::{OwnerSearch, complete};

    fn found(pids: &[u32], failure: Option<&str>) -> OwnerSearch {
        OwnerSearch {
            class: Vec::new(),
            pids: pids.to_vec(),
            failure: failure.map(str::to_owned),
        }
    }

    /// A failure after the target's window was found still refuses: the rest is unknown.
    #[test]
    fn only_a_complete_enumeration_lists_owners() {
        assert_eq!(complete(found(&[20, 10, 20], None), None), Ok(vec![10, 20]));
        assert!(complete(found(&[20, 10], Some("class unreadable")), None).is_err());
        assert!(complete(found(&[20, 10], None), Some("access denied".into())).is_err());
    }

    /// Serializes the tests that enumerate or create and destroy this process's windows: a
    /// window destroyed between the enumeration handing out its handle and the class or title
    /// read is, rightly, an enumeration error.
    static NATIVE_WINDOWS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn native_windows() -> std::sync::MutexGuard<'static, ()> {
        NATIVE_WINDOWS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Runs the real enumeration: it completes on any desktop, an empty list included.
    #[test]
    fn the_enumeration_completes() {
        let _windows = native_windows();
        super::tray_icon_owner_pids().unwrap();
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn CreateWindowExW(
            ex_style: u32,
            class: *const u16,
            title: *const u16,
            style: u32,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            parent: isize,
            menu: isize,
            instance: isize,
            parameter: *const std::ffi::c_void,
        ) -> isize;
        fn DestroyWindow(window: isize) -> i32;
    }

    /// A hidden top-level window of this process, made on the calling thread (so reading its
    /// title needs no message loop), destroyed when dropped.
    struct HiddenWindow(isize);

    impl HiddenWindow {
        fn titled(title: &str) -> Self {
            let class: Vec<u16> = "STATIC".encode_utf16().chain([0]).collect();
            let title: Vec<u16> = title.encode_utf16().chain([0]).collect();
            // SAFETY: both buffers are NUL-terminated and outlive the call; the system STATIC
            // class needs no registration; no WS_VISIBLE, no parent, menu or creation data.
            let window = unsafe {
                CreateWindowExW(
                    0,
                    class.as_ptr(),
                    title.as_ptr(),
                    0,
                    0,
                    0,
                    10,
                    10,
                    0,
                    0,
                    0,
                    std::ptr::null(),
                )
            };
            assert_ne!(window, 0, "{}", std::io::Error::last_os_error());
            Self(window)
        }
    }

    impl Drop for HiddenWindow {
        fn drop(&mut self) {
            // SAFETY: the window was created on this thread and is destroyed once.
            unsafe { DestroyWindow(self.0) };
        }
    }

    /// The positive case against a real window: without it, a search that always answers
    /// "no" would pass every other test.
    #[test]
    fn a_hidden_stickymd_titled_window_is_found_and_others_are_not() {
        // Held until every window below is destroyed (the guard drops last).
        let _windows = native_windows();
        let pid = std::process::id();
        let unrelated = HiddenWindow::titled("Clash Verge");
        let empty = HiddenWindow::titled("");
        assert_eq!(super::window_title(empty.0), Ok(String::new()));
        assert_eq!(
            super::window_title(unrelated.0),
            Ok("Clash Verge".to_owned())
        );
        assert_eq!(super::has_stickymd_window(pid), Ok(false));
        let stickymd = HiddenWindow::titled("StickyMD * — 外部修改冲突");
        assert_eq!(super::has_stickymd_window(pid), Ok(true));
        drop(stickymd);
        assert_eq!(super::has_stickymd_window(pid), Ok(false));
        // A destroyed handle is a failed read, not an empty title.
        let gone = unrelated.0;
        drop(unrelated);
        assert!(super::window_title(gone).is_err());
    }
}
