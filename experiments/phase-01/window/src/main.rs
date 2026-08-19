//! Phase 1B spike: window + CPU framebuffer proof-of-concept.
//!
//! plan_ref: docs/plan/09_windows_shell.md#winit-030--softbuffer-呈现链路
//! plan_ref: docs/plan/00_engineering_constitution.md#§5-idle-behavior
//!
//! Goal (from Phase 1 prompt 1B): prove that a winit 0.30 window can present a
//! tiny-skia-rendered framebuffer through softbuffer on Windows, while:
//!   * never continuously calling `request_redraw()` (ControlFlow::Wait),
//!   * only redrawing on a dirty event (resize / key),
//!   * driving Win32-only attributes (whole-window opacity, Win11 rounded
//!     corners) from a thin adapter (`win32.rs`).
//!
//! This is an experiment. It is deliberately NOT production-shaped and the
//! whole `experiments/phase-01` tree is deletable.
mod win32;

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;

use raw_window_handle::{HandleError, HasDisplayHandle, HasWindowHandle};
use softbuffer::{Context, Surface};
use tiny_skia::{Color, Paint, Pixmap, Rect, Transform};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

/// Owned handle source for softbuffer.
///
/// rwh's `DisplayHandle<'a>`/`WindowHandle<'a>` are borrowed, which does not fit
/// a struct that owns its `Context`/`Surface`. We wrap an `Arc<Window>` and
/// re-implement the two rwh traits by delegating to the live winit window.
struct WinRef(Arc<Window>);

impl HasWindowHandle for WinRef {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        self.0.window_handle()
    }
}

impl HasDisplayHandle for WinRef {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, HandleError> {
        self.0.display_handle()
    }
}

/// Opacity cycle used by the `O` key: 70% -> 85% -> 96% -> 100% -> wrap.
const OPACITY_STEPS: [u8; 4] = [70, 85, 96, 100];

fn nz(v: u32) -> Option<NonZeroU32> {
    NonZeroU32::new(v)
}

struct SpikeApp {
    window: Option<Arc<Window>>,
    #[allow(dead_code)]
    context: Option<Context<WinRef>>,
    surface: Option<Surface<WinRef, WinRef>>,
    pixmap: Option<Pixmap>,

    width: u32,
    height: u32,

    opacity_index: usize,
    corners_on: bool,

    redraw_count: u64,
    pixmap_realloc_count: u64,
    last_redraw: Option<Instant>,
    start: Instant,
}

impl SpikeApp {
    fn new() -> Self {
        Self {
            window: None,
            context: None,
            surface: None,
            pixmap: None,
            width: 0,
            height: 0,
            opacity_index: OPACITY_STEPS.len() - 1, // start at 100%
            corners_on: true,
            redraw_count: 0,
            pixmap_realloc_count: 0,
            last_redraw: None,
            start: Instant::now(),
        }
    }

    fn apply_win32_attributes(&self) {
        #[cfg(windows)]
        if let Some(window) = &self.window {
            if let Some(hwnd) = win32::hwnd_from_window(window) {
                let percent = OPACITY_STEPS[self.opacity_index];
                // SAFETY: `hwnd` came from a live winit window owned by this
                // process; it stays valid for the duration of these calls.
                unsafe {
                    if let Err(e) = win32::set_opacity_percent(hwnd, percent) {
                        println!("[spike] opacity({percent}%) failed: {e}");
                    }
                    if self.corners_on {
                        if let Err(e) = win32::enable_rounded_corners(hwnd) {
                            println!("[spike] rounded-corners failed: {e}");
                        }
                    }
                }
                println!(
                    "[spike] win32 attributes applied: opacity={}%, corners={}",
                    percent, self.corners_on
                );
            } else {
                println!("[spike] could not obtain HWND from winit window");
            }
        }
    }

    /// Resize the softbuffer surface + our cached pixmap to the physical size.
    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.width = width;
        self.height = height;

        if let Some(surface) = &mut self.surface {
            if let (Some(w), Some(h)) = (nz(width), nz(height)) {
                if let Err(e) = surface.resize(w, h) {
                    println!("[spike] surface.resize failed: {e}");
                }
            }
        }

        let needs_realloc = self
            .pixmap
            .as_ref()
            .map_or(true, |p| p.width() != width || p.height() != height);
        if needs_realloc {
            self.pixmap = Pixmap::new(width, height);
            self.pixmap_realloc_count += 1;
            println!("[spike] pixmap realloc #{}, size={width}x{height}", self.pixmap_realloc_count);
        }
    }

    fn render(&mut self) {
        let (width, height) = (self.width, self.height);
        if width == 0 || height == 0 {
            return;
        }
        let Some(surface) = &mut self.surface else { return };
        let Some(pixmap) = &mut self.pixmap else { return };

        draw_scene(pixmap, width, height, self.redraw_count);

        match surface.buffer_mut() {
            Ok(mut buffer) => {
                if buffer.len() != (width as usize) * (height as usize) {
                    println!(
                        "[spike] buffer len {} != {width}x{height}, skipping present",
                        buffer.len()
                    );
                    return;
                }
                for (dst, chunk) in buffer.iter_mut().zip(pixmap.data().chunks_exact(4)) {
                    let r = chunk[0] as u32;
                    let g = chunk[1] as u32;
                    let b = chunk[2] as u32;
                    let a = chunk[3] as u32;
                    *dst = (a << 24) | (r << 16) | (g << 8) | b;
                }
                if let Err(e) = buffer.present() {
                    println!("[spike] present failed: {e}");
                    return;
                }
                self.redraw_count += 1;
                self.last_redraw = Some(Instant::now());
            }
            Err(e) => println!("[spike] buffer_mut failed: {e}"),
        }
    }

    fn print_stats(&self, reason: &str) {
        let now = Instant::now();
        let uptime = now.duration_since(self.start).as_secs_f64();
        let idle = self
            .last_redraw
            .map(|t| now.duration_since(t).as_secs_f64())
            .unwrap_or(uptime);
        println!("--- spike-window stats ({reason}) ---");
        println!("  uptime_s           : {uptime:.2}");
        println!("  redraw_count       : {}", self.redraw_count);
        println!("  pixmap_reallocs    : {}", self.pixmap_realloc_count);
        println!("  seconds_since_draw : {idle:.2}  (idle = no redraw during this window)");
        println!("  final_size         : {}x{}", self.width, self.height);
        println!("--------------------------------------");
    }
}

impl ApplicationHandler for SpikeApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);

        if self.window.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("StickyMD Phase 1B — window/framebuffer spike")
            .with_inner_size(winit::dpi::PhysicalSize::new(820u32, 560u32))
            .with_min_inner_size(winit::dpi::PhysicalSize::new(200u32, 150u32));

        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                println!("[spike] create_window failed: {e}");
                event_loop.exit();
                return;
            }
        };

        let scale = window.scale_factor();
        let size = window.inner_size();
        println!("[spike] window created: {size:?} @ scale {scale}");

        let context = match Context::new(WinRef(window.clone())) {
            Ok(c) => c,
            Err(e) => {
                println!("[spike] softbuffer Context::new failed: {e}");
                event_loop.exit();
                return;
            }
        };
        let surface = match Surface::new(&context, WinRef(window.clone())) {
            Ok(s) => s,
            Err(e) => {
                println!("[spike] softbuffer Surface::new failed: {e}");
                event_loop.exit();
                return;
            }
        };

        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);

        self.resize(size.width, size.height);
        self.apply_win32_attributes();
        // Single initial draw; after this we only redraw on dirty events.
        self.render();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.print_stats("close-requested");
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                let scale = self.window.as_ref().map(|w| w.scale_factor()).unwrap_or(1.0);
                println!("[spike] Resized {}x{} @ scale {scale}", size.width, size.height);
                self.resize(size.width, size.height);
                self.render();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let size = self.window.as_ref().map(|w| w.inner_size());
                println!("[spike] ScaleFactorChanged -> {scale_factor} (size {size:?})");
                self.render();
            }
            WindowEvent::RedrawRequested => {
                // Only reached when something explicitly requested a redraw.
                self.render();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key,
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                if let PhysicalKey::Code(code) = physical_key {
                    match code {
                        KeyCode::KeyO => {
                            self.opacity_index = (self.opacity_index + 1) % OPACITY_STEPS.len();
                            println!(
                                "[spike] O pressed -> opacity {}%",
                                OPACITY_STEPS[self.opacity_index]
                            );
                            self.apply_win32_attributes();
                            self.render();
                        }
                        KeyCode::KeyR => {
                            self.corners_on = !self.corners_on;
                            println!("[spike] R pressed -> corners_on={}", self.corners_on);
                            self.apply_win32_attributes();
                            self.render();
                        }
                        KeyCode::Escape => {
                            self.print_stats("escape");
                            event_loop.exit();
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

/// Render a deterministic, low-cost scene into the pixmap.
///
/// No text shaping here — typography belongs to the Phase 1C (cosmic-text)
/// spike. This spike only proves the framebuffer present path.
fn draw_scene(pixmap: &mut Pixmap, w: u32, h: u32, frame: u64) {
    pixmap.fill(Color::from_rgba8(28, 30, 38, 255));

    let mut paint = Paint::default();

    // Border frame.
    paint.set_color_rgba8(96, 146, 255, 255);
    if let Some(r) = Rect::from_xywh(6.0, 6.0, w as f32 - 12.0, h as f32 - 12.0) {
        pixmap.fill_rect(r, &paint, Transform::identity(), None);
    }

    // Inner panel.
    paint.set_color_rgba8(22, 24, 32, 255);
    if let Some(r) = Rect::from_xywh(10.0, 10.0, w as f32 - 20.0, h as f32 - 20.0) {
        pixmap.fill_rect(r, &paint, Transform::identity(), None);
    }

    // Accent block that shifts with `frame` so a redraw is visually observable.
    let t = (frame % 80) as f32;
    paint.set_color_rgba8(255, 122, 60, 255);
    if let Some(r) = Rect::from_xywh(24.0 + t, 24.0, 42.0, 42.0) {
        pixmap.fill_rect(r, &paint, Transform::identity(), None);
    }

    // A static cool block for contrast.
    paint.set_color_rgba8(120, 220, 160, 255);
    if let Some(r) = Rect::from_xywh(w as f32 - 90.0, h as f32 - 90.0, 56.0, 56.0) {
        pixmap.fill_rect(r, &paint, Transform::identity(), None);
    }
}

fn main() {
    println!("[spike] phase-01 window/framebuffer spike starting (ControlFlow::Wait, dirty-only redraws)");
    println!("[spike] keys: O=opacity cycle  R=rounded corners  Esc=exit");
    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = SpikeApp::new();
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("[spike] run_app error: {e}");
        std::process::exit(1);
    }
}
