//! Nonprompting permission preflights. Called only after the VM interlock.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::super::model::Permissions;
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
}
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightPostEventAccess() -> bool;
    fn CGPreflightScreenCaptureAccess() -> bool;
}
pub(in crate::mac_gui) fn check() -> Permissions {
    // SAFETY: nonprompting zero-argument system preflights; no pointers or ownership.
    unsafe {
        Permissions {
            ax: AXIsProcessTrusted(),
            post: CGPreflightPostEventAccess(),
            screen: CGPreflightScreenCaptureAccess(),
        }
    }
}
