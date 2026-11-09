//! Top-edge collapse without the notch and without sitting behind the menu bar: the one window
//! shrinks to a small shape just below the menu bar. Two reveals each for a narrow bar and a small
//! square, with the shell's real timing: 100 ms hover, 500 ms leave, 140 ms ease-out animation.
//! `tab_probe` waits for a real mouse and ends by itself.
use std::num::NonZeroU32;
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId, WindowLevel};

const EXPANDED: Rect = Rect { x: 400.0, y: 33.0, w: 320.0, h: 240.0 };
const STYLES: [(&str, f64, f64); 2] = [("narrow bar", 120.0, 6.0), ("small square", 28.0, 28.0)];
// `tab_probe edge`: touching the top screen edge, above the menu bar, and translucent.
const EDGE_STYLES: [(&str, f64, f64); 2] = [("translucent top-edge bar", 120.0, 6.0), ("translucent top-edge square", 33.0, 33.0)];
// `tab_probe strip`: as on Windows. The window keeps its size and moves up until 3 points show.
const STRIP_STYLES: [(&str, f64, f64); 2] = [("full-width top-edge strip", EXPANDED.w, 3.0), ("full-width top-edge strip", EXPANDED.w, 3.0)];
const COLLAPSED_ALPHA: f64 = 0.35;
const ABOVE_MENU_BAR: isize = 27;
const FLOATING: isize = 3;
const REVEALS_PER_STYLE: usize = 2;
const ANIMATION: Duration = Duration::from_millis(140);
const HOVER: Duration = Duration::from_millis(100);
const LEAVE: Duration = Duration::from_millis(500);
// canJoinAllSpaces | fullScreenAuxiliary
const BEHAVIOR: usize = 1 | (1 << 8);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    fn lerp(self, to: Rect, t: f64) -> Rect {
        Rect {
            x: self.x + (to.x - self.x) * t,
            y: self.y + (to.y - self.y) * t,
            w: self.w + (to.w - self.w) * t,
            h: self.h + (to.h - self.h) * t,
        }
    }

    fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    Starting,
    Collapsing,
    Collapsed,
    Expanding,
    Expanded,
    Finished,
}

struct App {
    edge: bool,
    strip: bool,
    thickness: f64,
    styles: &'static [(&'static str, f64, f64)],
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    screen_height: f64,
    stage: Stage,
    style: usize,
    reveals: usize,
    from: Rect,
    to: Rect,
    animation_start: Instant,
    frames: u32,
    slowest: f64,
    hover_since: Option<Instant>,
    outside_since: Option<Instant>,
    collapsed_at: Instant,
    last_y: f64,
    crossed: u32,
    started: Instant,
    next: Instant,
    give_up: Instant,
}

impl App {
    fn at(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    fn ns_window(&self) -> *mut AnyObject {
        let window = self.window.as_ref().unwrap();
        let RawWindowHandle::AppKit(handle) = window.window_handle().unwrap().as_raw() else {
            panic!("not AppKit")
        };
        let view: *mut AnyObject = handle.ns_view.as_ptr().cast();
        unsafe { msg_send![view, window] }
    }

    fn active(&self) -> bool {
        let app: *mut AnyObject = unsafe { msg_send![class!(NSApplication), sharedApplication] };
        unsafe { msg_send![app, isActive] }
    }

    fn collapsed_rect(&self) -> Rect {
        if self.strip {
            return Rect { x: EXPANDED.x, y: self.thickness - EXPANDED.h, w: EXPANDED.w, h: EXPANDED.h };
        }
        let (_, w, h) = self.styles[self.style];
        Rect { x: EXPANDED.x + (EXPANDED.w - w) / 2.0, y: if self.edge { 0.0 } else { EXPANDED.y }, w, h }
    }

    fn set_level(&self, level: isize) {
        let ns = self.ns_window();
        let _: () = unsafe { msg_send![ns, setLevel: level] };
    }

    fn set_alpha(&self, alpha: f64) {
        let ns = self.ns_window();
        let _: () = unsafe { msg_send![ns, setAlphaValue: alpha] };
    }

    /// One atomic move and resize, as the host's `apply_frame` would do it.
    fn apply(&mut self, rect: Rect) {
        let frame = NSRect::new(NSPoint::new(rect.x, self.screen_height - rect.y - rect.h), NSSize::new(rect.w, rect.h));
        let ns = self.ns_window();
        let _: () = unsafe { msg_send![ns, setFrame: frame, display: Bool::NO] };
        let window = self.window.clone().unwrap();
        let size = window.inner_size();
        let surface = self.surface.as_mut().unwrap();
        surface.resize(NonZeroU32::new(size.width.max(1)).unwrap(), NonZeroU32::new(size.height.max(1)).unwrap()).unwrap();
        let mut buffer = surface.buffer_mut().unwrap();
        buffer.fill(0x00ff_d60a);
        buffer.present().unwrap();
    }

    fn animate(&mut self, from: Rect, to: Rect, stage: Stage) {
        if self.edge && stage == Stage::Collapsing {
            self.set_level(ABOVE_MENU_BAR);
        }
        self.from = from;
        self.to = to;
        self.stage = stage;
        self.animation_start = Instant::now();
        self.frames = 0;
        self.slowest = 0.0;
    }

    fn pointer(&self) -> (f64, f64) {
        let point: NSPoint = unsafe { msg_send![class!(NSEvent), mouseLocation] };
        (point.x, self.screen_height - point.y)
    }

    fn finish_animation(&mut self) {
        let window = self.window.clone().unwrap();
        let outer = window.outer_position().unwrap().to_logical::<f64>(window.scale_factor());
        let size = window.outer_size().to_logical::<f64>(window.scale_factor());
        println!(
            "{:8.3} {:?} done: frames={} slowest_step={:.2} ms read=({:.0},{:.0} {:.0}x{:.0}) app_active={}",
            self.at(), self.stage, self.frames, self.slowest, outer.x, outer.y, size.width, size.height, self.active()
        );
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_decorations(false).with_inner_size(LogicalSize::new(EXPANDED.w, EXPANDED.h));
        let window = Rc::new(event_loop.create_window(attributes).unwrap());
        window.set_window_level(WindowLevel::AlwaysOnTop);
        self.surface = Some(softbuffer::Surface::new(&softbuffer::Context::new(window.clone()).unwrap(), window.clone()).unwrap());
        self.window = Some(window);
        let screen: *mut AnyObject = unsafe { msg_send![class!(NSScreen), mainScreen] };
        let frame: NSRect = unsafe { msg_send![screen, frame] };
        self.screen_height = frame.size.height;
        let ns = self.ns_window();
        let _: () = unsafe { msg_send![ns, setCollectionBehavior: BEHAVIOR] };
        self.apply(EXPANDED);
        let _ = Command::new("/usr/bin/osascript").args(["-e", "tell application \"Finder\" to activate"]).status();
        self.next = Instant::now() + Duration::from_millis(900);
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CursorEntered { .. } => {
                println!("{:8.3} CursorEntered in {:?}", self.at(), self.stage);
                if self.stage == Stage::Collapsed {
                    self.hover_since = Some(Instant::now());
                }
            }
            WindowEvent::CursorLeft { .. } => {
                println!("{:8.3} CursorLeft in {:?}", self.at(), self.stage);
                if self.stage == Stage::Collapsed {
                    self.hover_since = None;
                }
            }
            WindowEvent::Focused(focused) => println!("{:8.3} Focused({focused})", self.at()),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() || self.stage == Stage::Finished {
            return;
        }
        let now = Instant::now();
        match self.stage {
            Stage::Starting if now >= self.next => {
                let to = self.collapsed_rect();
                println!("{:8.3} collapsing to {}", self.at(), self.styles[self.style].0);
                self.animate(EXPANDED, to, Stage::Collapsing);
            }
            Stage::Collapsing | Stage::Expanding => {
                let t = (now.duration_since(self.animation_start).as_secs_f64() / ANIMATION.as_secs_f64()).min(1.0);
                let eased = 1.0 - (1.0 - t).powi(3);
                let step = Instant::now();
                self.apply(self.from.lerp(self.to, eased));
                if self.edge && !self.strip {
                    let collapsed = if self.stage == Stage::Collapsing { eased } else { 1.0 - eased };
                    self.set_alpha(1.0 + (COLLAPSED_ALPHA - 1.0) * collapsed);
                }
                self.frames += 1;
                self.slowest = self.slowest.max(step.elapsed().as_secs_f64() * 1000.0);
                if t >= 1.0 {
                    self.finish_animation();
                    if self.stage == Stage::Collapsing {
                        self.stage = Stage::Collapsed;
                        self.hover_since = None;
                        self.collapsed_at = Instant::now();
                        self.crossed = 0;
                        self.last_y = 0.0;
                    } else {
                        if self.edge {
                            self.set_level(FLOATING);
                        }
                        self.stage = Stage::Expanded;
                        self.outside_since = None;
                    }
                }
            }
            Stage::Collapsed => {
                let rect = self.collapsed_rect();
                let (x, y) = self.pointer();
                if x >= rect.x && x <= rect.x + rect.w && self.last_y > rect.y + rect.h && y < rect.y {
                    self.crossed += 1;
                }
                self.last_y = y;
                if self.hover_since.is_some_and(|since| now.duration_since(since) >= HOVER) {
                    self.reveals += 1;
                    println!(
                        "{:8.3} {} reveal {}: waited {:.1} s, crossed_without_landing={}, app_active={}",
                        self.at(), self.styles[self.style].0, self.reveals,
                        now.duration_since(self.collapsed_at).as_secs_f64(), self.crossed, self.active()
                    );
                    self.animate(rect, EXPANDED, Stage::Expanding);
                }
            }
            Stage::Expanded => {
                let (x, y) = self.pointer();
                if EXPANDED.contains(x, y) {
                    self.outside_since = None;
                } else {
                    let since = *self.outside_since.get_or_insert(now);
                    if now.duration_since(since) >= LEAVE {
                        if self.reveals >= REVEALS_PER_STYLE {
                            self.reveals = 0;
                            self.style += 1;
                            if self.style == self.styles.len() {
                                println!("{:8.3} finished", self.at());
                                self.stage = Stage::Finished;
                                event_loop.exit();
                                return;
                            }
                        }
                        let to = self.collapsed_rect();
                        println!("{:8.3} collapsing to {}", self.at(), self.styles[self.style].0);
                        self.animate(EXPANDED, to, Stage::Collapsing);
                    }
                }
            }
            _ => {}
        }
        if now >= self.give_up {
            println!("{:8.3} gave up", self.at());
            self.stage = Stage::Finished;
            event_loop.exit();
            return;
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(8)));
    }
}

fn main() {
    let event_loop = EventLoop::builder().with_activation_policy(ActivationPolicy::Accessory).with_default_menu(false).build().unwrap();
    let now = Instant::now();
    let mode = std::env::args().nth(1);
    let strip = mode.as_deref() == Some("strip");
    let edge = strip || mode.as_deref() == Some("edge");
    // `tab_probe strip 2` leaves 2 points showing.
    let thickness = std::env::args().nth(2).and_then(|value| value.parse().ok()).unwrap_or(3.0);
    let mut app = App {
        edge,
        strip,
        thickness,
        styles: if strip {
            &STRIP_STYLES
        } else if edge {
            &EDGE_STYLES
        } else {
            &STYLES
        },
        window: None,
        surface: None,
        screen_height: 0.0,
        stage: Stage::Starting,
        style: 0,
        reveals: 0,
        from: EXPANDED,
        to: EXPANDED,
        animation_start: now,
        frames: 0,
        slowest: 0.0,
        hover_since: None,
        outside_since: None,
        collapsed_at: now,
        last_y: 0.0,
        crossed: 0,
        started: now,
        next: now,
        give_up: now + Duration::from_secs(1800),
    };
    event_loop.run_app(&mut app).unwrap();
}
