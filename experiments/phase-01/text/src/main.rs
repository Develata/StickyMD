//! Phase 1C spike: text + IME + script-based font runs.
//!
//! plan_ref: docs/plan/03_text_model.md ; docs/plan/09_windows_shell.md
//!
//! Goal (from Phase 1 prompt 1C): prove that a canonical `String` buffer can be
//! projected into a cosmic-text `Buffer` for shaping/rendering, that winit IME
//! events (Preedit/Commit) can drive edits, that script-based font runs work
//! (CJK vs. Latin), and that a thin Win32 clipboard adapter works.
//!
//! Model contract demonstrated here (mirrors the Phase 2 document model):
//!   * `canonical: String` is the single source of truth.
//!   * the cosmic-text `Buffer` is a *projection* rebuilt from `canonical`
//!     (it is never the authority).
//!   * IME `Commit` mutates `canonical`; `Preedit` is transient overlay.
//!
//! This is an experiment. Deliberately NOT production-shaped; deletable.
mod win32_clipboard;

use std::sync::Arc;
use std::time::Instant;

use cosmic_text::{
    Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Wrap,
};
use raw_window_handle::{HandleError, HasDisplayHandle, HasWindowHandle};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Ime, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

const PAD: f32 = 14.0;
const FONT_SIZE: f32 = 22.0;
const LINE_HEIGHT: f32 = 32.0;

const CJK_CANDIDATES: [&str; 6] = [
    "仿宋_GB2312",
    "FangSong_GB2312",
    "FangSong",
    "KaiTi",
    "SimSun",
    "Microsoft YaHei",
];
const LATIN_CANDIDATES: [&str; 2] = ["Times New Roman", "Georgia"];

const BG: u32 = 0xFF_1E_20_28;
const TEXT: Color = Color(0xFF_E6_E8_F0); // AARRGGBB
const CARET: Color = Color(0xFF_5A_9C_FF);
const PREEDIT_BG: Color = Color(0x66_5A_9C_FF);

/// Owned handle source for softbuffer (see the window spike for rationale).
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

/// True for characters that should use a CJK font run.
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x11FF      // Hangul Jamo
        | 0x2E80..=0x303F    // CJK radicals, Kangxi, CJK punctuation
        | 0x3040..=0x30FF    // Hiragana + Katakana
        | 0x31C0..=0x31EF
        | 0x3200..=0x33FF
        | 0x3400..=0x4DBF    // Ext A
        | 0x4E00..=0x9FFF    // CJK Unified
        | 0xF900..=0xFAFF    // Compatibility
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFFEF    // Fullwidth forms
        | 0x20000..=0x2FA1F
    )
}

/// Split `text` into (substring, is_cjk) runs. Neutral chars (space/newline/ascii
/// punct) attach to the previous run to avoid over-fragmentation.
fn script_runs(text: &str) -> Vec<(String, bool)> {
    let mut runs: Vec<(String, bool)> = Vec::new();
    for c in text.chars() {
        let cls = is_cjk(c);
        let neutral = c.is_whitespace() || (c.is_ascii_punctuation() && !cls);
        if let Some((buf, prev_cls)) = runs.last_mut()
            && (neutral || *prev_cls == cls)
        {
            buf.push(c);
            if neutral && !*prev_cls {
                // keep previous class
            }
            continue;
        }
        runs.push((c.to_string(), cls));
    }
    runs
}

/// Probe the font database for an exact family name (fontdb 0.23 Query API).
fn font_family_present(font_system: &mut FontSystem, name: &str) -> bool {
    use cosmic_text::fontdb::{Family as DbFamily, Query, Stretch, Style, Weight};
    let query = Query {
        families: &[DbFamily::Name(name)],
        weight: Weight::NORMAL,
        stretch: Stretch::Normal,
        style: Style::Normal,
    };
    font_system.db_mut().query(&query).is_some()
}

struct TextApp {
    window: Option<Arc<Window>>,
    #[allow(dead_code)]
    context: Option<Context<WinRef>>,
    surface: Option<Surface<WinRef, WinRef>>,

    width: u32,
    height: u32,
    scale: f64,

    font_system: FontSystem,
    swash: SwashCache,
    buffer: Buffer,

    // Canonical model (source of truth) + transient IME overlay.
    canonical: String,
    cursor: usize,
    preedit: Option<String>,
    undo: Vec<String>,

    cjk_family: &'static str,
    cjk_found: bool,
    latin_family: &'static str,
    latin_found: bool,

    modifiers: ModifiersState,
    ime_active: bool,
    composing: bool,

    // Counters for honest reporting.
    commit_events: u64,
    preedit_events: u64,
    rebuilds: u64,
    redraws: u64,
    start: Instant,
}

impl TextApp {
    fn new() -> Self {
        let mut font_system = FontSystem::new();
        let swash = SwashCache::new();
        let buffer = Buffer::new(&mut font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));

        let mut app = Self {
            window: None,
            context: None,
            surface: None,
            width: 0,
            height: 0,
            scale: 1.0,
            font_system,
            swash,
            buffer,
            canonical: String::from(
                "StickyMD Phase 1C text/IME spike\n中文与 English 混排测试：你好，世界！\nLaTeX inline $E=mc^2$ stays plain here.",
            ),
            cursor: 0,
            preedit: None,
            undo: Vec::new(),
            cjk_family: CJK_CANDIDATES[0],
            cjk_found: false,
            latin_family: LATIN_CANDIDATES[0],
            latin_found: false,
            modifiers: ModifiersState::default(),
            ime_active: false,
            composing: false,
            commit_events: 0,
            preedit_events: 0,
            rebuilds: 0,
            redraws: 0,
            start: Instant::now(),
        };
        app.cursor = app.canonical.len();
        app.probe_fonts();
        app
    }

    fn probe_fonts(&mut self) {
        for name in CJK_CANDIDATES {
            if font_family_present(&mut self.font_system, name) {
                self.cjk_family = name;
                self.cjk_found = true;
                break;
            }
        }
        for name in LATIN_CANDIDATES {
            if font_family_present(&mut self.font_system, name) {
                self.latin_family = name;
                self.latin_found = true;
                break;
            }
        }
        println!(
            "[spike-text] font probe: CJK='{}' (found={}) Latin='{}' (found={})",
            self.cjk_family, self.cjk_found, self.latin_family, self.latin_found
        );
    }

    fn push_undo(&mut self) {
        self.undo.push(self.canonical.clone());
        if self.undo.len() > 64 {
            self.undo.remove(0);
        }
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.canonical = prev;
            self.cursor = self.canonical.len();
            self.rebuild_projection();
            self.request_redraw();
            println!("[spike-text] undo -> {} bytes", self.canonical.len());
        }
    }

    /// Rebuild the cosmic-text Buffer (projection) from the canonical String.
    fn rebuild_projection(&mut self) {
        let mut display = String::with_capacity(self.canonical.len() + 16);
        display.push_str(&self.canonical[..self.cursor]);
        if let Some(p) = &self.preedit {
            display.push_str(p);
        }
        display.push_str(&self.canonical[self.cursor..]);

        let latin_attrs = Attrs::new().family(Family::Name(self.latin_family));
        let cjk_attrs = Attrs::new().family(Family::Name(self.cjk_family));

        let runs = script_runs(&display);
        let spans: Vec<(&str, Attrs)> = runs
            .iter()
            .map(|(s, cjk)| {
                (
                    s.as_str(),
                    if *cjk {
                        cjk_attrs.clone()
                    } else {
                        latin_attrs.clone()
                    },
                )
            })
            .collect();

        let area_w = ((self.width as f32) - PAD * 2.0).max(40.0);
        let area_h = ((self.height as f32) - PAD * 2.0).max(40.0);
        self.buffer.set_size(Some(area_w), Some(area_h));
        self.buffer.set_wrap(Wrap::Word);
        self.buffer
            .set_rich_text(spans, &latin_attrs, Shaping::Advanced, Some(Align::Left));
        self.buffer.shape_until_scroll(&mut self.font_system, false);
        self.rebuilds += 1;
    }

    /// Compute caret pixel rect (relative to buffer origin, i.e. without PAD).
    fn caret_rect(&mut self) -> Option<(f32, f32, f32)> {
        let caret_byte = self.cursor + self.preedit.as_ref().map(|p| p.len()).unwrap_or(0);
        let mut display = String::with_capacity(self.canonical.len() + 16);
        display.push_str(&self.canonical[..self.cursor]);
        if let Some(p) = &self.preedit {
            display.push_str(p);
        }
        display.push_str(&self.canonical[self.cursor..]);
        let clamped = caret_byte.min(display.len());
        let before = &display[..clamped];
        let line = before.bytes().filter(|&b| b == b'\n').count();
        let index = clamped - before.rfind('\n').map(|i| i + 1).unwrap_or(0);

        let mut last_for_line: Option<(f32, f32)> = None;
        for run in self.buffer.layout_runs() {
            if run.line_i != line {
                continue;
            }
            let (rs, re) = match (run.glyphs.first(), run.glyphs.last()) {
                (Some(f), Some(l)) => (f.start, l.end),
                _ => (0, 0),
            };
            last_for_line = Some((run.line_top, run.line_height));
            if index >= rs && index <= re {
                let x = caret_x_in_run(run.glyphs, index, run.line_w);
                return Some((x, run.line_top, run.line_height));
            }
        }
        last_for_line.map(|(top, h)| (0.0, top, h))
    }

    fn update_ime_area(&mut self) {
        let caret = self.caret_rect();
        let scale = self.scale;
        if let Some(window) = &self.window
            && let Some((x, y, h)) = caret
        {
            let px = ((x + PAD) * scale as f32) as i32;
            let py = ((y + PAD) * scale as f32) as i32;
            window.set_ime_cursor_area(
                winit::dpi::PhysicalPosition::new(px, py),
                winit::dpi::PhysicalSize::new(2u32, (h * scale as f32) as u32),
            );
        }
    }

    fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn insert_at_cursor(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.push_undo();
        self.canonical.insert_str(self.cursor, s);
        self.cursor += s.len();
        self.rebuild_projection();
        self.request_redraw();
    }

    fn delete_prev(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev = self.canonical[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.push_undo();
        self.canonical.replace_range(prev..self.cursor, "");
        self.cursor = prev;
        self.rebuild_projection();
        self.request_redraw();
    }

    fn move_left(&mut self) {
        self.cursor = self.canonical[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.request_redraw();
    }
    fn move_right(&mut self) {
        self.cursor = self.canonical[self.cursor..]
            .chars()
            .next()
            .map(|c| self.cursor + c.len_utf8())
            .unwrap_or(self.cursor);
        self.request_redraw();
    }

    fn render(&mut self) {
        let (width, height) = (self.width, self.height);
        if width == 0 || height == 0 {
            return;
        }

        // Compute caret / preedit geometry up front (needs layout only, no surface).
        let caret = self.caret_rect();
        let preedit_chars = self
            .preedit
            .as_ref()
            .map(|p| p.chars().count())
            .unwrap_or(0);

        let Some(surface) = &mut self.surface else {
            return;
        };
        let Ok(mut fb) = surface.buffer_mut() else {
            return;
        };

        for px in fb.iter_mut() {
            *px = BG;
        }

        // Text projection.
        self.buffer.draw(
            &mut self.font_system,
            &mut self.swash,
            TEXT,
            |x, y, w, h, color| {
                blend_rect(
                    &mut fb,
                    width as i32,
                    height as i32,
                    x + PAD as i32,
                    y + PAD as i32,
                    w,
                    h,
                    color,
                );
            },
        );

        // Preedit underline band.
        if let (Some((x, y, h)), true) = (caret, preedit_chars > 0) {
            let w = (preedit_chars as f32) * FONT_SIZE * 0.9;
            blend_rect(
                &mut fb,
                width as i32,
                height as i32,
                (x + PAD) as i32,
                (y + PAD + h - 3.0) as i32,
                w as u32,
                2,
                PREEDIT_BG,
            );
        }

        // Caret.
        if let Some((x, y, h)) = caret {
            blend_rect(
                &mut fb,
                width as i32,
                height as i32,
                (x + PAD) as i32,
                (y + PAD) as i32,
                2,
                h as u32,
                CARET,
            );
        }

        if let Err(e) = fb.present() {
            println!("[spike-text] present failed: {e}");
            return;
        }
        self.redraws += 1;
        self.update_ime_area();
    }

    fn print_stats(&self, reason: &str) {
        let up = Instant::now().duration_since(self.start).as_secs_f64();
        println!("--- spike-text stats ({reason}) ---");
        println!("  uptime_s        : {up:.2}");
        println!("  canonical_bytes : {}", self.canonical.len());
        println!("  commit_events   : {}", self.commit_events);
        println!("  preedit_events  : {}", self.preedit_events);
        println!("  rebuilds        : {}", self.rebuilds);
        println!("  redraws         : {}", self.redraws);
        println!("  undo_depth      : {}", self.undo.len());
        println!(
            "  cjk_family      : {} (found={})",
            self.cjk_family, self.cjk_found
        );
        println!(
            "  latin_family    : {} (found={})",
            self.latin_family, self.latin_found
        );
        println!("  clipboard_avail : {}", win32_clipboard::available());
        println!("-----------------------------------");
    }
}

fn caret_x_in_run(glyphs: &[cosmic_text::LayoutGlyph], index: usize, line_w: f32) -> f32 {
    if glyphs.is_empty() {
        return 0.0;
    }
    for g in glyphs {
        if index <= g.start {
            return g.x;
        }
        if index < g.end {
            let span = (g.end - g.start).max(1) as f32;
            return g.x + g.w * ((index - g.start) as f32 / span);
        }
    }
    glyphs.last().map(|g| g.x + g.w).unwrap_or(line_w)
}

#[allow(clippy::too_many_arguments)]
fn blend_rect(
    fb: &mut [u32],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    color: Color,
) {
    let sa = color.a() as u32;
    if sa == 0 {
        return;
    }
    let sr = color.r() as u32;
    let sg = color.g() as u32;
    let sb = color.b() as u32;
    for yy in y..(y + h as i32) {
        if yy < 0 || yy >= height {
            continue;
        }
        for xx in x..(x + w as i32) {
            if xx < 0 || xx >= width {
                continue;
            }
            let dst = &mut fb[(yy as usize) * (width as usize) + (xx as usize)];
            if sa == 255 {
                *dst = 0xFF00_0000 | (sr << 16) | (sg << 8) | sb;
            } else {
                let inv = 255 - sa;
                let dr = (*dst >> 16) & 0xFF;
                let dg = (*dst >> 8) & 0xFF;
                let db = *dst & 0xFF;
                let r = (sr * sa + dr * inv) / 255;
                let g = (sg * sa + dg * inv) / 255;
                let b = (sb * sa + db * inv) / 255;
                *dst = 0xFF00_0000 | (r << 16) | (g << 8) | b;
            }
        }
    }
}

impl ApplicationHandler for TextApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);
        if self.window.is_some() {
            return;
        }
        let attrs = WindowAttributes::default()
            .with_title("StickyMD Phase 1C — text/IME spike")
            .with_inner_size(winit::dpi::PhysicalSize::new(860u32, 540u32))
            .with_min_inner_size(winit::dpi::PhysicalSize::new(240u32, 160u32));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                println!("[spike-text] create_window failed: {e}");
                event_loop.exit();
                return;
            }
        };
        self.scale = window.scale_factor();
        let size = window.inner_size();
        self.width = size.width;
        self.height = size.height;
        window.set_ime_allowed(true);

        let context = match Context::new(WinRef(window.clone())) {
            Ok(c) => c,
            Err(e) => {
                println!("[spike-text] Context::new failed: {e}");
                event_loop.exit();
                return;
            }
        };
        let surface = match Surface::new(&context, WinRef(window.clone())) {
            Ok(s) => s,
            Err(e) => {
                println!("[spike-text] Surface::new failed: {e}");
                event_loop.exit();
                return;
            }
        };
        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);

        if let Some(s) = &mut self.surface
            && let (Some(w), Some(h)) = (
                std::num::NonZeroU32::new(self.width),
                std::num::NonZeroU32::new(self.height),
            )
        {
            let _ = s.resize(w, h);
        }
        self.rebuild_projection();
        self.render();
        println!(
            "[spike-text] window ready {}x{} @{}; type to test IME; Ctrl+C/V clipboard; Ctrl+Z undo",
            self.width, self.height, self.scale
        );

        if std::env::var("SPIKE_TEXT_SELFTEST").as_deref() == Ok("1") {
            let baseline = self.canonical.clone();
            self.insert_at_cursor("你好A");
            let after_insert_ok =
                self.canonical.ends_with("你好A") && self.canonical.len() == baseline.len() + 7;
            self.undo();
            let after_undo_ok = self.canonical == baseline && self.cursor == baseline.len();
            println!(
                "[spike-text] SELFTEST edit pipeline: insert={} undo={} rebuilds={}",
                if after_insert_ok { "PASS" } else { "FAIL" },
                if after_undo_ok { "PASS" } else { "FAIL" },
                self.rebuilds
            );
            self.print_stats("selftest");
            event_loop.exit();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.print_stats("close-requested");
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                self.width = size.width;
                self.height = size.height;
                if let Some(s) = &mut self.surface
                    && let (Some(w), Some(h)) = (
                        std::num::NonZeroU32::new(size.width),
                        std::num::NonZeroU32::new(size.height),
                    )
                {
                    let _ = s.resize(w, h);
                }
                self.rebuild_projection();
                self.render();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = scale_factor;
                println!("[spike-text] ScaleFactorChanged -> {scale_factor}");
                self.render();
            }
            WindowEvent::RedrawRequested => self.render(),
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
            }
            WindowEvent::Ime(ime) => match ime {
                Ime::Enabled => {
                    self.ime_active = true;
                    println!("[spike-text] Ime::Enabled");
                }
                Ime::Disabled => {
                    self.ime_active = false;
                    self.composing = false;
                    println!("[spike-text] Ime::Disabled");
                }
                Ime::Preedit(text, _cursor_range) => {
                    self.preedit_events += 1;
                    if text.is_empty() {
                        self.preedit = None;
                        self.composing = false;
                    } else {
                        self.preedit = Some(text.clone());
                        self.composing = true;
                    }
                    println!("[spike-text] Ime::Preedit {:?}", text);
                    self.rebuild_projection();
                    self.request_redraw();
                }
                Ime::Commit(text) => {
                    self.commit_events += 1;
                    self.composing = false;
                    self.preedit = None;
                    println!("[spike-text] Ime::Commit {:?}", text);
                    self.insert_at_cursor(&text);
                }
            },
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key,
                        state: ElementState::Pressed,
                        text,
                        ..
                    },
                ..
            } => {
                let ctrl = self.modifiers.control_key();
                if let PhysicalKey::Code(code) = physical_key {
                    match code {
                        KeyCode::Escape => {
                            self.print_stats("escape");
                            event_loop.exit();
                            return;
                        }
                        KeyCode::Backspace => {
                            self.delete_prev();
                            return;
                        }
                        KeyCode::ArrowLeft => {
                            self.move_left();
                            return;
                        }
                        KeyCode::ArrowRight => {
                            self.move_right();
                            return;
                        }
                        KeyCode::Enter | KeyCode::NumpadEnter => {
                            self.insert_at_cursor("\n");
                            return;
                        }
                        KeyCode::KeyC if ctrl => {
                            match win32_clipboard::set_text(&self.canonical) {
                                Ok(()) => println!(
                                    "[spike-text] clipboard SET ok ({} bytes)",
                                    self.canonical.len()
                                ),
                                Err(e) => println!("[spike-text] clipboard SET failed: {e}"),
                            }
                            return;
                        }
                        KeyCode::KeyV if ctrl => {
                            match win32_clipboard::get_text() {
                                Some(t) => {
                                    println!("[spike-text] clipboard GET ok ({} bytes)", t.len());
                                    self.insert_at_cursor(&t);
                                }
                                None => println!("[spike-text] clipboard GET returned None"),
                            }
                            return;
                        }
                        KeyCode::KeyZ if ctrl => {
                            self.undo();
                            return;
                        }
                        _ => {}
                    }
                }
                // Direct (non-IME) character input.
                if !ctrl
                    && !self.composing
                    && let Some(t) = text
                {
                    let s: &str = t.as_str();
                    if !s.is_empty() {
                        self.insert_at_cursor(s);
                    }
                }
            }
            _ => {}
        }
    }
}

fn main() {
    println!("[spike-text] phase-01 text/IME spike starting");
    println!(
        "[spike-text] clipboard adapter available = {}",
        win32_clipboard::available()
    );
    // Clipboard self-check (non-interactive). Save/restore the user's clipboard.
    let saved = win32_clipboard::get_text();
    let probe = "StickyMD spike clipboard 自检 ✓";
    match win32_clipboard::set_text(probe) {
        Ok(()) => {
            let got = win32_clipboard::get_text();
            println!(
                "[spike-text] clipboard self-check: set+get => {}",
                if got.as_deref() == Some(probe) {
                    "PASS"
                } else {
                    "MISMATCH/FAIL"
                }
            );
            if let Some(prev) = saved {
                let _ = win32_clipboard::set_text(&prev);
            }
        }
        Err(e) => println!("[spike-text] clipboard self-check set failed: {e}"),
    }

    let event_loop = EventLoop::new().expect("event loop");
    let mut app = TextApp::new();
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("[spike-text] run_app error: {e}");
        std::process::exit(1);
    }
}
