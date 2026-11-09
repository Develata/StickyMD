//! For the checks a person has to perform: logs what winit delivers for the pointer, keys and the
//! input method. `manual_probe [sliver]`; `sliver` parks the window off the left edge with 3 points
//! showing. Runs until Ctrl+C in the terminal.
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::window::{Window, WindowId};

struct App {
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    sliver: bool,
    started: Instant,
    moved: u32,
}

impl App {
    fn paint(&mut self) {
        let window = self.window.as_ref().unwrap();
        let size = window.inner_size();
        let surface = self.surface.as_mut().unwrap();
        surface.resize(NonZeroU32::new(size.width).unwrap(), NonZeroU32::new(size.height).unwrap()).unwrap();
        let mut buffer = surface.buffer_mut().unwrap();
        buffer.fill(0x00f4_e9a8);
        buffer.present().unwrap();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_decorations(false).with_inner_size(LogicalSize::new(320.0, 240.0));
        let window = Rc::new(event_loop.create_window(attributes).unwrap());
        window.set_ime_allowed(true);
        window.set_ime_cursor_area(LogicalPosition::new(40.0, 60.0), LogicalSize::new(2.0, 20.0));
        window.set_outer_position(LogicalPosition::new(if self.sliver { -317.0 } else { 300.0 }, 200.0));
        let context = softbuffer::Context::new(window.clone()).unwrap();
        self.surface = Some(softbuffer::Surface::new(&context, window.clone()).unwrap());
        self.window = Some(window);
        self.paint();
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let at = self.started.elapsed().as_secs_f64();
        match event {
            WindowEvent::RedrawRequested => self.paint(),
            WindowEvent::CursorMoved { .. } => {
                self.moved += 1;
                if self.moved == 1 || self.moved % 20 == 0 {
                    println!("{at:8.3} CursorMoved count={}", self.moved);
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                println!("{at:8.3} Key physical={:?} logical={:?} text={:?} repeat={}", event.physical_key, event.logical_key, event.text, event.repeat);
            }
            WindowEvent::ModifiersChanged(modifiers) => println!("{at:8.3} Modifiers {:?}", modifiers.state()),
            WindowEvent::Ime(ime) => println!("{at:8.3} Ime {ime:?}"),
            WindowEvent::CursorEntered { .. } | WindowEvent::CursorLeft { .. } | WindowEvent::Focused(_) => {
                println!("{at:8.3} {event:?}");
            }
            _ => {}
        }
    }
}

fn main() {
    let sliver = std::env::args().nth(1).as_deref() == Some("sliver");
    let event_loop = EventLoop::builder().with_activation_policy(ActivationPolicy::Accessory).with_default_menu(false).build().unwrap();
    let mut app = App { window: None, surface: None, sliver, started: Instant::now(), moved: 0 };
    event_loop.run_app(&mut app).unwrap();
}
