//! A bundle that logs how macOS starts and re-opens it. `Probe.app/Contents/MacOS/bundle_probe`.
//! Writes `<dir containing Probe.app>/probe-<pid>.log`, runs for the seconds in PROBE_SECONDS.
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{AllocAnyThread, class, define_class, msg_send, sel};
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::window::{Window, WindowId};

static LOG: OnceLock<Mutex<std::fs::File>> = OnceLock::new();

fn log(text: &str) {
    let mut file = LOG.get().unwrap().lock().unwrap();
    writeln!(file, "{text}").unwrap();
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "ProbeReopenHandler"]
    struct Reopen;

    impl Reopen {
        #[unsafe(method(handle:reply:))]
        fn handle(&self, _event: *mut AnyObject, _reply: *mut AnyObject) {
            log("apple event: reopen");
        }
    }
);

const fn four(code: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*code)
}

struct App {
    window: Option<Window>,
    handler: Option<Retained<Reopen>>,
    end: Instant,
}

impl ApplicationHandler for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::Init) {
            let handler: Retained<Reopen> = unsafe { msg_send![Reopen::alloc(), init] };
            unsafe {
                let manager: *mut AnyObject = msg_send![class!(NSAppleEventManager), sharedAppleEventManager];
                let _: () = msg_send![
                    manager,
                    setEventHandler: &*handler,
                    andSelector: sel!(handle:reply:),
                    forEventClass: four(b"aevt"),
                    andEventID: four(b"rapp")
                ];
            }
            self.handler = Some(handler);
            log("handler installed at Init");
        }
        if Instant::now() >= self.end {
            log("exit by timer");
            event_loop.exit();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.end));
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attributes = Window::default_attributes()
                .with_decorations(false)
                .with_inner_size(winit::dpi::LogicalSize::new(120.0, 80.0));
            self.window = Some(event_loop.create_window(attributes).unwrap());
        }
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let WindowEvent::Focused(focused) = event {
            log(&format!("focused: {focused}"));
        }
    }
}

fn main() {
    let exe = std::env::current_exe().unwrap();
    let bundle = exe.ancestors().nth(3).unwrap().to_path_buf();
    let program_dir: PathBuf = bundle.parent().unwrap().to_path_buf();
    let log_path = program_dir.join(format!("probe-{}.log", std::process::id()));
    LOG.set(Mutex::new(OpenOptions::new().create(true).append(true).open(&log_path).unwrap())).ok();
    log(&format!("pid {} exe {}", std::process::id(), exe.display()));
    log(&format!("canonical exe {}", std::fs::canonicalize(&exe).unwrap().display()));
    log(&format!("program dir {}", program_dir.display()));
    log(&format!("cwd {}", std::env::current_dir().unwrap().display()));
    log(&format!("args {:?}", std::env::args().collect::<Vec<_>>()));
    log(&format!("PATH {:?} TMPDIR {:?}", std::env::var("PATH").ok(), std::env::var("TMPDIR").ok()));
    let seconds = std::env::var("PROBE_SECONDS").ok().and_then(|s| s.parse().ok()).unwrap_or(12);
    let event_loop = EventLoop::builder()
        .with_activation_policy(ActivationPolicy::Accessory)
        .with_default_menu(false)
        .build()
        .unwrap();
    let mut app = App { window: None, handler: None, end: Instant::now() + Duration::from_secs(seconds) };
    event_loop.run_app(&mut app).unwrap();
}
