//! Collects the tested machine's `Facts` for role matching: `DisplayVersion`,
//! `CurrentBuild` and `EditionID` (`REG_SZ`) and `UBR` (`REG_DWORD`) from the 64-bit view
//! of `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`, and the native machine that
//! `IsWow64Process2` reports. A wrong value type, a failed read, a value longer than its
//! buffer or a capture past its deadline is an error; nothing is guessed or read from `ver`.

#[cfg(windows)]
use std::sync::mpsc;
#[cfg(windows)]
use std::time::Duration;

use super::role::Facts;
#[cfg(any(windows, test))]
use super::target::Machine;

/// Registry reads and one kernel query take milliseconds; past this the capture failed.
#[cfg(windows)]
const DEADLINE: Duration = Duration::from_secs(10);

/// Captures the facts on a worker thread, so a read that never returns becomes an error
/// at the deadline instead of a hang (the stuck thread ends with the process).
#[cfg(windows)]
pub(in crate::qualification) fn capture() -> Result<Facts, String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("manual-environment".to_owned())
        .spawn(move || {
            // The receiver is gone only after the deadline; the late result is moot.
            let _ = sender.send(read());
        })
        .map_err(|error| format!("cannot start the environment capture: {error}"))?;
    receiver.recv_timeout(DEADLINE).unwrap_or_else(|_| {
        Err(format!(
            "the environment capture did not finish within {} s",
            DEADLINE.as_secs()
        ))
    })
}

/// The capture when an answer is committed must equal the one at scenario start.
pub(in crate::qualification) fn unchanged(start: &Facts, end: &Facts) -> Result<(), String> {
    if start == end {
        Ok(())
    } else {
        Err(format!(
            "the environment changed during the scenario: Windows {} build {} {} {} at start, Windows {} build {} {} {} at commit",
            start.display_version,
            start.build(),
            start.edition_id,
            start.native_machine.as_str(),
            end.display_version,
            end.build(),
            end.edition_id,
            end.native_machine.as_str(),
        ))
    }
}

/// `CurrentBuild` is a decimal string such as `26200`.
#[cfg(any(windows, test))]
fn parse_build(text: &str) -> Result<u32, String> {
    if text.is_empty() || text.len() > 9 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!(
            "CurrentBuild `{text}` is not a decimal build number"
        ));
    }
    text.parse()
        .map_err(|error| format!("CurrentBuild `{text}`: {error}"))
}

/// `IMAGE_FILE_MACHINE_*` values from `IsWow64Process2`'s `pNativeMachine`.
#[cfg(any(windows, test))]
fn machine(code: u16) -> Result<Machine, String> {
    match code {
        0x8664 => Ok(Machine::Amd64),
        0xAA64 => Ok(Machine::Arm64),
        _ => Err(format!(
            "native machine 0x{code:04X} is neither AMD64 nor ARM64"
        )),
    }
}

#[cfg(windows)]
fn read() -> Result<Facts, String> {
    Ok(Facts {
        display_version: windows::string(windows::SOURCE, "DisplayVersion")?,
        current_build: parse_build(&windows::string(windows::SOURCE, "CurrentBuild")?)?,
        ubr: windows::dword(windows::SOURCE, "UBR")?,
        edition_id: windows::string(windows::SOURCE, "EditionID")?,
        native_machine: machine(windows::native_machine()?)?,
    })
}

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;
    use std::io;

    /// `HKEY_LOCAL_MACHINE` is `(HKEY)(ULONG_PTR)(LONG)0x80000002`: sign-extended.
    const HKEY_LOCAL_MACHINE: isize = 0x8000_0002_u32 as i32 as isize;
    const CURRENT_VERSION: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    const RRF_RT_REG_SZ: u32 = 0x0000_0002;
    const RRF_RT_REG_DWORD: u32 = 0x0000_0010;
    const RRF_SUBKEY_WOW6464KEY: u32 = 0x0001_0000;
    /// Without it a `REG_EXPAND_SZ` is expanded and reported as `REG_SZ`.
    const RRF_NOEXPAND: u32 = 0x1000_0000;
    const REG_SZ: u32 = 1;
    const REG_DWORD: u32 = 4;
    const ERROR_SUCCESS: i32 = 0;
    const ERROR_MORE_DATA: i32 = 234;
    const ERROR_UNSUPPORTED_TYPE: i32 = 1630;
    /// Longest accepted string value in UTF-16 units, terminator included; the log allows
    /// at most 64 bytes of `EditionID`.
    const MAX_STRING_UNITS: usize = 65;

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(
            key: isize,
            subkey: *const u16,
            value: *const u16,
            flags: u32,
            kind: *mut u32,
            data: *mut c_void,
            size: *mut u32,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> isize;
        fn IsWow64Process2(
            process: isize,
            process_machine: *mut u16,
            native_machine: *mut u16,
        ) -> i32;
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain([0]).collect()
    }

    /// Where the facts live: `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`.
    pub(super) const SOURCE: Key = Key {
        root: HKEY_LOCAL_MACHINE,
        subkey: CURRENT_VERSION,
    };

    #[derive(Clone, Copy)]
    pub(super) struct Key {
        pub(super) root: isize,
        pub(super) subkey: &'static str,
    }

    /// Reads `name` with the type restricted by `flags` into `data` (`size` bytes).
    /// Returns the bytes written and the reported type.
    fn get(
        key: Key,
        name: &str,
        flags: u32,
        data: *mut c_void,
        size: u32,
    ) -> Result<(u32, u32), String> {
        let subkey = wide(key.subkey);
        let value = wide(name);
        let mut kind = 0_u32;
        let mut written = size;
        // SAFETY: `subkey` and `value` are NUL-terminated UTF-16 that outlive the call;
        // `data` is writable for `size` bytes (callers pass their own buffer and its
        // length); `kind` and `written` are valid out-pointers.
        let status = unsafe {
            RegGetValueW(
                key.root,
                subkey.as_ptr(),
                value.as_ptr(),
                flags | RRF_SUBKEY_WOW6464KEY,
                &raw mut kind,
                data,
                &raw mut written,
            )
        };
        match status {
            ERROR_SUCCESS => Ok((written, kind)),
            ERROR_MORE_DATA => Err(format!("registry value {name} is longer than accepted")),
            ERROR_UNSUPPORTED_TYPE => Err(format!("registry value {name} has the wrong type")),
            code => Err(format!(
                "cannot read registry value {name}: {}",
                io::Error::from_raw_os_error(code)
            )),
        }
    }

    pub(super) fn string(key: Key, name: &str) -> Result<String, String> {
        let mut buffer = [0_u16; MAX_STRING_UNITS];
        let (written, kind) = get(
            key,
            name,
            RRF_RT_REG_SZ | RRF_NOEXPAND,
            buffer.as_mut_ptr().cast(),
            size_of_val(&buffer) as u32,
        )?;
        if kind != REG_SZ {
            return Err(format!("registry value {name} is not REG_SZ"));
        }
        // `written` counts bytes including the terminator RegGetValueW guarantees.
        let units = &buffer[..(written as usize / 2).min(buffer.len())];
        let text = units.strip_suffix(&[0]).unwrap_or(units);
        if text.is_empty() || text.contains(&0) {
            return Err(format!("registry value {name} is empty or holds a NUL"));
        }
        String::from_utf16(text).map_err(|error| format!("registry value {name}: {error}"))
    }

    pub(super) fn dword(key: Key, name: &str) -> Result<u32, String> {
        let mut value = 0_u32;
        let (written, kind) = get(
            key,
            name,
            RRF_RT_REG_DWORD,
            (&raw mut value).cast(),
            size_of::<u32>() as u32,
        )?;
        if kind != REG_DWORD || written != 4 {
            return Err(format!("registry value {name} is not a REG_DWORD"));
        }
        Ok(value)
    }

    pub(super) fn native_machine() -> Result<u16, String> {
        let mut process = 0_u16;
        let mut native = 0_u16;
        // SAFETY: GetCurrentProcess returns a pseudo-handle that needs no closing; both
        // out-pointers are valid u16 storage for the duration of the call.
        let succeeded =
            unsafe { IsWow64Process2(GetCurrentProcess(), &raw mut process, &raw mut native) };
        if succeeded == 0 {
            return Err(format!(
                "IsWow64Process2 failed: {}",
                io::Error::last_os_error()
            ));
        }
        Ok(native)
    }
}

#[cfg(test)]
mod tests {
    use super::{machine, parse_build, unchanged};
    use crate::qualification::manual_observation::role::fixtures::current;
    use crate::qualification::manual_observation::target::Machine;

    #[test]
    fn build_numbers_and_machines_are_read_strictly() {
        assert_eq!(parse_build("26200"), Ok(26200));
        for text in ["", " 26200", "26200.1", "0x6698", "+26200", "9999999999"] {
            assert!(parse_build(text).is_err(), "{text}");
        }
        assert_eq!(machine(0x8664), Ok(Machine::Amd64));
        assert_eq!(machine(0xAA64), Ok(Machine::Arm64));
        // x86 and an unknown machine are refused, not mapped to the nearest one.
        for code in [0x014C, 0x0000, 0x01C4] {
            assert!(machine(code).is_err(), "{code:#x}");
        }
    }

    #[test]
    fn a_changed_environment_is_refused() {
        assert_eq!(unchanged(&current(), &current()), Ok(()));
        let mut updated = current();
        updated.ubr += 1;
        let error = unchanged(&current(), &updated).unwrap_err();
        assert!(
            error.contains("26200.7019") && error.contains("26200.7020"),
            "{error}"
        );
    }

    /// Wrong types, an expandable string, an over-long or empty value and a missing value
    /// are refused by the same reads, against a scratch key under `HKCU`.
    #[cfg(windows)]
    #[test]
    fn registry_reads_refuse_wrong_types_and_lengths() {
        use super::windows::{Key, dword, string};
        let subkey = format!(
            r"Software\StickyMD-smoke-environment-{}",
            std::process::id()
        );
        let path = format!(r"HKCU\{subkey}");
        let add = |name: &str, kind: &str, data: &str| {
            let status = std::process::Command::new("reg")
                .args(["add", &path, "/v", name, "/t", kind, "/d", data, "/f"])
                .output()
                .unwrap()
                .status;
            assert!(status.success(), "reg add {name}");
        };
        add("Text", "REG_SZ", "25H2");
        add("Expand", "REG_EXPAND_SZ", "%SystemRoot%");
        add("Fits", "REG_SZ", &"x".repeat(64));
        add("Long", "REG_SZ", &"x".repeat(65));
        add("Empty", "REG_SZ", "");
        add("Number", "REG_DWORD", "7019");
        add("Binary", "REG_BINARY", "7B1B0000");
        add("NumberText", "REG_SZ", "7019");
        let key = Key {
            // HKEY_CURRENT_USER, sign-extended like HKEY_LOCAL_MACHINE.
            root: 0x8000_0001_u32 as i32 as isize,
            subkey: Box::leak(subkey.into_boxed_str()),
        };
        let outcome = std::panic::catch_unwind(|| {
            assert_eq!(string(key, "Text").as_deref(), Ok("25H2"));
            assert_eq!(string(key, "Fits").map(|text| text.len()), Ok(64));
            assert_eq!(dword(key, "Number"), Ok(7019));
            for name in ["Expand", "Long", "Empty", "Number", "Missing"] {
                assert!(string(key, name).is_err(), "{name}");
            }
            for name in ["Binary", "NumberText", "Missing"] {
                assert!(dword(key, name).is_err(), "{name}");
            }
        });
        let _ = std::process::Command::new("reg")
            .args(["delete", &path, "/f"])
            .output();
        if let Err(panic) = outcome {
            std::panic::resume_unwind(panic);
        }
    }

    /// The registry facts agree with the build `ver` reports on this host.
    #[cfg(windows)]
    #[test]
    fn this_host_reports_its_build_from_the_registry() {
        let facts = super::capture().unwrap();
        let reported = crate::qualification::windows_build::current();
        assert!(
            reported.contains(&format!(".{}.{}", facts.current_build, facts.ubr)),
            "{facts:?} vs {reported}"
        );
        assert!(!facts.display_version.is_empty() && !facts.edition_id.is_empty());
    }
}
