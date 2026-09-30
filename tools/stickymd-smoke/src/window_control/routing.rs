//! Privacy-bounded facts captured at the time a physical input route fails.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::GetWindowThreadProcessId;

#[link(name = "user32")]
unsafe extern "system" {
    fn GetClassNameW(window: isize, buffer: *mut u16, capacity: i32) -> i32;
}

pub(super) fn describe(window: isize) -> String {
    let mut pid = 0;
    let mut class = [0_u16; 256];
    // SAFETY: these read-only queries copy into bounded local storage. The HWND
    // is borrowed and may already be gone; failure remains explicit unknown data.
    let length = unsafe {
        GetWindowThreadProcessId(window, &raw mut pid);
        GetClassNameW(window, class.as_mut_ptr(), class.len() as i32)
    };
    let class = usize::try_from(length)
        .ok()
        .and_then(|length| class.get(..length))
        .filter(|value| !value.is_empty())
        .map(String::from_utf16_lossy);
    let image = crate::managed_process::process_executable(pid).and_then(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    });
    format!(
        "observed_pid={pid} observed_executable={:?} observed_class={:?}",
        image.as_deref().unwrap_or("UNKNOWN"),
        class.as_deref().unwrap_or("UNKNOWN")
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn vanished_window_diagnostics_are_explicit_and_have_no_path_or_title() {
        let facts = super::describe(0);
        assert_eq!(
            facts,
            "observed_pid=0 observed_executable=\"UNKNOWN\" observed_class=\"UNKNOWN\""
        );
    }
}
