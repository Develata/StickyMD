//! Can a 3 pt strip at a screen edge notice the pointer while another app is frontmost, and reveal
//! itself without taking focus? `hover_probe auto` warps the pointer; `hover_probe guided` waits for
//! a real mouse: touch the left edge, then the right edge, then just below the menu bar.
//! Four detectors run side by side and each reports its first hit per edge.
use std::cell::Cell;
use std::num::NonZeroU32;
use std::process::Command;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, NSObject, Sel};
use objc2::{AllocAnyThread, class, define_class, msg_send, sel};
use objc2_foundation::{NSPoint, NSRect};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId, WindowLevel};

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGWarpMouseCursorPosition(point: CGPoint) -> i32;
}

unsafe extern "C" {
    fn object_getClass(object: *const AnyObject) -> *const AnyClass;
    fn class_addMethod(class: *const AnyClass, name: Sel, imp: Imp, types: *const std::ffi::c_char) -> Bool;
}

extern "C-unwind" fn keep_frame(_this: *mut AnyObject, _sel: Sel, frame: NSRect, _screen: *mut AnyObject) -> NSRect {
    frame
}

const WINIT: usize = 0;
const AREA: usize = 1;
const GLOBAL: usize = 2;
const POLL: usize = 3;
const NAMES: [&str; 4] = ["winit tracking rect", "NSTrackingArea ActiveAlways", "NSEvent global monitor", "mouseLocation poll"];

static HITS: [AtomicU64; 4] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];

thread_local! {
    static STRIP: Cell<(f64, f64, f64, f64)> = const { Cell::new((0.0, 0.0, 0.0, 0.0)) };
    static STARTED: Instant = Instant::now();
}

fn seconds() -> f64 {
    STARTED.with(|s| s.elapsed().as_secs_f64())
}

fn hit(source: usize) {
    if HITS[source].fetch_add(1, Ordering::Relaxed) == 0 {
        println!("{:8.3} first hit: {}", seconds(), NAMES[source]);
    }
}

fn pointer() -> (f64, f64, f64) {
    let point: NSPoint = unsafe { msg_send![class!(NSEvent), mouseLocation] };
    let screen: *mut AnyObject = unsafe { msg_send![class!(NSScreen), mainScreen] };
    let frame: NSRect = unsafe { msg_send![screen, frame] };
    (point.x, frame.size.height - point.y, frame.size.height)
}

fn in_strip() -> bool {
    let (x, y, _) = pointer();
    let (left, top, width, height) = STRIP.with(Cell::get);
    x >= left && x <= left + width && y >= top && y <= top + height
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "HoverProbeOwner"]
    struct Owner;

    impl Owner {
        #[unsafe(method(mouseEntered:))]
        fn entered(&self, _event: *mut AnyObject) {
            hit(AREA);
        }

        #[unsafe(method(mouseExited:))]
        fn exited(&self, _event: *mut AnyObject) {}

        #[unsafe(method(mouseMoved:))]
        fn moved(&self, _event: *mut AnyObject) {
            hit(AREA);
        }
    }
);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Edge {
    Left,
    Right,
    Top,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Stage {
    Park,
    Armed,
    Revealed,
    Leaving,
    CostPoll20,
    CostPoll10,
    Done,
}

const SIZE: (f64, f64) = (320.0, 240.0);
const EDGES: [Edge; 3] = [Edge::Left, Edge::Right, Edge::Top];

struct App {
    auto: bool,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    owner: Option<Retained<Owner>>,
    monitor: Option<Retained<AnyObject>>,
    index: usize,
    stage: Stage,
    deadline: Instant,
    armed_at: Instant,
    outside_since: Option<Instant>,
    home: (f64, f64),
    cpu: f64,
    results: Vec<String>,
    edges: Vec<Edge>,
    /// How near the pointer came, so "never tried" and "tried and missed" can be told apart.
    closest: f64,
    crossed: u32,
    last_y: f64,
}

fn cpu_seconds() -> f64 {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    let time = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1e6;
    time(usage.ru_utime) + time(usage.ru_stime)
}

impl App {
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

    fn active(&self) -> bool {
        let app: *mut AnyObject = unsafe { msg_send![class!(NSApplication), sharedApplication] };
        unsafe { msg_send![app, isActive] }
    }

    fn screen(&self) -> (f64, f64, f64) {
        let screen: *mut AnyObject = unsafe { msg_send![class!(NSScreen), mainScreen] };
        let frame: NSRect = unsafe { msg_send![screen, frame] };
        let visible: NSRect = unsafe { msg_send![screen, visibleFrame] };
        let menu = frame.size.height - (visible.origin.y + visible.size.height);
        (frame.size.width, frame.size.height, menu)
    }

    fn paint(&mut self, color: u32) {
        let window = self.window.as_ref().unwrap();
        let size = window.inner_size();
        let surface = self.surface.as_mut().unwrap();
        surface.resize(NonZeroU32::new(size.width).unwrap(), NonZeroU32::new(size.height).unwrap()).unwrap();
        let mut buffer = surface.buffer_mut().unwrap();
        buffer.fill(color);
        buffer.present().unwrap();
    }

    fn track_approach(&mut self) {
        let (x, y, _) = pointer();
        let (left, top, width, height) = STRIP.with(Cell::get);
        let dx = (left - x).max(x - (left + width)).max(0.0);
        let dy = (top - y).max(y - (top + height)).max(0.0);
        self.closest = self.closest.min(dx.hypot(dy));
        if x >= left && x <= left + width && self.last_y > top + height && y < top {
            self.crossed += 1;
        }
        self.last_y = y;
    }

    fn park(&mut self) {
        let edge = self.edges[self.index];
        self.closest = f64::MAX;
        self.crossed = 0;
        self.last_y = 0.0;
        let (width, _, menu) = self.screen();
        let window = self.window.clone().unwrap();
        let (x, y, strip) = match edge {
            Edge::Left => (3.0 - SIZE.0, 300.0, (0.0, 300.0, 3.0, SIZE.1)),
            Edge::Right => (width - 3.0, 300.0, (width - 3.0, 300.0, 3.0, SIZE.1)),
            Edge::Top => (400.0, menu + 3.0 - SIZE.1, (400.0, menu, SIZE.0, 3.0)),
        };
        window.set_outer_position(LogicalPosition::new(x, y));
        STRIP.with(|cell| cell.set(strip));
        for counter in &HITS {
            counter.store(0, Ordering::Relaxed);
        }
        let color = match edge {
            Edge::Left => 0x00ff_3b30,
            Edge::Right => 0x0034_c759,
            Edge::Top => 0x00ff_9500,
        };
        self.paint(color);
        println!("{:8.3} parked {edge:?}, app_active={}", seconds(), self.active());
    }

    fn reveal(&mut self) {
        let (width, _, menu) = self.screen();
        let window = self.window.clone().unwrap();
        let (x, y) = match self.edges[self.index] {
            Edge::Left => (0.0, 300.0),
            Edge::Right => (width - SIZE.0, 300.0),
            Edge::Top => (400.0, menu),
        };
        window.set_outer_position(LogicalPosition::new(x, y));
        println!("{:8.3} revealed, app_active={} (must stay false)", seconds(), self.active());
    }

    fn pointer_outside_window(&self) -> bool {
        let (x, y, _) = pointer();
        let window = self.window.as_ref().unwrap();
        let outer = window.outer_position().unwrap().to_logical::<f64>(window.scale_factor());
        !(x >= outer.x && x <= outer.x + SIZE.0 && y >= outer.y && y <= outer.y + SIZE.1)
    }

    fn warp_into_strip(&self) {
        let (left, top, width, height) = STRIP.with(Cell::get);
        let target = CGPoint { x: left + width / 2.0, y: top + height / 2.0 };
        let result = unsafe { CGWarpMouseCursorPosition(target) };
        println!("{:8.3} warp into strip: {result}", seconds());
    }

    fn summarise(&mut self) {
        let edge = self.edges[self.index];
        let counts: Vec<String> = (0..4).map(|i| format!("{}={}", NAMES[i], HITS[i].load(Ordering::Relaxed))).collect();
        let line = format!("{edge:?}: {}; closest_pt={:.1} crossed_without_landing={}", counts.join(", "), self.closest, self.crossed);
        println!("{:8.3} {line}", seconds());
        self.results.push(line);
    }

    fn next_edge(&mut self, event_loop: &ActiveEventLoop) {
        self.summarise();
        self.index += 1;
        if self.index < self.edges.len() {
            self.stage = Stage::Park;
            self.deadline = Instant::now() + Duration::from_millis(400);
        } else if self.auto {
            let _ = unsafe { CGWarpMouseCursorPosition(CGPoint { x: self.home.0, y: self.home.1 }) };
            self.stage = Stage::CostPoll20;
            self.deadline = Instant::now() + Duration::from_secs(20);
            self.cpu = cpu_seconds();
        } else {
            println!("guided run finished: {:?}", self.results);
            self.stage = Stage::Done;
            event_loop.exit();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_decorations(false).with_inner_size(LogicalSize::new(SIZE.0, SIZE.1));
        let window = Rc::new(event_loop.create_window(attributes).unwrap());
        window.set_window_level(WindowLevel::AlwaysOnTop);
        let context = softbuffer::Context::new(window.clone()).unwrap();
        self.surface = Some(softbuffer::Surface::new(&context, window.clone()).unwrap());
        self.window = Some(window);
        unsafe {
            let class = object_getClass(self.ns_window().cast_const());
            let imp: Imp = std::mem::transmute(keep_frame as *const ());
            class_addMethod(class, sel!(constrainFrameRect:toScreen:), imp, c"{CGRect={CGPoint=dd}{CGSize=dd}}@:{CGRect={CGPoint=dd}{CGSize=dd}}@".as_ptr());
            let owner: Retained<Owner> = msg_send![Owner::alloc(), init];
            let bounds: NSRect = msg_send![self.view(), bounds];
            // MouseEnteredAndExited | MouseMoved | ActiveAlways | InVisibleRect
            let options: usize = 0x01 | 0x02 | 0x80 | 0x200;
            let area: *mut AnyObject = msg_send![class!(NSTrackingArea), alloc];
            let nil: *mut AnyObject = std::ptr::null_mut();
            let area: *mut AnyObject = msg_send![area, initWithRect: bounds, options: options, owner: &*owner, userInfo: nil];
            let _: () = msg_send![self.view(), addTrackingArea: area];
            self.owner = Some(owner);
            let block = RcBlock::new(|_event: *mut AnyObject| {
                if in_strip() {
                    hit(GLOBAL);
                }
            });
            let mask: u64 = 1 << 5; // NSEventMaskMouseMoved
            let monitor: Option<Retained<AnyObject>> = msg_send![class!(NSEvent), addGlobalMonitorForEventsMatchingMask: mask, handler: &*block];
            println!("global monitor installed: {}", monitor.is_some());
            self.monitor = monitor;
        }
        let (x, y, _) = pointer();
        self.home = (x, y);
        let _ = Command::new("/usr/bin/osascript").args(["-e", "tell application \"Finder\" to activate"]).status();
        self.deadline = Instant::now() + Duration::from_millis(1200);
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CursorEntered { .. } => {
                println!("{:8.3} winit CursorEntered", seconds());
                hit(WINIT);
            }
            WindowEvent::CursorMoved { .. } => hit(WINIT),
            WindowEvent::CursorLeft { .. } => println!("{:8.3} winit CursorLeft", seconds()),
            WindowEvent::Focused(focused) => println!("{:8.3} winit Focused({focused})", seconds()),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            return;
        }
        let now = Instant::now();
        let poll = self.stage == Stage::Armed && in_strip();
        if self.stage == Stage::Armed {
            self.track_approach();
        }
        if poll {
            hit(POLL);
        }
        match self.stage {
            Stage::Park if now >= self.deadline => {
                self.park();
                self.stage = Stage::Armed;
                self.armed_at = now;
                self.deadline = now + if self.auto { Duration::from_millis(900) } else { Duration::from_secs(1800) };
                if self.auto {
                    self.warp_into_strip();
                }
            }
            Stage::Armed => {
                let any = HITS.iter().any(|h| h.load(Ordering::Relaxed) > 0);
                // Give the slower detectors a moment to report before the window moves.
                if any && now.duration_since(self.armed_at) > Duration::from_millis(150) {
                    std::thread::sleep(Duration::from_millis(250));
                    self.reveal();
                    self.stage = Stage::Revealed;
                    self.deadline = now + Duration::from_millis(900);
                } else if now >= self.deadline {
                    println!("{:8.3} nothing fired on this edge", seconds());
                    self.next_edge(event_loop);
                }
            }
            Stage::Revealed if !self.auto || now >= self.deadline => {
                if self.auto {
                    let _ = unsafe { CGWarpMouseCursorPosition(CGPoint { x: 760.0, y: 700.0 }) };
                }
                self.stage = Stage::Leaving;
                self.outside_since = None;
            }
            Stage::Leaving => {
                if self.pointer_outside_window() {
                    let since = *self.outside_since.get_or_insert(now);
                    if now.duration_since(since) >= Duration::from_millis(500) {
                        println!("{:8.3} left for 500 ms, app_active={}", seconds(), self.active());
                        self.next_edge(event_loop);
                    }
                } else {
                    self.outside_since = None;
                }
            }
            Stage::CostPoll20 => {
                let _ = pointer();
                if now >= self.deadline {
                    println!("poll at 20 Hz: cpu={:.3}%", (cpu_seconds() - self.cpu) / 20.0 * 100.0);
                    self.stage = Stage::CostPoll10;
                    self.deadline = now + Duration::from_secs(20);
                    self.cpu = cpu_seconds();
                }
            }
            Stage::CostPoll10 => {
                let _ = pointer();
                if now >= self.deadline {
                    println!("poll at 10 Hz: cpu={:.3}%", (cpu_seconds() - self.cpu) / 20.0 * 100.0);
                    println!("auto run finished: {:?}", self.results);
                    self.stage = Stage::Done;
                    event_loop.exit();
                }
            }
            _ => {}
        }
        let tick = match self.stage {
            Stage::CostPoll20 => Duration::from_millis(50),
            Stage::CostPoll10 => Duration::from_millis(100),
            _ => Duration::from_millis(16),
        };
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + tick));
    }
}

fn main() {
    let auto = std::env::args().nth(1).as_deref() != Some("guided");
    // `hover_probe guided left,top` tests only those edges.
    let edges: Vec<Edge> = match std::env::args().nth(2) {
        Some(list) => list
            .split(',')
            .map(|name| match name {
                "left" => Edge::Left,
                "right" => Edge::Right,
                "top" => Edge::Top,
                other => panic!("unknown edge {other}"),
            })
            .collect(),
        None => EDGES.to_vec(),
    };
    let event_loop = EventLoop::builder().with_activation_policy(ActivationPolicy::Accessory).with_default_menu(false).build().unwrap();
    let now = Instant::now();
    let mut app = App {
        auto,
        window: None,
        surface: None,
        owner: None,
        monitor: None,
        index: 0,
        stage: Stage::Park,
        deadline: now,
        armed_at: now,
        outside_since: None,
        home: (0.0, 0.0),
        cpu: 0.0,
        results: Vec::new(),
        edges,
        closest: f64::MAX,
        crossed: 0,
        last_y: 0.0,
    };
    seconds();
    event_loop.run_app(&mut app).unwrap();
}
