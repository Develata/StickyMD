//! StickyMD macOS entry point.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping
#![deny(unsafe_op_in_unsafe_fn)]

/// Large blocks are their own mappings, so a frame freed on hide leaves the footprint at once
/// (`10` macOS gates).
#[cfg(target_os = "macos")]
#[global_allocator]
static ALLOCATOR: stickymd_posix::allocator::LargeMappedAllocator =
    stickymd_posix::allocator::LargeMappedAllocator;

#[cfg(target_os = "macos")]
mod assets;
#[cfg(target_os = "macos")]
mod config;
#[cfg(target_os = "macos")]
mod export;
#[cfg(target_os = "macos")]
mod host;
#[cfg(target_os = "macos")]
mod paths;
#[cfg(target_os = "macos")]
mod persistence;
#[cfg(target_os = "macos")]
mod platform;
#[cfg(target_os = "macos")]
mod preview;
#[cfg(target_os = "macos")]
mod startup;
#[cfg(all(target_os = "macos", test))]
mod test_support;

#[cfg(target_os = "macos")]
fn main() -> std::process::ExitCode {
    use std::time::Instant;

    use host::MacHost;
    use paths::{RuntimePaths, RuntimePathsError};
    use persistence::{MacAssetStore, mac_io_ports};
    use platform::macos::app::disable_press_and_hold;
    use platform::macos::clipboard::MacClipboard;
    use platform::macos::instance::{InstanceDisposition, InstanceGuard};
    use startup::bootstrap;
    use stickymd_app::execution::{IoCompletion, PersistenceWorker};
    use stickymd_app::flow::reconcile_at_startup;
    use stickymd_app::session::{AppSession, InstantClock, SessionParts};
    use stickymd_desktop::host::{DesktopEvent, EventSink, HostSignal};
    use stickymd_desktop::shell::DesktopShell;
    use stickymd_render::system_fonts::{FontDefaults, set_font_defaults};
    use winit::event_loop::EventLoop;
    use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

    if !disable_press_and_hold() {
        eprintln!("the accent popup could not be turned off; holding a key may add characters");
    }
    set_font_defaults(FontDefaults::MACOS);
    let paths = match RuntimePaths::resolve_current() {
        Ok(paths) => paths,
        Err(error) => fatal_startup(&format!("无法确定程序目录：{error}")),
    };
    let mut instance = match InstanceGuard::acquire(&paths.program_dir) {
        Ok(InstanceDisposition::Primary(instance)) => instance,
        Ok(InstanceDisposition::SecondarySignaled) => return std::process::ExitCode::SUCCESS,
        Err(error) => fatal_startup(&format!("无法建立单实例保护：{error}")),
    };
    if let Err(error) = paths.verify_program_directory_writable() {
        let advice = if matches!(error, RuntimePathsError::VolumeUnsupported(_)) {
            "当前卷不支持安全保存，请将程序移到 APFS 卷。"
        } else if paths.is_translocated() {
            "当前目录不可写，请先用访达把 StickyMD.app 移到别处。"
        } else {
            "当前目录不可写，请将程序移动到有写权限的文件夹。"
        };
        fatal_startup(&format!("{advice}\n\n{error}"));
    }
    if let Err(error) = paths.ensure_layout() {
        fatal_startup(&format!("无法创建便签目录：{error}"));
    }
    let mut outcome = match bootstrap(&paths) {
        Ok(outcome) => outcome,
        Err(error) => fatal_startup(&format!("StickyMD 无法安全启动：{error}")),
    };
    let mut startup_assets = match MacAssetStore::open(&paths) {
        Ok(store) => store,
        Err(error) => fatal_startup(&format!("无法打开便签目录：{error}")),
    };
    if let Some(warning) = reconcile_at_startup(
        &mut startup_assets,
        &outcome.document,
        outcome.recovery.is_some(),
    ) {
        outcome.warnings.push(warning);
    }

    let event_loop = match EventLoop::<DesktopEvent>::with_user_event()
        .with_activation_policy(ActivationPolicy::Accessory)
        .with_default_menu(false)
        .build()
    {
        Ok(event_loop) => event_loop,
        Err(error) => fatal_startup(&format!("无法创建事件循环：{error}")),
    };
    let proxy = event_loop.create_proxy();
    let sink = EventSink::new(move |event| {
        let _ = proxy.send_event(event);
    });
    let wake_sink = sink.clone();
    if let Err(error) = instance.start_listener(move || {
        wake_sink.post(DesktopEvent::Host(HostSignal::ShowRequested));
    }) {
        fatal_startup(&format!("无法监听第二实例唤醒请求：{error}"));
    }
    let ports = match mac_io_ports(&paths) {
        Ok(ports) => ports,
        Err(error) => fatal_startup(&format!("无法打开便签目录：{error}")),
    };
    let note_directory = ports.preview_directory;
    let io_sink = sink.clone();
    let worker = match PersistenceWorker::start(ports.io, move |completion: IoCompletion| {
        io_sink.post(DesktopEvent::Io(completion));
    }) {
        Ok(worker) => worker,
        Err(error) => fatal_startup(&format!("无法启动持久化工作线程：{error}")),
    };
    let started = Instant::now();
    let diagnostic = outcome.warnings.last().cloned();
    let session = AppSession::new(SessionParts {
        document: outcome.document,
        clipboard: Box::new(MacClipboard),
        config: outcome.config,
        config_persistence_allowed: outcome.config_persistence_allowed,
        recovery: outcome.recovery,
        recovery_canonical_to_preserve: outcome.recovery_canonical_to_preserve,
        worker,
        clock: Box::new(InstantClock::starting_at(started)),
    });
    let host = MacHost::new(paths, note_directory, instance, sink.clone());
    let mut app = DesktopShell::new(host, session, sink, started, diagnostic);
    let run = event_loop.run_app(&mut app);
    let startup_failed = app.startup_failed();
    drop(app);
    if let Err(error) = run {
        fatal_startup(&format!("事件循环异常结束：{error}"));
    }
    if startup_failed {
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(target_os = "macos")]
fn fatal_startup(message: &str) -> ! {
    eprintln!("{message}");
    platform::macos::dialog::show_error("StickyMD", message);
    std::process::exit(1);
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("The StickyMD macOS host builds only for aarch64-apple-darwin.");
}
