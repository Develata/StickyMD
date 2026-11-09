//! Caret blink cost, driven by event-loop timers so every change is committed when the loop turns.
//! Heap numbers count the Rust allocator only; AppKit and Core Animation allocate outside it, and
//! WindowServer's own work is not in this process's CPU time.
use std::alloc::{GlobalAlloc, Layout, System};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId};

struct Counting;
static ALLOCATED: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

unsafe extern "C" {
    fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut u64) -> i32;
}

fn footprint_mib() -> f64 {
    let mut info = [0u64; 64];
    assert_eq!(unsafe { proc_pid_rusage(std::process::id() as i32, 4, info.as_mut_ptr()) }, 0);
    info[9] as f64 / 1024.0 / 1024.0
}

fn cpu_seconds() -> f64 {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    let time = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1e6;
    time(usage.ru_utime) + time(usage.ru_stime)
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Settle1,
    Idle,
    FullFrames,
    Settle2,
    Layer,
    Done,
}

const WINDOW: Duration = Duration::from_secs(30);
const BLINK: Duration = Duration::from_millis(530);

struct App {
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    layer: Option<Retained<AnyObject>>,
    phase: Phase,
    phase_end: Instant,
    next_blink: Instant,
    cpu: f64,
    heap: u64,
    blinks: u64,
    committed: u64,
    state: bool,
    footprint: f64,
}

impl App {
    fn begin(&mut self, phase: Phase, length: Duration) {
        self.phase = phase;
        self.phase_end = Instant::now() + length;
        self.next_blink = Instant::now();
        self.cpu = cpu_seconds();
        self.heap = ALLOCATED.load(Ordering::Relaxed);
        self.blinks = 0;
        self.committed = 0;
        self.footprint = footprint_mib();
    }

    fn report(&self, label: &str, length: Duration) {
        let cpu = (cpu_seconds() - self.cpu) / length.as_secs_f64() * 100.0;
        let heap = ALLOCATED.load(Ordering::Relaxed) - self.heap;
        println!(
            "{label}: cpu={cpu:.3}% blinks={} rust_heap_per_blink={} committed_ticks={} footprint {:.1}->{:.1} MiB",
            self.blinks,
            heap.checked_div(self.blinks).unwrap_or(0),
            self.committed,
            self.footprint,
            footprint_mib()
        );
    }

    fn view(&self) -> *mut AnyObject {
        let window = self.window.as_ref().unwrap();
        let RawWindowHandle::AppKit(handle) = window.window_handle().unwrap().as_raw() else {
            panic!("not AppKit")
        };
        handle.ns_view.as_ptr().cast()
    }

    fn present(&mut self, on: bool) {
        let surface = self.surface.as_mut().unwrap();
        let mut buffer = surface.buffer_mut().unwrap();
        buffer.fill(if on { 0x00ee_eeee } else { 0x00ed_edee });
        buffer.present().unwrap();
    }

    fn add_layer(&mut self) {
        unsafe {
            let root: *mut AnyObject = msg_send![self.view(), layer];
            let layer: Retained<AnyObject> = msg_send![class!(CALayer), new];
            let frame = NSRect::new(NSPoint::new(40.0, 40.0), NSSize::new(1.0, 20.0));
            let _: () = msg_send![&*layer, setFrame: frame];
            let filter = NSString::from_str("differenceBlendMode");
            let _: () = msg_send![&*layer, setCompositingFilter: &*filter];
            let color: *mut AnyObject = msg_send![class!(NSColor), whiteColor];
            let cg: *mut AnyObject = msg_send![color, CGColor];
            let _: () = msg_send![&*layer, setBackgroundColor: cg];
            let _: () = msg_send![root, addSublayer: &*layer];
            self.layer = Some(layer);
        }
    }

    fn toggle_layer(&mut self) {
        let layer = self.layer.clone().unwrap();
        // The previous change reached the render tree if the presentation layer shows it.
        let shown: *mut AnyObject = unsafe { msg_send![&*layer, presentationLayer] };
        if !shown.is_null() {
            let hidden: bool = unsafe { msg_send![shown, isHidden] };
            if hidden == self.state {
                self.committed += 1;
            }
        }
        self.state = !self.state;
        unsafe {
            let _: () = msg_send![class!(CATransaction), begin];
            let _: () = msg_send![class!(CATransaction), setDisableActions: Bool::YES];
            let _: () = msg_send![&*layer, setHidden: Bool::new(self.state)];
            let _: () = msg_send![class!(CATransaction), commit];
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_decorations(false).with_inner_size(LogicalSize::new(520.0, 680.0));
        let window = Rc::new(event_loop.create_window(attributes).unwrap());
        let size = window.inner_size();
        let context = softbuffer::Context::new(window.clone()).unwrap();
        let mut surface = softbuffer::Surface::new(&context, window.clone()).unwrap();
        surface.resize(NonZeroU32::new(size.width).unwrap(), NonZeroU32::new(size.height).unwrap()).unwrap();
        self.surface = Some(surface);
        self.window = Some(window);
        self.present(true);
        self.begin(Phase::Settle1, Duration::from_secs(10));
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, _event: WindowEvent) {}

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            return;
        }
        let now = Instant::now();
        if now >= self.phase_end {
            match self.phase {
                Phase::Settle1 => self.begin(Phase::Idle, WINDOW),
                Phase::Idle => {
                    self.report("idle, timer loop only", WINDOW);
                    self.begin(Phase::FullFrames, WINDOW);
                }
                Phase::FullFrames => {
                    self.report("caret as full frames", WINDOW);
                    self.add_layer();
                    self.begin(Phase::Settle2, Duration::from_secs(10));
                }
                Phase::Settle2 => self.begin(Phase::Layer, WINDOW),
                Phase::Layer => {
                    self.report("caret as sublayer toggle", WINDOW);
                    self.phase = Phase::Done;
                }
                Phase::Done => event_loop.exit(),
            }
        }
        let now = Instant::now();
        if now >= self.next_blink && matches!(self.phase, Phase::FullFrames | Phase::Layer) {
            self.next_blink = now + BLINK;
            self.blinks += 1;
            if self.phase == Phase::FullFrames {
                self.state = !self.state;
                self.present(self.state);
            } else {
                self.toggle_layer();
            }
        }
        let wake = if matches!(self.phase, Phase::FullFrames | Phase::Layer) { self.next_blink.min(self.phase_end) } else { self.phase_end };
        event_loop.set_control_flow(ControlFlow::WaitUntil(wake));
    }
}

fn main() {
    let event_loop = EventLoop::builder().with_activation_policy(ActivationPolicy::Accessory).with_default_menu(false).build().unwrap();
    let now = Instant::now();
    let mut app = App { window: None, surface: None, layer: None, phase: Phase::Settle1, phase_end: now, next_blink: now, cpu: 0.0, heap: 0, blinks: 0, committed: 0, state: false, footprint: 0.0 };
    event_loop.run_app(&mut app).unwrap();
}
