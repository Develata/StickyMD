//! Which automation this process may use. Only asks; never triggers a permission prompt.
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn CGPreflightPostEventAccess() -> bool;
    fn CGPreflightListenEventAccess() -> bool;
    fn CGPreflightScreenCaptureAccess() -> bool;
}

fn main() {
    unsafe {
        println!("AXIsProcessTrusted: {}", AXIsProcessTrusted());
        println!("CGPreflightPostEventAccess: {}", CGPreflightPostEventAccess());
        println!("CGPreflightListenEventAccess: {}", CGPreflightListenEventAccess());
        println!("CGPreflightScreenCaptureAccess: {}", CGPreflightScreenCaptureAccess());
    }
}
