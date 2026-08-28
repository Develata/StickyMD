//! Test-only TSF input-profile activation with explicit restoration.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::ffi::c_void;
use std::ptr;
use std::thread;
use std::time::{Duration, Instant};

use super::WindowHandle;

const S_OK: i32 = 0;
const COINIT_APARTMENTTHREADED: u32 = 0x2;
const CLSCTX_INPROC_SERVER: u32 = 0x1;
const TF_PROFILETYPE_INPUTPROCESSOR: u32 = 1;
const TF_IPPMF_FORSESSION: u32 = 0x2000_0000;
const TF_IPP_FLAG_ENABLED: u32 = 0x2;
const WM_INPUTLANGCHANGEREQUEST: u32 = 0x0050;
const WM_IME_CONTROL: u32 = 0x0283;
const IMC_GETCONVERSIONMODE: usize = 0x0001;
const IMC_SETCONVERSIONMODE: usize = 0x0002;
const IMC_GETOPENSTATUS: usize = 0x0005;
const IMC_SETOPENSTATUS: usize = 0x0006;
const IME_CMODE_NATIVE: isize = 0x0001;
const PROFILE_TIMEOUT: Duration = Duration::from_secs(3);

const CLSID_TF_INPUT_PROCESSOR_PROFILES: Guid = Guid::new(
    0x33C5_3A50,
    0xF456,
    0x4884,
    [0xB0, 0x49, 0x85, 0xFD, 0x64, 0x3E, 0xCF, 0xED],
);
const IID_INPUT_PROCESSOR_PROFILE_MGR: Guid = Guid::new(
    0x71C6_E74C,
    0x0F28,
    0x11D8,
    [0xA8, 0x2A, 0x00, 0x06, 0x5B, 0x84, 0x43, 0x5C],
);
const GUID_TFCAT_TIP_KEYBOARD: Guid = Guid::new(
    0x3474_5C63,
    0xB2F0,
    0x4784,
    [0x8B, 0x67, 0x5E, 0x12, 0xC8, 0x70, 0x1A, 0x31],
);

const MICROSOFT_PINYIN: ProfileSpec = ProfileSpec {
    name: "Microsoft Pinyin",
    class: Guid::new(
        0x81D4_E9C9,
        0x1D3B,
        0x41BC,
        [0x9E, 0x6C, 0x4B, 0x40, 0xBF, 0x79, 0xE3, 0x5E],
    ),
    profile: Guid::new(
        0xFA55_0B04,
        0x5AD7,
        0x411F,
        [0xA5, 0xAC, 0xCA, 0x03, 0x8E, 0xC5, 0x15, 0xD7],
    ),
};

const WETYPE: ProfileSpec = ProfileSpec {
    name: "WeType",
    class: Guid::new(
        0x8659_8FB9,
        0x66A2,
        0x463E,
        [0xB9, 0xC2, 0xAE, 0xB9, 0x06, 0xD4, 0x77, 0xAD],
    ),
    profile: Guid::new(
        0x607F_DF85,
        0xFCC8,
        0x4DBD,
        [0xA3, 0x65, 0x41, 0x29, 0x6F, 0x98, 0x0C, 0x9C],
    ),
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImeProfile {
    MicrosoftPinyin,
    WeType,
}

impl ImeProfile {
    pub(crate) const fn name(self) -> &'static str {
        self.spec().name
    }

    const fn spec(self) -> ProfileSpec {
        match self {
            Self::MicrosoftPinyin => MICROSOFT_PINYIN,
            Self::WeType => WETYPE,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

impl Guid {
    const fn new(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Self {
        Self {
            data1,
            data2,
            data3,
            data4,
        }
    }
}

#[derive(Clone, Copy)]
struct ProfileSpec {
    name: &'static str,
    class: Guid,
    profile: Guid,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct NativeProfile {
    profile_type: u32,
    language: u16,
    class: Guid,
    profile: Guid,
    category: Guid,
    substitute_layout: isize,
    capabilities: u32,
    keyboard_layout: isize,
    flags: u32,
}

#[repr(C)]
struct UnknownVTable {
    query_interface: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
}

#[repr(C)]
struct ProfileManagerVTable {
    base: UnknownVTable,
    activate_profile: unsafe extern "system" fn(
        *mut c_void,
        u32,
        u16,
        *const Guid,
        *const Guid,
        isize,
        u32,
    ) -> i32,
    deactivate_profile: usize,
    get_profile: unsafe extern "system" fn(
        *mut c_void,
        u32,
        u16,
        *const Guid,
        *const Guid,
        isize,
        *mut NativeProfile,
    ) -> i32,
    enum_profiles: usize,
    release_input_processor: usize,
    register_profile: usize,
    unregister_profile: usize,
    get_active_profile:
        unsafe extern "system" fn(*mut c_void, *const Guid, *mut NativeProfile) -> i32,
}

#[repr(C)]
struct ProfileManager {
    vtable: *const ProfileManagerVTable,
}

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoInitializeEx(reserved: *const c_void, concurrency: u32) -> i32;
    fn CoUninitialize();
    fn CoCreateInstance(
        class: *const Guid,
        outer: *mut c_void,
        context: u32,
        interface: *const Guid,
        result: *mut *mut c_void,
    ) -> i32;
}

#[link(name = "imm32")]
unsafe extern "system" {
    fn ImmGetDefaultIMEWnd(window: isize) -> isize;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetKeyboardLayout(thread_id: u32) -> isize;
    fn GetWindowThreadProcessId(window: isize, process_id: *mut u32) -> u32;
    fn PostMessageW(window: isize, message: u32, wparam: usize, lparam: isize) -> i32;
    fn SendMessageW(window: isize, message: u32, wparam: usize, lparam: isize) -> isize;
}

pub(crate) fn set_ime_open_status(window: WindowHandle, open: bool) -> Result<(), String> {
    let ime_window = default_ime_window(window)?;
    send_ime_control(ime_window, IMC_SETOPENSTATUS, isize::from(open))?;
    let observed = send_ime_query(ime_window, IMC_GETOPENSTATUS) != 0;
    if observed != open {
        return Err(format!(
            "StickyMD IME open status acknowledgement mismatch: requested={open} observed={observed}"
        ));
    }
    Ok(())
}

pub(crate) fn set_ime_native_mode(window: WindowHandle, native: bool) -> Result<(), String> {
    let ime_window = default_ime_window(window)?;
    let mode = if native { IME_CMODE_NATIVE } else { 0 };
    send_ime_control(ime_window, IMC_SETCONVERSIONMODE, mode)?;
    let observed = send_ime_query(ime_window, IMC_GETCONVERSIONMODE);
    if (observed & IME_CMODE_NATIVE != 0) != native {
        return Err(format!(
            "StickyMD IME conversion acknowledgement mismatch: requested_native={native} observed_mode=0x{observed:x}"
        ));
    }
    Ok(())
}

fn default_ime_window(window: WindowHandle) -> Result<isize, String> {
    // SAFETY: WindowHandle contains the borrowed live StickyMD HWND selected by
    // exact-candidate process ownership. The returned default IME HWND is also
    // borrowed and exists specifically to receive WM_IME_CONTROL.
    let ime_window = unsafe { ImmGetDefaultIMEWnd(window.0) };
    if ime_window == 0 {
        return Err(format!(
            "StickyMD thread has no default IME window: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(ime_window)
}

fn send_ime_control(ime_window: isize, command: usize, value: isize) -> Result<(), String> {
    // SAFETY: the borrowed default IME HWND belongs to the candidate thread;
    // scalar command parameters contain no pointers and SendMessageW returns
    // only after the IME window has processed the request.
    let result = unsafe { SendMessageW(ime_window, WM_IME_CONTROL, command, value) };
    if result != 0 {
        return Err(format!(
            "default IME window rejected command 0x{command:x} value=0x{value:x}: result={result}"
        ));
    }
    Ok(())
}

fn send_ime_query(ime_window: isize, command: usize) -> isize {
    // SAFETY: the same borrowed IME HWND remains live and the GET command has
    // no pointer-bearing parameter.
    unsafe { SendMessageW(ime_window, WM_IME_CONTROL, command, 0) }
}

fn route_input_language(
    window: WindowHandle,
    profile: &NativeProfile,
    label: &str,
) -> Result<(), String> {
    let thread_id = window_thread_id(window)?;
    let before = keyboard_layout(thread_id);
    if profile.substitute_layout == 0 {
        return if input_language(before) == profile.language {
            Ok(())
        } else {
            Err(format!(
                "cannot route {label} to StickyMD: profile language=0x{:04x}, current layout=0x{before:x}, and TSF exposed no substitute layout",
                profile.language
            ))
        };
    }

    // A matching LANGID is insufficient: the target thread can still be bound
    // to a different TIP for the same language. Always post the substitute HKL
    // after TSF profile activation so the already-running candidate refreshes
    // its text-service binding.
    // SAFETY: the exact-candidate HWND is live and focused. The message copies
    // the scalar substitute HKL returned by TSF and retains no caller-owned
    // memory. DefWindowProc performs the target-thread locale transition.
    if unsafe {
        PostMessageW(
            window.0,
            WM_INPUTLANGCHANGEREQUEST,
            0,
            profile.substitute_layout,
        )
    } == 0
    {
        return Err(format!(
            "cannot post {label} input-language request to StickyMD: {}",
            std::io::Error::last_os_error()
        ));
    }

    // WM_INPUTLANGCHANGEREQUEST is explicitly a posted message. When the
    // target already has the same LANGID, GetKeyboardLayout cannot distinguish
    // the old TIP binding from the newly requested one, so allow the candidate
    // thread one narrow dispatch interval before acknowledgement polling.
    thread::sleep(Duration::from_millis(50));

    let deadline = Instant::now() + PROFILE_TIMEOUT;
    loop {
        let observed = keyboard_layout(thread_id);
        if input_language(observed) == profile.language {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "StickyMD did not acknowledge {label} input language: requested_lang=0x{:04x} substitute_layout=0x{:x} observed_layout=0x{observed:x}",
                profile.language, profile.substitute_layout
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn window_thread_id(window: WindowHandle) -> Result<u32, String> {
    let mut process_id = 0_u32;
    // SAFETY: the exact-candidate HWND is borrowed and `process_id` is valid
    // writable stack storage. The API retains no pointer.
    let thread_id = unsafe { GetWindowThreadProcessId(window.0, &raw mut process_id) };
    if thread_id == 0 {
        return Err(format!(
            "cannot read StickyMD GUI thread for input-language routing: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(thread_id)
}

fn keyboard_layout(thread_id: u32) -> isize {
    // SAFETY: the thread id came from the live exact-candidate HWND; the API
    // returns a copied input-locale handle and retains no caller state.
    unsafe { GetKeyboardLayout(thread_id) }
}

fn input_language(layout: isize) -> u16 {
    (layout as usize & 0xffff) as u16
}

pub(crate) struct ImeProfileGuard {
    manager: *mut ProfileManager,
    original: NativeProfile,
    target: NativeProfile,
    target_label: &'static str,
    active: bool,
    com_initialized: bool,
}

impl ImeProfileGuard {
    pub(crate) fn activate(profile: ImeProfile, window: WindowHandle) -> Result<Self, String> {
        let initialized = {
            // SAFETY: null reserved storage and a documented apartment flag are
            // passed on the current smoke thread. Successful initialization is
            // balanced by `CoUninitialize` on the same thread.
            unsafe { CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED) }
        };
        if initialized < 0 {
            return Err(format!(
                "cannot initialize TSF COM apartment: HRESULT=0x{:08X}",
                initialized as u32
            ));
        }

        let mut raw = ptr::null_mut();
        // SAFETY: all GUID pointers refer to static values, aggregation is not
        // requested, and `raw` is valid writable interface-pointer storage.
        let created = unsafe {
            CoCreateInstance(
                &CLSID_TF_INPUT_PROCESSOR_PROFILES,
                ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &IID_INPUT_PROCESSOR_PROFILE_MGR,
                &mut raw,
            )
        };
        if created != S_OK || raw.is_null() {
            // SAFETY: COM initialization succeeded on this thread and no
            // interface ownership escaped.
            unsafe { CoUninitialize() };
            return Err(format!(
                "cannot create TSF profile manager: HRESULT=0x{:08X}",
                created as u32
            ));
        }

        let manager = raw.cast::<ProfileManager>();
        let mut guard = Self {
            manager,
            original: NativeProfile::default(),
            target: NativeProfile::default(),
            target_label: profile.name(),
            active: false,
            com_initialized: true,
        };
        guard.original = active_profile(manager)?;
        let spec = profile.spec();
        let target = registered_profile(manager, spec)?;
        if target.flags & TF_IPP_FLAG_ENABLED == 0 {
            return Err(format!(
                "{} TSF profile is installed but disabled",
                spec.name
            ));
        }

        // From this point Drop must restore the captured profile even when
        // activation succeeds but acknowledgement times out.
        guard.active = true;
        guard.target = target;
        guard.activate_native(&target, spec.name)?;
        route_input_language(window, &target, spec.name)?;
        Ok(guard)
    }

    pub(crate) fn route_to(&self, window: WindowHandle) -> Result<(), String> {
        if !self.active {
            return Err("cannot route a restored IME profile guard".to_owned());
        }
        route_input_language(window, &self.target, self.target_label)
    }

    pub(crate) fn restore(mut self) -> Result<(), String> {
        let result = self.restore_inner();
        self.release();
        result
    }

    fn activate_native(&mut self, profile: &NativeProfile, label: &str) -> Result<(), String> {
        // SAFETY: the guard owns the interface reference for this whole call.
        let manager = unsafe { self.manager.as_ref() }
            .ok_or_else(|| "TSF profile manager is null".to_owned())?;
        let vtable = profile_vtable(manager)?;
        // SAFETY: `self.manager` owns a live COM interface; all profile fields
        // are copied values supplied by TSF and the call retains no pointers.
        let result = unsafe {
            (vtable.activate_profile)(
                self.manager.cast(),
                profile.profile_type,
                profile.language,
                &profile.class,
                &profile.profile,
                profile.keyboard_layout,
                TF_IPPMF_FORSESSION,
            )
        };
        if result != S_OK {
            return Err(format!(
                "cannot activate {label} for the current input desktop: HRESULT=0x{:08X}",
                result as u32
            ));
        }
        wait_for_profile(self.manager, profile, label)
    }

    fn restore_inner(&mut self) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        let original = self.original;
        self.activate_native(&original, "original input profile")?;
        self.active = false;
        Ok(())
    }

    fn release(&mut self) {
        if !self.manager.is_null() {
            release_manager(self.manager);
            self.manager = ptr::null_mut();
        }
        if self.com_initialized {
            // SAFETY: this balances successful initialization on this same
            // thread after all COM interface references have been released.
            unsafe { CoUninitialize() };
            self.com_initialized = false;
        }
    }
}

impl Drop for ImeProfileGuard {
    fn drop(&mut self) {
        let _ = self.restore_inner();
        self.release();
    }
}

fn active_profile(manager: *mut ProfileManager) -> Result<NativeProfile, String> {
    // SAFETY: callers retain the live interface reference for this call.
    let manager_ref =
        unsafe { manager.as_ref() }.ok_or_else(|| "TSF profile manager is null".to_owned())?;
    let vtable = profile_vtable(manager_ref)?;
    let mut profile = NativeProfile::default();
    // SAFETY: `manager` is live and `profile` is valid writable storage for a
    // copied TSF profile structure.
    let result = unsafe {
        (vtable.get_active_profile)(manager.cast(), &GUID_TFCAT_TIP_KEYBOARD, &mut profile)
    };
    if result != S_OK {
        return Err(format!(
            "cannot read active TSF keyboard profile: HRESULT=0x{:08X}",
            result as u32
        ));
    }
    Ok(profile)
}

fn registered_profile(
    manager: *mut ProfileManager,
    spec: ProfileSpec,
) -> Result<NativeProfile, String> {
    // SAFETY: callers retain the live interface reference for this call.
    let manager_ref =
        unsafe { manager.as_ref() }.ok_or_else(|| "TSF profile manager is null".to_owned())?;
    let vtable = profile_vtable(manager_ref)?;
    let mut profile = NativeProfile::default();
    // SAFETY: `manager` is live, GUID pointers refer to copied constants, and
    // `profile` is valid writable storage retained only for this call.
    let result = unsafe {
        (vtable.get_profile)(
            manager.cast(),
            TF_PROFILETYPE_INPUTPROCESSOR,
            0x0804,
            &spec.class,
            &spec.profile,
            0,
            &mut profile,
        )
    };
    if result != S_OK {
        return Err(format!(
            "{} TSF profile is unavailable: HRESULT=0x{:08X}",
            spec.name, result as u32
        ));
    }
    Ok(profile)
}

fn wait_for_profile(
    manager: *mut ProfileManager,
    expected: &NativeProfile,
    label: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + PROFILE_TIMEOUT;
    loop {
        let observed = active_profile(manager)?;
        if same_profile(&observed, expected) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "TSF did not acknowledge {label} activation within {PROFILE_TIMEOUT:?}"
            ));
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn same_profile(left: &NativeProfile, right: &NativeProfile) -> bool {
    left.profile_type == right.profile_type
        && left.language == right.language
        && left.class == right.class
        && left.profile == right.profile
        && left.keyboard_layout == right.keyboard_layout
}

fn profile_vtable(manager: &ProfileManager) -> Result<&ProfileManagerVTable, String> {
    // SAFETY: a successful `CoCreateInstance` for this IID supplies a stable
    // vtable for the lifetime of the borrowed interface.
    unsafe { manager.vtable.as_ref() }
        .ok_or_else(|| "TSF profile manager has a null vtable".to_owned())
}

fn release_manager(manager: *mut ProfileManager) {
    // SAFETY: this is called before consuming the guard's sole live reference.
    let Some(manager_ref) = (unsafe { manager.as_ref() }) else {
        return;
    };
    let Ok(vtable) = profile_vtable(manager_ref) else {
        return;
    };
    // SAFETY: the guard owns exactly one interface reference returned by
    // `CoCreateInstance`; this consumes that reference once.
    unsafe { (vtable.base.release)(manager.cast()) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_profile_guids_match_windows_tip_contract() {
        assert_eq!(MICROSOFT_PINYIN.class.data1, 0x81D4_E9C9);
        assert_eq!(MICROSOFT_PINYIN.profile.data1, 0xFA55_0B04);
        assert_eq!(WETYPE.class.data1, 0x8659_8FB9);
        assert_eq!(WETYPE.profile.data1, 0x607F_DF85);
        assert_ne!(MICROSOFT_PINYIN.class, WETYPE.class);
    }

    #[test]
    fn profile_identity_ignores_capabilities_but_not_tip_identity() {
        let left = NativeProfile {
            profile_type: 1,
            language: 0x0804,
            class: MICROSOFT_PINYIN.class,
            profile: MICROSOFT_PINYIN.profile,
            capabilities: 1,
            ..NativeProfile::default()
        };
        let mut right = left;
        right.capabilities = 2;
        assert!(same_profile(&left, &right));
        right.profile = WETYPE.profile;
        assert!(!same_profile(&left, &right));
    }
}
