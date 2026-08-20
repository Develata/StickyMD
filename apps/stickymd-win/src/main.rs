//! StickyMD Windows development entry point.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(windows)]
mod app;
#[cfg(windows)]
mod flow;
#[cfg(windows)]
mod instruction;
#[cfg(windows)]
mod interaction;
#[cfg(windows)]
mod platform;
#[cfg(windows)]
mod surface;

#[cfg(windows)]
fn main() {
    use app::StickyApp;
    use winit::event_loop::EventLoop;

    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            eprintln!("event loop creation failed: {error}");
            std::process::exit(1);
        }
    };
    let mut app = StickyApp::new();
    if let Err(error) = event_loop.run_app(&mut app) {
        eprintln!("application event loop failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("StickyMD v1 targets Windows 11 x64 only.");
}
