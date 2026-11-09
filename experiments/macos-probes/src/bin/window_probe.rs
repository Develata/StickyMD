//! Drives one borderless winit window through what the StickyMD host needs and prints what AppKit
//! reports back. `window_probe [tray-before-loop]`. Runs about a minute, needs a logged-in desktop.
use std::alloc::{GlobalAlloc, Layout, System};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId, WindowLevel};

struct Counting;
static ALLOCATED: AtomicU64 = AtomicU64::new(0);
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size() as u64, Ordering::Relaxed);
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size() as u64, Ordering::Relaxed);
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
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

/// ri_phys_footprint of rusage_info_v4, in MiB.
fn footprint_mib() -> f64 {
    let mut info = [0u64; 64];
    let result = unsafe { proc_pid_rusage(std::process::id() as i32, 4, info.as_mut_ptr()) };
    assert_eq!(result, 0);
    info[9] as f64 / 1024.0 / 1024.0
}

fn cpu_seconds() -> f64 {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    let time = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1e6;
    time(usage.ru_utime) + time(usage.ru_stime)
}

fn percentile(samples: &mut [f64], p: f64) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[((samples.len() - 1) as f64 * p).round() as usize]
}

fn ns_app() -> *mut AnyObject {
    unsafe { msg_send![class!(NSApplication), sharedApplication] }
}

fn icon() -> Icon {
    Icon::from_rgba(vec![200u8; 32 * 32 * 4], 32, 32).unwrap()
}

fn make_tray() -> Result<TrayIcon, tray_icon::Error> {
    TrayIconBuilder::new().with_icon(icon()).with_tooltip("probe").build()
}

#[derive(Debug)]
enum Probe {
    RunModal,
    FromWorker(u32),
}

struct App {
    proxy: EventLoopProxy<Probe>,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    tray: Option<TrayIcon>,
    tray_before: bool,
    step: usize,
    next: Instant,
    in_modal: bool,
    worker_delivered_in_modal: u32,
    worker_delivered: u32,
    focus_events: Vec<(bool, f64)>,
    started: Instant,
}

impl App {
    fn ns_window(&self) -> *mut AnyObject {
        let window = self.window.as_ref().unwrap();
        let RawWindowHandle::AppKit(handle) = window.window_handle().unwrap().as_raw() else {
            panic!("not AppKit")
        };
        let view = handle.ns_view.as_ptr().cast::<AnyObject>();
        unsafe { msg_send![view, window] }
    }

    fn frame(&self) -> NSRect {
        unsafe { msg_send![self.ns_window(), frame] }
    }

    fn report_placement(&self, label: &str, requested: (f64, f64)) {
        let window = self.window.as_ref().unwrap();
        let scale = window.scale_factor();
        let outer = window.outer_position().unwrap().to_logical::<f64>(scale);
        let frame = self.frame();
        println!(
            "place {label}: requested=({:.0},{:.0}) winit_read=({:.1},{:.1}) ns_frame=origin({:.1},{:.1}) size({:.1},{:.1})",
            requested.0, requested.1, outer.x, outer.y, frame.origin.x, frame.origin.y, frame.size.width, frame.size.height
        );
    }

    fn screens(&self) -> (f64, f64) {
        let screens: *mut AnyObject = unsafe { msg_send![class!(NSScreen), screens] };
        let count: usize = unsafe { msg_send![screens, count] };
        let mut main_size = (0.0, 0.0);
        for index in 0..count {
            let screen: *mut AnyObject = unsafe { msg_send![screens, objectAtIndex: index] };
            let frame: NSRect = unsafe { msg_send![screen, frame] };
            let visible: NSRect = unsafe { msg_send![screen, visibleFrame] };
            let scale: f64 = unsafe { msg_send![screen, backingScaleFactor] };
            println!(
                "screen {index}: frame=({},{} {}x{}) visible=({},{} {}x{}) scale={scale}",
                frame.origin.x, frame.origin.y, frame.size.width, frame.size.height,
                visible.origin.x, visible.origin.y, visible.size.width, visible.size.height
            );
            if index == 0 {
                main_size = (frame.size.width, frame.size.height);
            }
        }
        main_size
    }

    fn frame_cost(&mut self, label: &str, count: usize) {
        let window = self.window.clone().unwrap();
        let size = window.inner_size();
        let surface = self.surface.as_mut().unwrap();
        surface
            .resize(NonZeroU32::new(size.width).unwrap(), NonZeroU32::new(size.height).unwrap())
            .unwrap();
        let bytes_before = ALLOCATED.load(Ordering::Relaxed);
        let calls_before = ALLOCATIONS.load(Ordering::Relaxed);
        let footprint_before = footprint_mib();
        let (mut acquire, mut present, mut age_sum) = (Vec::new(), Vec::new(), 0u64);
        for frame in 0..count {
            let start = Instant::now();
            let mut buffer = surface.buffer_mut().unwrap();
            acquire.push(start.elapsed().as_secs_f64() * 1000.0);
            age_sum += u64::from(buffer.age());
            let color = if frame % 2 == 0 { 0x00ee_eeee } else { 0x00dd_dddd };
            buffer.fill(color);
            let start = Instant::now();
            buffer.present().unwrap();
            present.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        let per_frame = (ALLOCATED.load(Ordering::Relaxed) - bytes_before) as f64 / count as f64;
        let calls = (ALLOCATIONS.load(Ordering::Relaxed) - calls_before) as f64 / count as f64;
        println!(
            "frames {label}: physical={}x{} expected_buffer={} B heap_per_frame={per_frame:.0} B allocs_per_frame={calls:.1} age_sum={age_sum} buffer_mut p50={:.3} p95={:.3} ms present p50={:.3} p95={:.3} ms footprint {:.1}->{:.1} MiB",
            size.width,
            size.height,
            size.width as u64 * size.height as u64 * 4,
            percentile(&mut acquire, 0.5),
            percentile(&mut acquire, 0.95),
            percentile(&mut present, 0.5),
            percentile(&mut present, 0.95),
            footprint_before,
            footprint_mib()
        );
    }

    /// One blink: the whole window as a new frame, as softbuffer's AppKit backend must do it.
    fn present_once(&mut self, on: bool) {
        let surface = self.surface.as_mut().unwrap();
        let mut buffer = surface.buffer_mut().unwrap();
        buffer.fill(if on { 0x00ee_eeee } else { 0x00ed_edee });
        buffer.present().unwrap();
    }

    fn caret_layer(&self) -> Retained<AnyObject> {
        let window = self.window.as_ref().unwrap();
        let RawWindowHandle::AppKit(handle) = window.window_handle().unwrap().as_raw() else {
            panic!("not AppKit")
        };
        let view = handle.ns_view.as_ptr().cast::<AnyObject>();
        unsafe {
            let root: *mut AnyObject = msg_send![view, layer];
            let layer: Retained<AnyObject> = msg_send![class!(CALayer), new];
            let frame = NSRect::new(NSPoint::new(40.0, 40.0), NSSize::new(1.0, 20.0));
            let _: () = msg_send![&*layer, setFrame: frame];
            let filter = NSString::from_str("differenceBlendMode");
            let _: () = msg_send![&*layer, setCompositingFilter: &*filter];
            let color: *mut AnyObject = msg_send![class!(NSColor), whiteColor];
            let cg: *mut AnyObject = msg_send![color, CGColor];
            let _: () = msg_send![&*layer, setBackgroundColor: cg];
            let _: () = msg_send![root, addSublayer: &*layer];
            layer
        }
    }

    fn blink_scenarios(&mut self) {
        // Each scenario holds the main thread on purpose: only this process's own cost is read.
        let duration = Duration::from_secs(12);
        let interval = Duration::from_millis(530);
        let cpu_idle = cpu_seconds();
        std::thread::sleep(duration);
        println!("cpu idle: {:.3}%", (cpu_seconds() - cpu_idle) / duration.as_secs_f64() * 100.0);

        let footprint = footprint_mib();
        let cpu = cpu_seconds();
        let bytes = ALLOCATED.load(Ordering::Relaxed);
        let start = Instant::now();
        let mut on = false;
        let mut blinks = 0u64;
        while start.elapsed() < duration {
            on = !on;
            self.present_once(on);
            blinks += 1;
            std::thread::sleep(interval);
        }
        println!(
            "cpu caret as full frames: {:.3}% blinks={blinks} heap_bytes_per_blink={} footprint {:.1}->{:.1} MiB",
            (cpu_seconds() - cpu) / duration.as_secs_f64() * 100.0,
            (ALLOCATED.load(Ordering::Relaxed) - bytes) / blinks,
            footprint,
            footprint_mib()
        );

        let layer = self.caret_layer();
        let cpu = cpu_seconds();
        let bytes = ALLOCATED.load(Ordering::Relaxed);
        let start = Instant::now();
        let mut blinks = 0u64;
        let mut hidden = false;
        while start.elapsed() < duration {
            hidden = !hidden;
            unsafe {
                let _: () = msg_send![class!(CATransaction), begin];
                let _: () = msg_send![class!(CATransaction), setDisableActions: Bool::YES];
                let _: () = msg_send![&*layer, setHidden: Bool::new(hidden)];
                let _: () = msg_send![class!(CATransaction), commit];
            }
            blinks += 1;
            std::thread::sleep(interval);
        }
        println!(
            "cpu caret as sublayer toggle: {:.3}% blinks={blinks} heap_bytes_per_blink={}",
            (cpu_seconds() - cpu) / duration.as_secs_f64() * 100.0,
            (ALLOCATED.load(Ordering::Relaxed) - bytes) / blinks
        );
        unsafe {
            let _: () = msg_send![&*layer, removeFromSuperlayer];
        }
    }

    fn run_modal(&mut self) {
        // A modal session inside the event handler, as an export dialog would be. A worker keeps
        // posting and the window is resized meanwhile; the alert closes itself.
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            for index in 0..6 {
                std::thread::sleep(Duration::from_millis(100));
                let _ = proxy.send_event(Probe::FromWorker(index));
            }
        });
        let delivered_before = self.worker_delivered;
        let window = self.window.clone().unwrap();
        self.in_modal = true;
        unsafe {
            let app = ns_app();
            let alert: Retained<AnyObject> = msg_send![class!(NSAlert), new];
            let text = NSString::from_str("probe: closes itself");
            let _: () = msg_send![&*alert, setMessageText: &*text];
            let modes = objc2_foundation::NSArray::from_retained_slice(&[NSString::from_str("NSModalPanelRunLoopMode")]);
            let selector = objc2::sel!(abortModal);
            let nil: *mut AnyObject = std::ptr::null_mut();
            let _: () = msg_send![
                app,
                performSelector: selector,
                withObject: nil,
                afterDelay: 1.0f64,
                inModes: &*modes
            ];
            let _ = window.request_inner_size(LogicalSize::new(540.0, 700.0));
            let response: isize = msg_send![&*alert, runModal];
            println!("modal returned {response}");
        }
        self.in_modal = false;
        println!(
            "modal: no panic; worker events delivered during the modal={} (before+during total {})",
            self.worker_delivered_in_modal,
            self.worker_delivered - delivered_before
        );
    }

    fn advance(&mut self, event_loop: &ActiveEventLoop) {
        let window = self.window.clone().unwrap();
        let size = window.outer_size().to_logical::<f64>(window.scale_factor());
        let (screen_w, screen_h) = (self.screens_cached_w(), self.screens_cached_h());
        let wait = Duration::from_millis(250);
        match self.step {
            0 => {
                let (x, y) = (100.0, 120.0);
                window.set_outer_position(LogicalPosition::new(x, y));
                self.next = Instant::now() + wait;
            }
            1 => self.report_placement("on screen", (100.0, 120.0)),
            2 => window.set_outer_position(LogicalPosition::new(3.0 - size.width, 120.0)),
            3 => self.report_placement("off left leaving 3", (3.0 - size.width, 120.0)),
            4 => window.set_outer_position(LogicalPosition::new(screen_w - 3.0, 120.0)),
            5 => self.report_placement("off right leaving 3", (screen_w - 3.0, 120.0)),
            6 => window.set_outer_position(LogicalPosition::new(300.0, 3.0 - size.height)),
            7 => self.report_placement("off top leaving 3", (300.0, 3.0 - size.height)),
            8 => window.set_outer_position(LogicalPosition::new(300.0, 0.0)),
            9 => self.report_placement("under the menu bar y=0", (300.0, 0.0)),
            10 => window.set_outer_position(LogicalPosition::new(300.0, screen_h - 3.0)),
            11 => self.report_placement("off bottom leaving 3", (300.0, screen_h - 3.0)),
            12 => window.set_outer_position(LogicalPosition::new(200.0, 160.0)),
            13 => {
                let read = |w: *mut AnyObject| -> isize { unsafe { msg_send![w, level] } };
                let ns = self.ns_window();
                let normal = read(ns);
                window.set_window_level(WindowLevel::AlwaysOnTop);
                let top = read(ns);
                window.set_window_level(WindowLevel::Normal);
                println!("level: normal={normal} always_on_top={top} back={}", read(ns));
            }
            14 => {
                let visible = |w: *mut AnyObject| -> bool { unsafe { msg_send![w, isVisible] } };
                let ns = self.ns_window();
                let before = (visible(ns), window.is_visible());
                window.set_visible(false);
                let hidden = (visible(ns), window.is_visible());
                window.set_visible(true);
                println!("visibility: before={before:?} hidden={hidden:?} shown=({},{:?}) minimized={:?}", visible(ns), window.is_visible(), window.is_minimized());
            }
            15 => unsafe {
                let ns = self.ns_window();
                let _: () = msg_send![ns, setAlphaValue: 0.4f64];
                let alpha: f64 = msg_send![ns, alphaValue];
                let opaque: bool = msg_send![ns, isOpaque];
                let _: () = msg_send![ns, setAlphaValue: 1.0f64];
                println!("alpha: set 0.4 read {alpha} isOpaque={opaque} back={}", { let a: f64 = msg_send![ns, alphaValue]; a });
            },
            16 => unsafe {
                let app = ns_app();
                let ns = self.ns_window();
                let before: (bool, bool) = (msg_send![app, isActive], msg_send![ns, isKeyWindow]);
                window.focus_window();
                let after: (bool, bool) = (msg_send![app, isActive], msg_send![ns, isKeyWindow]);
                println!("focus_window: (app_active, key) before={before:?} immediately_after={after:?}");
                self.next = Instant::now() + Duration::from_millis(800);
            },
            17 => unsafe {
                let app = ns_app();
                let ns = self.ns_window();
                let after: (bool, bool) = (msg_send![app, isActive], msg_send![ns, isKeyWindow]);
                println!("focus_window: after 800 ms={after:?} focus_events={:?}", self.focus_events);
                let behavior: usize = msg_send![ns, collectionBehavior];
                println!("collectionBehavior default={behavior:#x}");
                let _: () = msg_send![ns, setCollectionBehavior: 1usize | (1usize << 8)];
                let read: usize = msg_send![ns, collectionBehavior];
                println!("collectionBehavior canJoinAllSpaces|fullScreenAuxiliary read={read:#x}");
                let hides: bool = msg_send![ns, hidesOnDeactivate];
                println!("hidesOnDeactivate={hides}");
            },
            18 => {
                if let Some(tray) = &self.tray {
                    println!("tray rect: {:?}", tray.rect());
                }
                println!("footprint with window shown: {:.1} MiB", footprint_mib());
                let _ = self.proxy.send_event(Probe::RunModal);
            }
            19 => {
                window.set_outer_position(LogicalPosition::new(200.0, 160.0));
                let _ = window.request_inner_size(LogicalSize::new(520.0, 680.0));
                self.next = Instant::now() + Duration::from_millis(500);
            }
            20 => self.frame_cost("default 520x680", 240),
            21 => {
                let _ = window.request_inner_size(LogicalSize::new(900.0, 680.0));
                self.next = Instant::now() + Duration::from_millis(500);
            }
            22 => self.frame_cost("split 900x680", 240),
            23 => {
                let _ = window.request_inner_size(LogicalSize::new(520.0, 680.0));
                self.next = Instant::now() + Duration::from_millis(500);
            }
            24 => self.frame_cost("back to default", 60),
            25 => self.blink_scenarios(),
            _ => {
                println!("elapsed: {:.1} s", self.started.elapsed().as_secs_f64());
                event_loop.exit();
                return;
            }
        }
        self.step += 1;
        if self.next <= Instant::now() {
            self.next = Instant::now() + wait;
        }
    }

    fn screens_cached_w(&self) -> f64 {
        let screen: *mut AnyObject = unsafe { msg_send![class!(NSScreen), mainScreen] };
        let frame: NSRect = unsafe { msg_send![screen, frame] };
        frame.size.width
    }

    fn screens_cached_h(&self) -> f64 {
        let screen: *mut AnyObject = unsafe { msg_send![class!(NSScreen), mainScreen] };
        let frame: NSRect = unsafe { msg_send![screen, frame] };
        frame.size.height
    }
}

impl ApplicationHandler<Probe> for App {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::Init) && !self.tray_before {
            match make_tray() {
                Ok(tray) => {
                    println!("tray at Init: created, rect={:?}", tray.rect());
                    self.tray = Some(tray);
                }
                Err(error) => println!("tray at Init: {error}"),
            }
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("macos probe")
            .with_decorations(false)
            .with_inner_size(LogicalSize::new(520.0, 680.0));
        let window = Rc::new(event_loop.create_window(attributes).unwrap());
        let context = softbuffer::Context::new(window.clone()).unwrap();
        self.surface = Some(softbuffer::Surface::new(&context, window.clone()).unwrap());
        self.window = Some(window.clone());
        println!(
            "window: scale={} inner={:?} outer={:?} monitor={:?} activationPolicy={}",
            window.scale_factor(),
            window.inner_size(),
            window.outer_size(),
            window.current_monitor().and_then(|m| m.name()),
            { let p: isize = unsafe { msg_send![ns_app(), activationPolicy] }; p }
        );
        self.screens();
        self.present_once(true);
        self.next = Instant::now() + Duration::from_millis(500);
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next));
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let WindowEvent::Focused(focused) = event {
            self.focus_events.push((focused, self.started.elapsed().as_secs_f64()));
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: Probe) {
        match event {
            Probe::RunModal => self.run_modal(),
            Probe::FromWorker(_) => {
                self.worker_delivered += 1;
                if self.in_modal {
                    self.worker_delivered_in_modal += 1;
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() && Instant::now() >= self.next {
            self.advance(event_loop);
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next));
    }
}

fn main() {
    let tray_before = std::env::args().nth(1).as_deref() == Some("tray-before-loop");
    let event_loop = EventLoop::<Probe>::with_user_event()
        .with_activation_policy(ActivationPolicy::Accessory)
        .with_default_menu(false)
        .build()
        .unwrap();
    let tray = if tray_before {
        match make_tray() {
            Ok(tray) => {
                println!("tray before the loop: created, rect={:?}", tray.rect());
                Some(tray)
            }
            Err(error) => {
                println!("tray before the loop: {error}");
                None
            }
        }
    } else {
        None
    };
    let mut app = App {
        proxy: event_loop.create_proxy(),
        window: None,
        surface: None,
        tray,
        tray_before,
        step: 0,
        next: Instant::now(),
        in_modal: false,
        worker_delivered_in_modal: 0,
        worker_delivered: 0,
        focus_events: Vec::new(),
        started: Instant::now(),
    };
    event_loop.run_app(&mut app).unwrap();
    if let Some(tray) = &app.tray {
        println!("tray at the end: rect={:?}", tray.rect());
    }
}
