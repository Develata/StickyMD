//! Token authentication LUID distinguishes logons even when Windows session numbers are reused.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

#[repr(C)]
#[derive(Default)]
struct Luid {
    low: u32,
    high: i32,
}

// https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-token_statistics
#[repr(C)]
#[derive(Default)]
struct TokenStatistics {
    token: Luid,
    authentication: Luid,
    expiration: i64,
    kind: u32,
    impersonation: u32,
    charged: u32,
    available: u32,
    groups: u32,
    privileges: u32,
    modified: Luid,
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn OpenProcessToken(process: isize, access: u32, token: *mut isize) -> i32;
    fn GetTokenInformation(
        token: isize,
        class: u32,
        output: *mut TokenStatistics,
        length: u32,
        returned: *mut u32,
    ) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CloseHandle(handle: isize) -> i32;
}

pub(super) fn identity() -> Result<String, String> {
    let mut token = 0;
    // SAFETY: -1 is the current-process pseudo handle; TOKEN_QUERY only, with a valid out pointer.
    if unsafe { OpenProcessToken(-1, 0x0008, &mut token) } == 0 {
        return Err("cannot inspect diagnostic logon identity".into());
    }
    let mut stats = TokenStatistics::default();
    let mut returned = 0;
    let size = std::mem::size_of::<TokenStatistics>() as u32;
    // SAFETY: the owned token stays live, class 10 is TokenStatistics and the output buffer matches it.
    let success = unsafe { GetTokenInformation(token, 10, &mut stats, size, &mut returned) };
    // SAFETY: successful OpenProcessToken transferred one owned handle, used only above.
    unsafe { CloseHandle(token) };
    if success == 0
        || returned != size
        || (stats.authentication.low == 0 && stats.authentication.high == 0)
    {
        return Err("incomplete diagnostic logon identity".into());
    }
    Ok(format!(
        "{}:{}",
        stats.authentication.high, stats.authentication.low
    ))
}
