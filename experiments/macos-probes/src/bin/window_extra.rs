//! Follow-ups to window_probe: lifting AppKit's top-edge clamp, waking from the background, and
//! whether a difference-blended sublayer really inverts. `window_extra <png path>`.
use std::num::NonZeroU32;
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Sel};
use objc2::{class, msg_send, sel};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalPosition;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId, WindowLevel};

extern "C-unwind" fn keep_frame(_this: *mut AnyObject, _sel: Sel, frame: NSRect, _screen: *mut AnyObject) -> NSRect {
    frame
}

unsafe extern "C" {
    fn object_getClass(object: *const AnyObject) -> *const AnyClass;
    fn class_addMethod(class: *const AnyClass, name: Sel, imp: Imp, types: *const std::ffi::c_char) -> Bool;
}

struct App {
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    layer: Option<Retained<AnyObject>>,
    png: String,
    step: usize,
    next: Instant,
    focus: Vec<bool>,
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

    fn report(&self, label: &str) {
        let window = self.window.as_ref().unwrap();
        let outer = window.outer_position().unwrap().to_logical::<f64>(window.scale_factor());
        let frame: NSRect = unsafe { msg_send![self.ns_window(), frame] };
        println!("{label}: winit=({:.1},{:.1}) ns_origin=({:.1},{:.1})", outer.x, outer.y, frame.origin.x, frame.origin.y);
    }

    fn active(&self) -> (bool, bool) {
        let app: *mut AnyObject = unsafe { msg_send![class!(NSApplication), sharedApplication] };
        unsafe { (msg_send![app, isActive], msg_send![self.ns_window(), isKeyWindow]) }
    }

    fn activate_finder() {
        let _ = Command::new("/usr/bin/osascript").args(["-e", "tell application \"Finder\" to activate"]).status();
    }

    fn paint(&mut self) {
        // Left half red-ish, right half blue-ish, so an inversion is visible per channel.
        let window = self.window.as_ref().unwrap();
        let size = window.inner_size();
        let surface = self.surface.as_mut().unwrap();
        surface.resize(NonZeroU32::new(size.width).unwrap(), NonZeroU32::new(size.height).unwrap()).unwrap();
        let mut buffer = surface.buffer_mut().unwrap();
        let width = size.width as usize;
        for (index, pixel) in buffer.iter_mut().enumerate() {
            *pixel = if index % width < width / 2 { 0x00c0_4020 } else { 0x0020_40c0 };
        }
        buffer.present().unwrap();
    }

    fn add_inverting_layer(&mut self) {
        let window = self.window.clone().unwrap();
        let size = window.inner_size().to_logical::<f64>(window.scale_factor());
        unsafe {
            let root: *mut AnyObject = msg_send![self.view(), layer];
            let layer: Retained<AnyObject> = msg_send![class!(CALayer), new];
            let frame = NSRect::new(NSPoint::new(size.width / 2.0 - 40.0, 40.0), NSSize::new(80.0, 40.0));
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

    fn capture(&self, suffix: &str) {
        let window = self.window.as_ref().unwrap();
        let outer = window.outer_position().unwrap().to_logical::<f64>(window.scale_factor());
        let size = window.outer_size().to_logical::<f64>(window.scale_factor());
        let region = format!("{},{},{},{}", outer.x as i64, outer.y as i64, size.width as i64, 120);
        let path = format!("{}{suffix}.png", self.png);
        let status = Command::new("/usr/sbin/screencapture").args(["-x", "-R", &region, &path]).status();
        println!("screencapture {suffix}: {status:?} region={region} exists={}", std::path::Path::new(&path).exists());
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_decorations(false).with_inner_size(LogicalSize::new(520.0, 680.0));
        let window = Rc::new(event_loop.create_window(attributes).unwrap());
        let context = softbuffer::Context::new(window.clone()).unwrap();
        self.surface = Some(softbuffer::Surface::new(&context, window.clone()).unwrap());
        self.window = Some(window);
        self.paint();
        self.next = Instant::now() + Duration::from_millis(500);
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let WindowEvent::Focused(focused) = event {
            self.focus.push(focused);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() && Instant::now() >= self.next {
            let window = self.window.clone().unwrap();
            let height = window.outer_size().to_logical::<f64>(window.scale_factor()).height;
            let mut wait = Duration::from_millis(300);
            match self.step {
                0 => window.set_outer_position(LogicalPosition::new(300.0, 3.0 - height)),
                1 => self.report("top, clamp in place"),
                2 => unsafe {
                    let class = object_getClass(self.ns_window().cast_const());
                    let imp: Imp = std::mem::transmute(keep_frame as *const ());
                    let added = class_addMethod(class, sel!(constrainFrameRect:toScreen:), imp, c"{CGRect={CGPoint=dd}{CGSize=dd}}@:{CGRect={CGPoint=dd}{CGSize=dd}}@".as_ptr());
                    println!("class_addMethod constrainFrameRect on {}: {}", (*class).name().to_string_lossy(), added.as_bool());
                },
                3 => window.set_outer_position(LogicalPosition::new(300.0, 3.0 - height)),
                4 => self.report("top after overriding constrainFrameRect"),
                5 => window.set_outer_position(LogicalPosition::new(300.0, 36.0 - height)),
                6 => self.report("3 below a 33 pt menu bar"),
                7 => window.set_outer_position(LogicalPosition::new(200.0, 160.0)),
                8 => {
                    println!("before Finder: {:?}", self.active());
                    Self::activate_finder();
                    wait = Duration::from_millis(1000);
                }
                9 => {
                    println!("Finder frontmost: (app_active,key)={:?}", self.active());
                    window.focus_window();
                    println!("focus_window immediate: {:?}", self.active());
                    wait = Duration::from_millis(1000);
                }
                10 => {
                    println!("focus_window after 1 s, from background: {:?} events={:?}", self.active(), self.focus);
                    window.set_window_level(WindowLevel::AlwaysOnTop);
                    self.paint();
                    wait = Duration::from_millis(500);
                }
                11 => self.capture("-plain"),
                12 => {
                    self.add_inverting_layer();
                    wait = Duration::from_millis(500);
                }
                13 => self.capture("-inverted"),
                _ => {
                    event_loop.exit();
                    return;
                }
            }
            self.step += 1;
            self.next = Instant::now() + wait;
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next));
    }
}

fn main() {
    let png = std::env::args().nth(1).expect("png prefix");
    let event_loop = EventLoop::builder().with_activation_policy(ActivationPolicy::Accessory).with_default_menu(false).build().unwrap();
    let mut app = App { window: None, surface: None, layer: None, png, step: 0, next: Instant::now(), focus: Vec::new() };
    event_loop.run_app(&mut app).unwrap();
}
