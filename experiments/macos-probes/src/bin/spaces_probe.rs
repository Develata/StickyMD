//! What a floating or docked window sees while a person switches desktops (Spaces).
//! Yellow window: joins all Spaces, 4 switches. Red 3 pt strip at the left edge: same, 2 switches.
//! Blue window: default behaviour, 2 switches. Ends by itself; gives up after 30 minutes.
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Sel};
use objc2::{class, msg_send, sel};
use objc2_foundation::{NSRect, NSString};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId, WindowLevel};

unsafe extern "C" {
    fn object_getClass(object: *const AnyObject) -> *const AnyClass;
    fn class_addMethod(class: *const AnyClass, name: Sel, imp: Imp, types: *const std::ffi::c_char) -> Bool;
}

extern "C-unwind" fn keep_frame(_this: *mut AnyObject, _sel: Sel, frame: NSRect, _screen: *mut AnyObject) -> NSRect {
    frame
}

static CHANGES: AtomicU64 = AtomicU64::new(0);

/// Read-only private call, for this probe's log only: which Space is active.
fn active_space() -> u64 {
    type ConnectionId = unsafe extern "C" fn() -> i32;
    type GetActive = unsafe extern "C" fn(i32) -> u64;
    unsafe {
        let connection = libc::dlsym(libc::RTLD_DEFAULT, c"CGSMainConnectionID".as_ptr());
        let active = libc::dlsym(libc::RTLD_DEFAULT, c"CGSGetActiveSpace".as_ptr());
        if connection.is_null() || active.is_null() {
            return 0;
        }
        let connection: ConnectionId = std::mem::transmute(connection);
        let active: GetActive = std::mem::transmute(active);
        active(connection())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Snapshot(&'static str),
    Repaint,
    HideShow,
    Focus,
}

const PHASES: [(&str, u64, u32); 3] = [
    ("joins all Spaces, floating", 4, 0x00ff_d60a),
    ("joins all Spaces, docked strip", 2, 0x00ff_3b30),
    ("default behaviour", 2, 0x000a_84ff),
];

struct App {
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    observer: Option<Retained<AnyObject>>,
    started: Instant,
    phase: usize,
    seen: u64,
    in_phase: u64,
    schedule: Vec<(Instant, Action)>,
    phase_ends: Option<Instant>,
    give_up: Instant,
}

impl App {
    fn at(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    fn view(&self) -> *mut AnyObject {
        let window = self.window.as_ref().unwrap();
        let RawWindowHandle::AppKit(handle) = window.window_handle().unwrap().as_raw() else {
            panic!("not AppKit")
        };
        handle.ns_view.as_ptr().cast()
    }

    fn ns_window(&self) -> *mut AnyObject {
        unsafe { msg_send![self.view(), window] }
    }

    fn snapshot(&self, label: &str) {
        let ns = self.ns_window();
        let app: *mut AnyObject = unsafe { msg_send![class!(NSApplication), sharedApplication] };
        unsafe {
            let on_space: bool = msg_send![ns, isOnActiveSpace];
            let visible: bool = msg_send![ns, isVisible];
            let key: bool = msg_send![ns, isKeyWindow];
            let active: bool = msg_send![app, isActive];
            let frame: NSRect = msg_send![ns, frame];
            let occlusion: usize = msg_send![ns, occlusionState];
            println!(
                "{:8.3} [{label}] space={} on_active_space={on_space} visible={visible} key={key} app_active={active} origin=({:.0},{:.0}) occlusion_visible={}",
                self.at(), active_space(), frame.origin.x, frame.origin.y, occlusion & 2 != 0
            );
        }
    }

    fn paint(&mut self) {
        let color = PHASES[self.phase.min(PHASES.len() - 1)].2;
        let window = self.window.as_ref().unwrap();
        let size = window.inner_size();
        let surface = self.surface.as_mut().unwrap();
        surface.resize(NonZeroU32::new(size.width).unwrap(), NonZeroU32::new(size.height).unwrap()).unwrap();
        match surface.buffer_mut() {
            Ok(mut buffer) => {
                buffer.fill(color);
                if let Err(error) = buffer.present() {
                    println!("present failed: {error}");
                }
            }
            Err(error) => println!("buffer failed: {error}"),
        }
    }

    fn enter_phase(&mut self) {
        let (name, need, _) = PHASES[self.phase];
        println!("{:8.3} === phase {}: {name}, needs {need} switches", self.at(), self.phase + 1);
        let window = self.window.clone().unwrap();
        let ns = self.ns_window();
        self.in_phase = 0;
        self.phase_ends = None;
        match self.phase {
            0 => unsafe {
                let _: () = msg_send![ns, setCollectionBehavior: 1usize | (1usize << 8)];
                window.set_window_level(WindowLevel::AlwaysOnTop);
                window.set_outer_position(LogicalPosition::new(420.0, 260.0));
                window.focus_window();
            },
            1 => window.set_outer_position(LogicalPosition::new(3.0 - 320.0, 300.0)),
            _ => unsafe {
                let _: () = msg_send![ns, setCollectionBehavior: 0usize];
                window.set_window_level(WindowLevel::Normal);
                window.set_outer_position(LogicalPosition::new(420.0, 260.0));
                window.focus_window();
            },
        }
        self.paint();
        self.snapshot("phase start");
    }

    fn space_changed(&mut self, now: Instant) {
        self.in_phase += 1;
        let n = self.in_phase;
        println!("{:8.3} --- space change {n} in phase {}", self.at(), self.phase + 1);
        self.snapshot("at change");
        self.schedule.push((now + Duration::from_millis(200), Action::Repaint));
        self.schedule.push((now + Duration::from_millis(400), Action::Snapshot("+0.4 s")));
        self.schedule.push((now + Duration::from_millis(1200), Action::Snapshot("+1.2 s")));
        match (self.phase, n) {
            (0, 2) => self.schedule.push((now + Duration::from_millis(1300), Action::HideShow)),
            (0, 3) | (2, 1) => self.schedule.push((now + Duration::from_millis(1500), Action::Focus)),
            _ => {}
        }
        if n >= PHASES[self.phase].1 {
            self.phase_ends = Some(now + Duration::from_millis(3000));
        } else if self.phase == 2 {
            self.phase_ends = Some(now + Duration::from_secs(8));
        }
    }

    fn run(&mut self, action: Action) {
        let window = self.window.clone().unwrap();
        match action {
            Action::Snapshot(label) => self.snapshot(label),
            Action::Repaint => self.paint(),
            Action::HideShow => {
                window.set_visible(false);
                self.snapshot("hidden");
                window.set_visible(true);
                self.paint();
                self.snapshot("shown again on this Space");
            }
            Action::Focus => {
                self.snapshot("before focus_window");
                window.focus_window();
                self.schedule.push((Instant::now() + Duration::from_millis(1200), Action::Snapshot("+1.2 s after focus_window")));
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_decorations(false).with_inner_size(LogicalSize::new(320.0, 240.0));
        let window = Rc::new(event_loop.create_window(attributes).unwrap());
        let context = softbuffer::Context::new(window.clone()).unwrap();
        self.surface = Some(softbuffer::Surface::new(&context, window.clone()).unwrap());
        self.window = Some(window);
        unsafe {
            let class = object_getClass(self.ns_window().cast_const());
            let imp: Imp = std::mem::transmute(keep_frame as *const ());
            class_addMethod(class, sel!(constrainFrameRect:toScreen:), imp, c"{CGRect={CGPoint=dd}{CGSize=dd}}@:{CGRect={CGPoint=dd}{CGSize=dd}}@".as_ptr());
            let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
            let center: *mut AnyObject = msg_send![workspace, notificationCenter];
            let queue: *mut AnyObject = msg_send![class!(NSOperationQueue), mainQueue];
            let name = NSString::from_str("NSWorkspaceActiveSpaceDidChangeNotification");
            let block = RcBlock::new(|_note: *mut AnyObject| {
                CHANGES.fetch_add(1, Ordering::Relaxed);
            });
            let nil: *mut AnyObject = std::ptr::null_mut();
            let observer: Retained<AnyObject> = msg_send![center, addObserverForName: &*name, object: nil, queue: queue, usingBlock: &*block];
            self.observer = Some(observer);
        }
        self.enter_phase();
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::Focused(_) | WindowEvent::Occluded(_) | WindowEvent::Moved(_) | WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } | WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                println!("{:8.3} event {event:?}", self.at());
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            return;
        }
        let now = Instant::now();
        let changes = CHANGES.load(Ordering::Relaxed);
        while self.seen < changes {
            self.seen += 1;
            self.space_changed(now);
        }
        let mut due = Vec::new();
        self.schedule.retain(|(when, action)| {
            if *when <= now {
                due.push(*action);
                false
            } else {
                true
            }
        });
        for action in due {
            self.run(action);
        }
        if self.phase_ends.is_some_and(|end| now >= end) && self.schedule.is_empty() {
            self.phase += 1;
            if self.phase == PHASES.len() {
                println!("{:8.3} finished", self.at());
                self.phase_ends = None;
                event_loop.exit();
                return;
            }
            self.enter_phase();
        }
        if now >= self.give_up {
            println!("{:8.3} gave up in phase {}", self.at(), self.phase + 1);
            event_loop.exit();
            return;
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50)));
    }
}

fn main() {
    let event_loop = EventLoop::builder().with_activation_policy(ActivationPolicy::Accessory).with_default_menu(false).build().unwrap();
    let now = Instant::now();
    let mut app = App {
        window: None,
        surface: None,
        observer: None,
        started: now,
        phase: 0,
        seen: 0,
        in_phase: 0,
        schedule: Vec::new(),
        phase_ends: None,
        give_up: now + Duration::from_secs(1800),
    };
    event_loop.run_app(&mut app).unwrap();
}
