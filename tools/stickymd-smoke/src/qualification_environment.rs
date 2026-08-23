//! Fast, tooling-only qualification environment inspection.
//!
//! This adapter answers one question: can the current Windows session produce
//! meaningful GUI runtime, performance, resource, or manual evidence? It never
//! changes product state and never records window titles, user names, or paths.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QualificationEnvironmentStatus {
    Valid,
    EnvironmentBlocked,
    #[cfg_attr(
        windows,
        allow(
            dead_code,
            reason = "UNSUPPORTED is emitted only by the non-Windows qualification adapter"
        )
    )]
    Unsupported,
    Error,
}

impl QualificationEnvironmentStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Valid => "VALID",
            Self::EnvironmentBlocked => "ENVIRONMENT_BLOCKED",
            Self::Unsupported => "UNSUPPORTED",
            Self::Error => "ERROR",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QualificationEnvironment {
    pub(crate) status: QualificationEnvironmentStatus,
    pub(crate) interactive_session: bool,
    pub(crate) input_desktop_usable: bool,
    pub(crate) workstation_locked: bool,
    pub(crate) interactive_shell_present: bool,
    pub(crate) foreground_available: bool,
    pub(crate) display_count: u32,
    pub(crate) detail: Option<String>,
}

impl QualificationEnvironment {
    pub(crate) fn summary(&self) -> String {
        let detail = self
            .detail
            .as_deref()
            .map(|value| format!("; detail={value}"))
            .unwrap_or_default();
        format!(
            "status={}; interactive={}; desktop_usable={}; locked={}; shell={}; foreground={}; displays={}{}",
            self.status.as_str(),
            self.interactive_session,
            self.input_desktop_usable,
            self.workstation_locked,
            self.interactive_shell_present,
            self.foreground_available,
            self.display_count,
            detail,
        )
    }
}

#[cfg(windows)]
pub(crate) fn inspect() -> QualificationEnvironment {
    match windows::inspect() {
        Ok(facts) => classify(facts),
        Err(detail) => QualificationEnvironment {
            status: QualificationEnvironmentStatus::Error,
            interactive_session: false,
            input_desktop_usable: false,
            workstation_locked: false,
            interactive_shell_present: false,
            foreground_available: false,
            display_count: 0,
            detail: Some(detail),
        },
    }
}

#[cfg(not(windows))]
pub(crate) fn inspect() -> QualificationEnvironment {
    QualificationEnvironment {
        status: QualificationEnvironmentStatus::Unsupported,
        interactive_session: false,
        input_desktop_usable: false,
        workstation_locked: false,
        interactive_shell_present: false,
        foreground_available: false,
        display_count: 0,
        detail: Some("GUI qualification requires Windows 11".to_owned()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EnvironmentFacts {
    session_active: bool,
    session_unlocked: bool,
    input_desktop_usable: bool,
    interactive_shell_present: bool,
    foreground_available: bool,
    display_count: u32,
}

fn classify(facts: EnvironmentFacts) -> QualificationEnvironment {
    let interactive_session = facts.session_active && facts.display_count > 0;
    let workstation_locked = !facts.session_unlocked;
    let valid = interactive_session
        && facts.input_desktop_usable
        && !workstation_locked
        && facts.interactive_shell_present
        && facts.foreground_available;
    QualificationEnvironment {
        status: if valid {
            QualificationEnvironmentStatus::Valid
        } else {
            QualificationEnvironmentStatus::EnvironmentBlocked
        },
        interactive_session,
        input_desktop_usable: facts.input_desktop_usable,
        workstation_locked,
        interactive_shell_present: facts.interactive_shell_present,
        foreground_available: facts.foreground_available,
        display_count: facts.display_count,
        detail: (!valid).then(|| {
            "current session cannot provide valid interactive GUI qualification evidence".to_owned()
        }),
    }
}

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;
    use std::mem;
    use std::ptr;

    use super::EnvironmentFacts;

    type Bool = i32;
    type Dword = u32;
    type KernelHandle = isize;
    type Hdesk = *mut c_void;
    type Hwnd = *mut c_void;

    const FALSE: Bool = 0;
    const TH32CS_SNAPPROCESS: Dword = 0x0000_0002;
    const INVALID_HANDLE_VALUE: KernelHandle = -1;
    const DESKTOP_READOBJECTS: Dword = 0x0000_0001;
    const DESKTOP_SWITCHDESKTOP: Dword = 0x0000_0100;
    const WTS_SESSION_INFO_EX: Dword = 25;
    const WTS_ACTIVE: i32 = 0;
    const WTS_SESSIONSTATE_UNLOCK: i32 = 1;
    const SM_CMONITORS: i32 = 80;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct WtsInfoExLevel1Head {
        session_id: Dword,
        session_state: i32,
        session_flags: i32,
    }

    #[repr(C)]
    union WtsInfoExDataHead {
        level1: WtsInfoExLevel1Head,
        alignment: u64,
    }

    #[repr(C)]
    struct WtsInfoExHead {
        level: Dword,
        data: WtsInfoExDataHead,
    }

    #[repr(C)]
    struct ProcessEntry32W {
        size: Dword,
        usage: Dword,
        process_id: Dword,
        default_heap_id: usize,
        module_id: Dword,
        threads: Dword,
        parent_process_id: Dword,
        base_priority: i32,
        flags: Dword,
        executable: [u16; 260],
    }

    #[link(name = "Wtsapi32")]
    unsafe extern "system" {
        fn WTSQuerySessionInformationW(
            server: *mut c_void,
            session_id: Dword,
            info_class: Dword,
            buffer: *mut *mut u16,
            bytes_returned: *mut Dword,
        ) -> Bool;
        fn WTSFreeMemory(memory: *mut c_void);
    }

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn ProcessIdToSessionId(process_id: Dword, session_id: *mut Dword) -> Bool;
        fn CreateToolhelp32Snapshot(flags: Dword, process_id: Dword) -> KernelHandle;
        fn Process32FirstW(snapshot: KernelHandle, entry: *mut ProcessEntry32W) -> Bool;
        fn Process32NextW(snapshot: KernelHandle, entry: *mut ProcessEntry32W) -> Bool;
        fn CloseHandle(handle: KernelHandle) -> Bool;
    }

    #[link(name = "User32")]
    unsafe extern "system" {
        fn OpenInputDesktop(flags: Dword, inherit: Bool, desired_access: Dword) -> Hdesk;
        fn CloseDesktop(desktop: Hdesk) -> Bool;
        fn GetForegroundWindow() -> Hwnd;
        fn GetSystemMetrics(index: i32) -> i32;
    }

    struct OwnedWtsMemory(*mut c_void);

    impl Drop for OwnedWtsMemory {
        fn drop(&mut self) {
            // SAFETY: WTSQuerySessionInformationW allocated this non-null buffer and
            // transfers exactly one WTSFreeMemory obligation to the caller.
            unsafe { WTSFreeMemory(self.0) };
        }
    }

    struct OwnedHandle(KernelHandle);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // SAFETY: the handle came from CreateToolhelp32Snapshot, is not
            // INVALID_HANDLE_VALUE, and this guard is its sole owner.
            unsafe { CloseHandle(self.0) };
        }
    }

    pub(super) fn inspect() -> Result<EnvironmentFacts, String> {
        let session_id = current_session_id()?;
        let (session_state, session_flags) = session_information(session_id)?;
        let input_desktop_usable = input_desktop_usable();
        let interactive_shell_present = process_exists_in_session("explorer.exe", session_id)?;
        // SAFETY: GetForegroundWindow and GetSystemMetrics take no pointers and
        // return process-independent snapshots of the current interactive session.
        let (foreground_available, display_count) = unsafe {
            (
                !GetForegroundWindow().is_null(),
                u32::try_from(GetSystemMetrics(SM_CMONITORS).max(0)).unwrap_or(0),
            )
        };
        Ok(EnvironmentFacts {
            session_active: session_state == WTS_ACTIVE,
            session_unlocked: session_flags == WTS_SESSIONSTATE_UNLOCK,
            input_desktop_usable,
            interactive_shell_present,
            foreground_available,
            display_count,
        })
    }

    fn current_session_id() -> Result<Dword, String> {
        let mut session_id = 0;
        // SAFETY: session_id is a valid out pointer for the duration of the call;
        // std::process::id returns the current live process identifier.
        let succeeded = unsafe { ProcessIdToSessionId(std::process::id(), &mut session_id) };
        if succeeded == FALSE {
            Err("ProcessIdToSessionId failed".to_owned())
        } else {
            Ok(session_id)
        }
    }

    fn session_information(session_id: Dword) -> Result<(i32, i32), String> {
        let mut buffer = ptr::null_mut();
        let mut bytes_returned = 0;
        // SAFETY: buffer and bytes_returned are valid out pointers. A null server
        // selects the local server, and session_id belongs to the current process.
        let succeeded = unsafe {
            WTSQuerySessionInformationW(
                ptr::null_mut(),
                session_id,
                WTS_SESSION_INFO_EX,
                &mut buffer,
                &mut bytes_returned,
            )
        };
        if succeeded == FALSE || buffer.is_null() {
            return Err("WTSQuerySessionInformationW(WTSSessionInfoEx) failed".to_owned());
        }
        let memory = OwnedWtsMemory(buffer.cast());
        if (bytes_returned as usize) < mem::size_of::<WtsInfoExHead>() {
            return Err("WTSSessionInfoEx returned a truncated buffer".to_owned());
        }
        // SAFETY: the successful API call returned at least WtsInfoExHead bytes;
        // WTSSessionInfoEx level 1 stores WTSINFOEX_LEVEL1 in the active union arm.
        let info = unsafe { &*memory.0.cast::<WtsInfoExHead>() };
        if info.level != 1 {
            return Err(format!("unsupported WTSSessionInfoEx level {}", info.level));
        }
        // SAFETY: level == 1 establishes that level1 is the initialized union arm.
        let level1 = unsafe { info.data.level1 };
        if level1.session_id != session_id {
            return Err("WTSSessionInfoEx returned a different session".to_owned());
        }
        Ok((level1.session_state, level1.session_flags))
    }

    fn input_desktop_usable() -> bool {
        // SAFETY: no borrowed pointers are involved; the returned desktop handle is
        // owned by this function and closed exactly once below when non-null.
        let desktop =
            unsafe { OpenInputDesktop(0, FALSE, DESKTOP_READOBJECTS | DESKTOP_SWITCHDESKTOP) };
        if desktop.is_null() {
            return false;
        }
        // SAFETY: desktop is a valid handle returned by OpenInputDesktop and has
        // not been closed yet.
        unsafe { CloseDesktop(desktop) };
        true
    }

    fn process_exists_in_session(name: &str, expected_session: Dword) -> Result<bool, String> {
        // SAFETY: CreateToolhelp32Snapshot takes value arguments and returns an
        // owned snapshot handle on success.
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err("CreateToolhelp32Snapshot failed".to_owned());
        }
        let snapshot = OwnedHandle(snapshot);
        // SAFETY: ProcessEntry32W is a plain C record; zero is a valid initial
        // state and the documented size field is set before enumeration.
        let mut entry: ProcessEntry32W = unsafe { mem::zeroed() };
        entry.size = u32::try_from(mem::size_of::<ProcessEntry32W>())
            .map_err(|_| "PROCESSENTRY32W size overflow".to_owned())?;
        // SAFETY: snapshot is valid and entry points to writable storage with the
        // documented size field initialized.
        let mut available = unsafe { Process32FirstW(snapshot.0, &mut entry) } != FALSE;
        while available {
            if executable_name(&entry.executable).eq_ignore_ascii_case(name) {
                let mut session_id = 0;
                // SAFETY: session_id is a valid out pointer and process_id came
                // from the live Toolhelp snapshot.
                let mapped = unsafe { ProcessIdToSessionId(entry.process_id, &mut session_id) };
                if mapped != FALSE && session_id == expected_session {
                    return Ok(true);
                }
            }
            // SAFETY: snapshot and entry remain valid across the enumeration loop.
            available = unsafe { Process32NextW(snapshot.0, &mut entry) } != FALSE;
        }
        Ok(false)
    }

    fn executable_name(buffer: &[u16]) -> String {
        let length = buffer
            .iter()
            .position(|character| *character == 0)
            .unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..length])
    }
}

#[cfg(test)]
mod tests {
    use super::{EnvironmentFacts, QualificationEnvironmentStatus, classify};

    fn valid_facts() -> EnvironmentFacts {
        EnvironmentFacts {
            session_active: true,
            session_unlocked: true,
            input_desktop_usable: true,
            interactive_shell_present: true,
            foreground_available: true,
            display_count: 1,
        }
    }

    #[test]
    fn all_interactive_facts_are_required_for_valid_evidence() {
        assert_eq!(
            classify(valid_facts()).status,
            QualificationEnvironmentStatus::Valid
        );

        let mut locked = valid_facts();
        locked.session_unlocked = false;
        let blocked = classify(locked);
        assert_eq!(
            blocked.status,
            QualificationEnvironmentStatus::EnvironmentBlocked
        );
        assert!(blocked.workstation_locked);
    }

    #[test]
    fn missing_shell_or_display_blocks_instead_of_passing() {
        let mut no_shell = valid_facts();
        no_shell.interactive_shell_present = false;
        assert_eq!(
            classify(no_shell).status,
            QualificationEnvironmentStatus::EnvironmentBlocked
        );

        let mut no_display = valid_facts();
        no_display.display_count = 0;
        assert_eq!(
            classify(no_display).status,
            QualificationEnvironmentStatus::EnvironmentBlocked
        );
    }
}
