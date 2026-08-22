//! StickyMD Windows development entry point.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod app;
mod assets;
#[cfg(windows)]
mod config;
#[cfg(windows)]
mod export;
#[cfg(windows)]
mod flow;
#[cfg(windows)]
mod instruction;
#[cfg(windows)]
mod interaction;
#[cfg(windows)]
mod persistence;
#[cfg(windows)]
mod platform;
#[cfg(windows)]
mod preview;
#[cfg(windows)]
mod startup;
#[cfg(windows)]
mod surface;

#[cfg(windows)]
fn main() {
    use app::{AppEvent, StickyApp};
    use persistence::{IoCompletion, PersistenceWorker};
    use platform::windows::program_dir::RuntimePaths;
    use platform::windows::single_instance::{InstanceDisposition, SingleInstanceGuard};
    use startup::StartupDiagnostics;
    use startup::{BootstrapMilestone, bootstrap_observed};
    use std::sync::{Arc, Mutex};
    use winit::event_loop::{EventLoop, EventLoopProxy};
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
        Ok(InstanceDisposition::SecondarySignaled) => return,
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
    // A normal startup is a quiescent destructive-GC boundary: no editor or
    // worker exists yet, so the reference snapshot cannot become stale while
    // reconciliation is deleting proven unreferenced trash. Recovery choices
    // perform the same barrier before re-enabling input.
    if bootstrap.recovery.is_none() {
        match assets::AssetStorage::open(&paths.images_dir, &paths.trash_dir).and_then(|storage| {
            assets::reconcile_safe_boundary(
                &storage,
                &paths.note_file,
                bootstrap.document.base_disk_hash(),
                bootstrap.document.managed_ref_counts(),
            )
        }) {
            Ok(report) if !report.missing_references.is_empty() => {
                bootstrap.warnings.push(format!(
                    "有 {} 个受管图片引用缺少文件；Markdown 未修改。",
                    report.missing_references.len()
                ))
            }
            Ok(_) => {}
            Err(error) => bootstrap.warnings.push(format!(
                "启动图片整理未完成；用户文件与文档均未删除：{error}"
            )),
        }
    }

    let native_proxy: Arc<Mutex<Option<EventLoopProxy<AppEvent>>>> = Arc::new(Mutex::new(None));
    let native_proxy_for_hook = Arc::clone(&native_proxy);
    let mut event_loop_builder = EventLoop::<AppEvent>::with_user_event();
    event_loop_builder.with_msg_hook(move |message| {
        if let Some(signal) = platform::windows::native_message::translate_message(message)
            && let Ok(proxy) = native_proxy_for_hook.lock()
            && let Some(proxy) = proxy.as_ref()
        {
            let _ = proxy.send_event(AppEvent::Native(signal));
        }
        false
    });
    let event_loop = match event_loop_builder.build() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            eprintln!("event loop creation failed: {error}");
            std::process::exit(1);
        }
    };
    startup_diagnostics.record("event_loop_ready");
    let proxy = event_loop.create_proxy();
    if let Ok(mut slot) = native_proxy.lock() {
        *slot = Some(proxy.clone());
    }
    let wake_proxy = proxy.clone();
    if let Err(error) = instance.start_listener(move || {
        let _ = wake_proxy.send_event(AppEvent::ShowRequested);
    }) {
        fatal_startup(&format!("无法监听第二实例唤醒请求：{error}"));
    }
    let io_proxy = proxy.clone();
    let worker = match PersistenceWorker::start(move |completion: IoCompletion| {
        let _ = io_proxy.send_event(AppEvent::Io(completion));
    }) {
        Ok(worker) => worker,
        Err(error) => fatal_startup(&format!("无法启动持久化工作线程：{error}")),
    };
    let mut app = StickyApp::new(
        paths,
        bootstrap,
        instance,
        worker,
        proxy,
        startup_diagnostics,
    );
    if let Err(error) = event_loop.run_app(&mut app) {
        eprintln!("application event loop failed: {error}");
        std::process::exit(1);
    }
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
