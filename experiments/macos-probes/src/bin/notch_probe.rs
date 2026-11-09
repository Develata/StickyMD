//! Can the notch be the sensor for a window docked at the top? A small black panel covers the
//! notch's dead zone above the menu bar; hovering it reveals a note window below the notch without
//! taking focus. `notch_probe auto` reports geometry and placement only; `notch_probe guided` waits
//! for a real mouse and ends after three reveals.
use std::num::NonZeroU32;
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Sel};
use objc2::{class, msg_send, sel};
use objc2_foundation::{NSEdgeInsets, NSPoint, NSRect};
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

const NOTE: (f64, f64) = (320.0, 240.0);
const PARKED_X: f64 = -2000.0;
const ABOVE_MENU_BAR: isize = 27; // kCGMainMenuWindowLevel (24) + 3
// canJoinAllSpaces | stationary | ignoresCycle | fullScreenAuxiliary
const BEHAVIOR: usize = 1 | (1 << 4) | (1 << 6) | (1 << 8);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Stage {
    Place,
    CheckPlain,
    CheckOverride,
    Waiting,
    Revealed,
    Finished,
}

struct App {
    guided: bool,
    sensor: Option<Rc<Window>>,
    note: Option<Rc<Window>>,
    surfaces: Vec<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    notch: (f64, f64, f64),
    screen_height: f64,
    stage: Stage,
    next: Instant,
    outside_since: Option<Instant>,
    reveals: u32,
    started: Instant,
    give_up: Instant,
}

fn ns_window(window: &Window) -> *mut AnyObject {
    let RawWindowHandle::AppKit(handle) = window.window_handle().unwrap().as_raw() else {
        panic!("not AppKit")
    };
    let view: *mut AnyObject = handle.ns_view.as_ptr().cast();
    unsafe { msg_send![view, window] }
}

fn paint(surface: &mut softbuffer::Surface<Rc<Window>, Rc<Window>>, window: &Window, color: u32) {
    let size = window.inner_size();
    surface.resize(NonZeroU32::new(size.width).unwrap(), NonZeroU32::new(size.height).unwrap()).unwrap();
    let mut buffer = surface.buffer_mut().unwrap();
    buffer.fill(color);
    buffer.present().unwrap();
}

impl App {
    fn at(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    fn active(&self) -> bool {
        let app: *mut AnyObject = unsafe { msg_send![class!(NSApplication), sharedApplication] };
        unsafe { msg_send![app, isActive] }
    }

    fn geometry(&mut self) {
        let screen: *mut AnyObject = unsafe { msg_send![class!(NSScreen), mainScreen] };
        unsafe {
            let frame: NSRect = msg_send![screen, frame];
            let visible: NSRect = msg_send![screen, visibleFrame];
            let insets: NSEdgeInsets = msg_send![screen, safeAreaInsets];
            let left: NSRect = msg_send![screen, auxiliaryTopLeftArea];
            let right: NSRect = msg_send![screen, auxiliaryTopRightArea];
            let x = left.origin.x + left.size.width;
            let width = right.origin.x - x;
            println!(
                "screen {}x{} visible top {:.0}; safeArea.top={} auxLeft=({},{} {}x{}) auxRight=({},{} {}x{}) notch x={x} width={width}",
                frame.size.width, frame.size.height,
                frame.size.height - (visible.origin.y + visible.size.height),
                insets.top,
                left.origin.x, left.origin.y, left.size.width, left.size.height,
                right.origin.x, right.origin.y, right.size.width, right.size.height,
            );
            self.notch = (x, width, insets.top);
            self.screen_height = frame.size.height;
        }
    }

    fn report_sensor(&self, label: &str) {
        let sensor = self.sensor.as_ref().unwrap();
        let ns = ns_window(sensor);
        unsafe {
            let frame: NSRect = msg_send![ns, frame];
            let level: isize = msg_send![ns, level];
            let on_space: bool = msg_send![ns, isOnActiveSpace];
            let occlusion: usize = msg_send![ns, occlusionState];
            println!(
                "{:8.3} [{label}] top_gap={:.1} x={:.1} size={:.1}x{:.1} level={level} on_active_space={on_space} occlusion_visible={} app_active={}",
                self.at(),
                self.screen_height - (frame.origin.y + frame.size.height),
                frame.origin.x, frame.size.width, frame.size.height,
                occlusion & 2 != 0, self.active()
            );
        }
    }

    fn place_sensor(&self) {
        let sensor = self.sensor.as_ref().unwrap();
        sensor.set_outer_position(LogicalPosition::new(self.notch.0, 0.0));
    }

    fn pointer(&self) -> (f64, f64) {
        let point: NSPoint = unsafe { msg_send![class!(NSEvent), mouseLocation] };
        (point.x, self.screen_height - point.y)
    }

    fn reveal(&mut self) {
        let note = self.note.as_ref().unwrap();
        note.set_outer_position(LogicalPosition::new(self.notch.0 + self.notch.1 / 2.0 - NOTE.0 / 2.0, self.notch.2));
        self.stage = Stage::Revealed;
        self.outside_since = None;
        self.reveals += 1;
        println!("{:8.3} reveal {}: app_active={} (must stay false)", self.at(), self.reveals, self.active());
    }

    fn pointer_outside(&self) -> bool {
        let (x, y) = self.pointer();
        let (notch_x, width, height) = self.notch;
        let in_notch = x >= notch_x && x <= notch_x + width && y <= height;
        let left = notch_x + width / 2.0 - NOTE.0 / 2.0;
        let in_note = x >= left && x <= left + NOTE.0 && y >= height && y <= height + NOTE.1;
        !(in_notch || in_note)
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.sensor.is_some() {
            return;
        }
        self.geometry();
        if self.notch.2 <= 0.0 {
            println!("this display has no notch");
            event_loop.exit();
            return;
        }
        let attributes = Window::default_attributes().with_decorations(false).with_inner_size(LogicalSize::new(self.notch.1, self.notch.2));
        let sensor = Rc::new(event_loop.create_window(attributes).unwrap());
        let attributes = Window::default_attributes().with_decorations(false).with_inner_size(LogicalSize::new(NOTE.0, NOTE.1));
        let note = Rc::new(event_loop.create_window(attributes).unwrap());
        note.set_window_level(WindowLevel::AlwaysOnTop);
        note.set_outer_position(LogicalPosition::new(PARKED_X, 200.0));
        let mut sensor_surface = softbuffer::Surface::new(&softbuffer::Context::new(sensor.clone()).unwrap(), sensor.clone()).unwrap();
        let mut note_surface = softbuffer::Surface::new(&softbuffer::Context::new(note.clone()).unwrap(), note.clone()).unwrap();
        paint(&mut sensor_surface, &sensor, 0x0000_0000);
        paint(&mut note_surface, &note, 0x00ff_d60a);
        self.surfaces = vec![sensor_surface, note_surface];
        unsafe {
            for window in [&sensor, &note] {
                let ns = ns_window(window);
                let _: () = msg_send![ns, setCollectionBehavior: BEHAVIOR];
            }
            let ns = ns_window(&sensor);
            let _: () = msg_send![ns, setLevel: ABOVE_MENU_BAR];
            let _: () = msg_send![ns, setHasShadow: Bool::NO];
        }
        self.sensor = Some(sensor);
        self.note = Some(note);
        let _ = Command::new("/usr/bin/osascript").args(["-e", "tell application \"Finder\" to activate"]).status();
        self.next = Instant::now() + Duration::from_millis(600);
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let is_sensor = self.sensor.as_ref().is_some_and(|window| window.id() == id);
        let name = if is_sensor { "sensor" } else { "note" };
        match event {
            WindowEvent::CursorEntered { .. } => {
                println!("{:8.3} {name} CursorEntered", self.at());
                if is_sensor && self.stage == Stage::Waiting {
                    self.reveal();
                }
            }
            WindowEvent::CursorLeft { .. } => println!("{:8.3} {name} CursorLeft", self.at()),
            WindowEvent::Focused(focused) => println!("{:8.3} {name} Focused({focused})", self.at()),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.sensor.is_none() {
            return;
        }
        let now = Instant::now();
        match self.stage {
            Stage::Place if now >= self.next => {
                self.place_sensor();
                self.stage = Stage::CheckPlain;
                self.next = now + Duration::from_millis(300);
            }
            Stage::CheckPlain if now >= self.next => {
                self.report_sensor("placed at y=0, no override");
                let sensor = self.sensor.clone().unwrap();
                let frame: NSRect = unsafe { msg_send![ns_window(&sensor), frame] };
                if self.screen_height - (frame.origin.y + frame.size.height) > 0.5 {
                    unsafe {
                        let class = object_getClass(ns_window(&sensor).cast_const());
                        let imp: Imp = std::mem::transmute(keep_frame as *const ());
                        class_addMethod(class, sel!(constrainFrameRect:toScreen:), imp, c"{CGRect={CGPoint=dd}{CGSize=dd}}@:{CGRect={CGPoint=dd}{CGSize=dd}}@".as_ptr());
                    }
                    self.place_sensor();
                }
                self.stage = Stage::CheckOverride;
                self.next = now + Duration::from_millis(300);
            }
            Stage::CheckOverride if now >= self.next => {
                self.report_sensor("final placement");
                if self.guided {
                    println!("{:8.3} waiting for the pointer at the notch", self.at());
                    self.stage = Stage::Waiting;
                } else {
                    self.stage = Stage::Finished;
                    event_loop.exit();
                    return;
                }
            }
            Stage::Revealed => {
                if self.pointer_outside() {
                    let since = *self.outside_since.get_or_insert(now);
                    if now.duration_since(since) >= Duration::from_millis(500) {
                        let note = self.note.clone().unwrap();
                        note.set_outer_position(LogicalPosition::new(PARKED_X, 200.0));
                        println!("{:8.3} hidden after 500 ms outside: app_active={}", self.at(), self.active());
                        self.report_sensor("sensor after hide");
                        if self.reveals >= 3 {
                            println!("{:8.3} finished", self.at());
                            self.stage = Stage::Finished;
                            event_loop.exit();
                            return;
                        }
                        self.stage = Stage::Waiting;
                    }
                } else {
                    self.outside_since = None;
                }
            }
            _ => {}
        }
        if now >= self.give_up && self.stage != Stage::Finished {
            println!("{:8.3} gave up after {} reveals", self.at(), self.reveals);
            self.stage = Stage::Finished;
            event_loop.exit();
            return;
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(16)));
    }
}

fn main() {
    let guided = std::env::args().nth(1).as_deref() == Some("guided");
    let event_loop = EventLoop::builder().with_activation_policy(ActivationPolicy::Accessory).with_default_menu(false).build().unwrap();
    let now = Instant::now();
    let mut app = App {
        guided,
        sensor: None,
        note: None,
        surfaces: Vec::new(),
        notch: (0.0, 0.0, 0.0),
        screen_height: 0.0,
        stage: Stage::Place,
        next: now,
        outside_since: None,
        reveals: 0,
        started: now,
        give_up: now + Duration::from_secs(1800),
    };
    event_loop.run_app(&mut app).unwrap();
}
