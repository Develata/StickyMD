//! Windows named-event owner for editor-ready startup measurements.

use std::ffi::c_void;
use std::time::Duration;

const WAIT_OBJECT_0: u32 = 0;
const WAIT_TIMEOUT: u32 = 258;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(
        attributes: *const c_void,
        manual_reset: i32,
        initial_state: i32,
        name: *const u16,
    ) -> isize;
    fn WaitForSingleObject(handle: isize, milliseconds: u32) -> u32;
    fn CloseHandle(handle: isize) -> i32;
}

pub(crate) struct ReadyEvent {
    handle: isize,
    name: String,
}

impl ReadyEvent {
    pub(crate) fn create(sequence: u64) -> Result<Self, String> {
        let name = format!(
            "Local\\StickyMD.Smoke.EditorReady.{}.{}",
            std::process::id(),
            sequence
        );
        let wide = name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        // SAFETY: `wide` is NUL-terminated and remains live for the call;
        // default security attributes create an auto-reset, initially clear event.
        let handle = unsafe { CreateEventW(std::ptr::null(), 0, 0, wide.as_ptr()) };
        if handle == 0 {
            return Err(format!(
                "CreateEventW failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(Self { handle, name })
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn wait(&self, timeout: Duration) -> Result<(), String> {
        let milliseconds = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
        // SAFETY: `handle` is owned by this object and remains live for the wait.
        match unsafe { WaitForSingleObject(self.handle, milliseconds) } {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_TIMEOUT => Err(format!("editor-ready event timed out after {timeout:?}")),
            code => Err(format!(
                "WaitForSingleObject returned {code}: {}",
                std::io::Error::last_os_error()
            )),
        }
    }
}

impl Drop for ReadyEvent {
    fn drop(&mut self) {
        // SAFETY: this object uniquely owns the non-zero event handle.
        let _ = unsafe { CloseHandle(self.handle) };
    }
}

#[cfg(test)]
mod tests {
    use super::ReadyEvent;

    #[test]
    fn phase9_ready_event_name_is_process_scoped_and_local() {
        let event = ReadyEvent::create(7).expect("create named event");
        assert!(
            event
                .name()
                .starts_with("Local\\StickyMD.Smoke.EditorReady.")
        );
        assert!(event.name().ends_with(".7"));
    }
}
