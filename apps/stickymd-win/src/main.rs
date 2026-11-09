//! StickyMD Windows development entry point.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(windows, windows_subsystem = "windows")]

mod assets;
#[cfg(windows)]
mod config;
#[cfg(windows)]
mod export;
#[cfg(windows)]
mod host;
#[cfg(windows)]
mod persistence;
#[cfg(windows)]
mod platform;
#[cfg(windows)]
mod preview;
#[cfg(all(windows, test))]
mod search_label_fonts;
#[cfg(windows)]
mod startup;
#[cfg(test)]
mod test_support;

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    use host::WinHost;
    use persistence::{WinAssetStore, win_io_ports};
    use platform::windows::program_dir::RuntimePaths;
    use platform::windows::single_instance::{InstanceDisposition, SingleInstanceGuard};
    use startup::StartupDiagnostics;
    use startup::{BootstrapMilestone, bootstrap_observed};
    use std::sync::{Arc, Mutex};
    use std::time::Instant;
    use stickymd_app::execution::{IoCompletion, PersistenceWorker};
    use stickymd_app::flow::reconcile_at_startup;
    use stickymd_desktop::host::{DesktopEvent, EventSink, HostSignal};
    use stickymd_desktop::shell::DesktopShell;
    use winit::event_loop::EventLoop;
    use winit::platform::windows::EventLoopBuilderExtWindows;

    let mut startup_diagnostics = StartupDiagnostics::from_environment();
    startup_diagnostics.record("main_enter");
    let paths = match RuntimePaths::resolve_current() {
        Ok(paths) => paths,
        Err(error) => fatal_startup(&format!("无法确定程序目录：{error}")),
    };
    startup_diagnostics.record("program_dir_ready");
    let mut instance = match SingleInstanceGuard::acquire(&paths.program_dir) {
        Ok(InstanceDisposition::Primary(instance)) => instance,
        Ok(InstanceDisposition::SecondarySignaled) => return std::process::ExitCode::SUCCESS,
        Err(error) => fatal_startup(&format!("无法建立单实例保护：{error}")),
    };
    startup_diagnostics.record("single_instance_ready");
    if let Err(error) = paths.verify_program_directory_writable() {
        fatal_startup(&format!(
            "当前目录不可写，请将程序移动到有写权限的文件夹。\n\n{error}"
        ));
    }
    if let Err(error) = paths.ensure_layout() {
        fatal_startup(&format!("无法创建便签目录：{error}"));
    }
    startup_diagnostics.record("persistence_ready");
    let mut bootstrap = match bootstrap_observed(&paths, |milestone| match milestone {
        BootstrapMilestone::ConfigReady => startup_diagnostics.record("config_ready"),
    }) {
        Ok(bootstrap) => bootstrap,
        Err(error) => fatal_startup(&format!("StickyMD 无法安全启动：{error}")),
    };
    startup_diagnostics.record("document_ready");
    if let Some(warning) = reconcile_at_startup(
        &mut WinAssetStore::new(&paths),
        &bootstrap.document,
        bootstrap.recovery.is_some(),
    ) {
        bootstrap.warnings.push(warning);
    }

    let native_sink: Arc<Mutex<Option<EventSink>>> = Arc::new(Mutex::new(None));
    let native_sink_for_hook = Arc::clone(&native_sink);
    let mut event_loop_builder = EventLoop::<DesktopEvent>::with_user_event();
    event_loop_builder.with_msg_hook(move |message| {
        let action = platform::windows::native_message::handle_message(message);
        if let Some(signal) = action.signal
            && let Ok(sink) = native_sink_for_hook.lock()
            && let Some(sink) = sink.as_ref()
        {
            sink.post(DesktopEvent::Host(signal));
        }
        action.handled
    });
    let event_loop = match event_loop_builder.build() {
        Ok(event_loop) => event_loop,
        Err(error) => fatal_startup(&format!("无法创建事件循环：{error}")),
    };
    startup_diagnostics.record("event_loop_ready");
    let proxy = event_loop.create_proxy();
    let sink = EventSink::new(move |event| {
        let _ = proxy.send_event(event);
    });
    if let Ok(mut slot) = native_sink.lock() {
        *slot = Some(sink.clone());
    }
    let wake_sink = sink.clone();
    if let Err(error) = instance.start_listener(move || {
        wake_sink.post(DesktopEvent::Host(HostSignal::ShowRequested));
    }) {
        fatal_startup(&format!("无法监听第二实例唤醒请求：{error}"));
    }
    let io_sink = sink.clone();
    let ports = win_io_ports(&paths);
    let worker = match PersistenceWorker::start(ports, move |completion: IoCompletion| {
        io_sink.post(DesktopEvent::Io(completion));
    }) {
        Ok(worker) => worker,
        Err(error) => fatal_startup(&format!("无法启动持久化工作线程：{error}")),
    };
    let started = Instant::now();
    let (session, diagnostic) = bootstrap.into_session(worker, started);
    let host = WinHost::new(paths, instance, startup_diagnostics);
    let mut app = DesktopShell::new(host, session, sink, started, diagnostic);
    if let Err(error) = event_loop.run_app(&mut app) {
        fatal_startup(&format!("事件循环异常结束：{error}"));
    }
    if app.startup_failed() {
        return std::process::ExitCode::FAILURE;
    }
    app.host().startup_diagnostic_exit_code()
}

#[cfg(windows)]
fn fatal_startup(message: &str) -> ! {
    eprintln!("{message}");
    platform::windows::message_box::show_error("StickyMD", message);
    std::process::exit(1);
}

#[cfg(not(windows))]
fn main() {
    eprintln!("StickyMD v1 targets Windows 11 x64 only.");
}
