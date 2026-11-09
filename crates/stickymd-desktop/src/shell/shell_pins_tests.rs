//! Characterization pins of the shell, taken before its leaves left this crate and kept here while
//! they move: control rectangles, pixel hashes of the toolbar, scrollbars and Search overlay, the
//! hover cursor and the routed press map. They state what the code did, oddities included, at DPI
//! 1.0, 1.25, 1.5 and 2.0, in the three views, at the default and minimum sizes and on both sides
//! of the 294 DIP compact threshold.
//!
//! A press goes through the real `handle_mouse_button`, on a fixture that reports a window size,
//! DPI and native gesture outcome (`headless_view`, `headless_window_gesture`), so the shell handler
//! runs as it does in a window. The armed probe stops it where it would act: nothing is applied,
//! and hundreds of thousands of presses stay cheap. The tests at the end run presses unprobed.
//!
//! Search is hashed with no Source projection, its marker path: its text goes through the machine's
//! fonts, so a hash of it would pin the machine.
//!
//! `shell_pins/*.txt` hold the expected lines. `STICKYMD_SHELL_PINS=bless` rewrites them; any other
//! value is a directory that receives the full maps behind the hashes, for comparing two commits.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use std::fmt::Write as _;

use crate::controls::{
    ControlId, ControlLayout, ControlRect, ControlState, ToolbarVisual, paint_toolbar,
};
use crate::geometry::geometry;
use crate::scroll::{PreviewFacts, view_bars};
use crate::search::{SearchHit, SearchLayout, paint_search_overlay};
use stickymd_app::instruction::{WindowPlatformIntent, WindowPreferenceIntent};
use stickymd_app::interaction::{SearchField, SearchSession};
use stickymd_render::source::SourceScrollMetrics;
use tiny_skia::Pixmap;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton};
use winit::window::CursorIcon;

use super::headless_tests::{HeadlessApp, TestApp, fixture};
use stickymd_app::config::{ThemeMode, ViewMode};

/// One step of a press: where it stopped, or a preference it applied on the way there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PressRoute {
    PopupBlank,
    PopupField,
    PopupClosed,
    Button(ControlId),
    Window(WindowPlatformIntent),
    Preference(WindowPreferenceIntent),
    Search(SearchHit),
    Scroll,
    Preview,
    Source,
}

/// See `press_probe!`.
#[derive(Default)]
pub(super) struct PressProbe {
    armed: bool,
    routes: Vec<PressRoute>,
}

impl PressProbe {
    /// Records `route` if armed; then the press must stop here.
    pub(super) fn stops(&mut self, route: PressRoute) -> bool {
        if self.armed {
            self.routes.push(route);
        }
        self.armed
    }
}

const DPIS: [f64; 4] = [1.0, 1.25, 1.5, 2.0];
const MODES: [ViewMode; 3] = [ViewMode::Source, ViewMode::Split, ViewMode::Preview];
const CONTROLS: [ControlId; 9] = [
    ControlId::Source,
    ControlId::Split,
    ControlId::Preview,
    ControlId::ConvertMath,
    ControlId::Topmost,
    ControlId::Theme,
    ControlId::Opacity,
    ControlId::Collapse,
    ControlId::Close,
];

/// The window sizes of one DPI: default, minimum, and the widths just below and at the compact
/// threshold (a width of at least 294 DIP keeps the regular layout).
fn sizes(dpi: f64) -> [(&'static str, PhysicalSize<u32>); 4] {
    let dip = |width: f64, height: f64| {
        PhysicalSize::new((width * dpi).round() as u32, (height * dpi).round() as u32)
    };
    let threshold = (294.0 * dpi).ceil() as u32;
    let height = (200.0 * dpi).round() as u32;
    [
        ("default", dip(520.0, 680.0)),
        ("minimum", dip(220.0, 120.0)),
        ("compact", PhysicalSize::new(threshold - 1, height)),
        ("regular", PhysicalSize::new(threshold, height)),
    ]
}

/// One pinned configuration.
#[derive(Clone, Copy)]
struct Config {
    dpi: f64,
    label: &'static str,
    size: PhysicalSize<u32>,
    mode: ViewMode,
}

impl Config {
    fn all_at(dpi: f64) -> impl Iterator<Item = Self> {
        sizes(dpi).into_iter().flat_map(move |(label, size)| {
            MODES.into_iter().map(move |mode| Self {
                dpi,
                label,
                size,
                mode,
            })
        })
    }

    fn all() -> impl Iterator<Item = Self> {
        DPIS.into_iter().flat_map(Self::all_at)
    }

    fn key(self) -> String {
        format!(
            "dpi={} {}={}x{} mode={:?}",
            self.dpi, self.label, self.size.width, self.size.height, self.mode
        )
    }

    fn layout(self) -> ControlLayout {
        ControlLayout::new(self.mode, self.size, self.dpi)
    }

    fn view(self) -> crate::geometry::ViewGeometry {
        geometry(self.mode, self.size, self.dpi as f32)
    }
}

/// FNV-1a over 64-bit words: a stable digest, unlike `DefaultHasher`.
fn fnv(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut words = bytes.chunks_exact(8);
    for word in &mut words {
        hash ^= u64::from_le_bytes(word.try_into().unwrap());
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    for &byte in words.remainder() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Compares `actual` with the expected lines, or rewrites them (the module comment).
fn check(name: &str, expected: &str, actual: String) {
    if std::env::var("STICKYMD_SHELL_PINS").as_deref() == Ok("bless") {
        let path = format!(
            "{}/src/shell/shell_pins/{name}.txt",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::write(path, &actual).unwrap();
        return;
    }
    let differing = expected
        .lines()
        .zip(actual.lines())
        .filter(|(expected, actual)| expected != actual)
        .take(12)
        .map(|(expected, actual)| format!("- {expected}\n+ {actual}\n"))
        .collect::<String>();
    assert!(
        expected == actual,
        "{name}: {} expected lines, {} actual\n{differing}",
        expected.lines().count(),
        actual.lines().count()
    );
}

/// Writes a full map beside the hash when a directory was asked for.
fn dump(name: &str, text: &str) {
    if let Ok(dir) = std::env::var("STICKYMD_SHELL_PINS")
        && dir != "bless"
    {
        let path = std::path::Path::new(&dir).join(format!("{name}.txt"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

fn rect_text(rect: ControlRect) -> String {
    format!("{},{},{},{}", rect.x, rect.y, rect.width, rect.height)
}

/// Every rectangle `ControlLayout` hands out, and the split sync rectangle of the view.
#[test]
fn phase15_the_control_rectangles_stay_pinned() {
    let mut lines = String::new();
    for config in Config::all() {
        let layout = config.layout();
        write!(lines, "{}", config.key()).unwrap();
        for id in CONTROLS {
            write!(lines, " {id:?}={}", rect_text(layout.rect(id))).unwrap();
        }
        for (name, rect) in [
            ("toolbar", layout.toolbar),
            ("popup", layout.opacity_popup),
            ("slider", layout.opacity_slider),
            ("field", layout.opacity_input),
        ] {
            write!(lines, " {name}={}", rect_text(rect)).unwrap();
        }
        if let Some(sync) = layout.split_sync {
            write!(lines, " sync={}", rect_text(sync)).unwrap();
        }
        lines.push('\n');
    }
    check(
        "rectangles",
        include_str!("shell_pins/rectangles.txt"),
        lines,
    );
}

fn canvas(size: PhysicalSize<u32>) -> Pixmap {
    let mut pixmap = Pixmap::new(size.width, size.height).unwrap();
    pixmap.fill(tiny_skia::Color::from_rgba8(248, 246, 239, 255));
    pixmap
}

/// The toolbar with the visual states that change it: plain, with everything on in dark, and with
/// the popup open, showing the value and then a typed entry.
#[test]
fn phase15_the_toolbar_pixels_stay_pinned() {
    let mut lines = String::new();
    for config in Config::all() {
        let layout = config.layout();
        let plain = ToolbarVisual {
            mode: config.mode,
            topmost: false,
            dark: false,
            system_theme: false,
            diagnostic: false,
            emphasized: true,
            opacity_popup: false,
            opacity: 96,
            opacity_field: None,
            split_scroll_sync: true,
        };
        let mut typed = ControlState::new(96);
        typed.focus_input();
        typed.type_input("7x");
        for (name, visual) in [
            ("plain", plain),
            (
                "dark",
                ToolbarVisual {
                    topmost: true,
                    dark: true,
                    system_theme: true,
                    diagnostic: true,
                    emphasized: false,
                    split_scroll_sync: false,
                    ..plain
                },
            ),
            (
                "popup",
                ToolbarVisual {
                    opacity_popup: true,
                    opacity: 55,
                    ..plain
                },
            ),
            (
                "field",
                ToolbarVisual {
                    opacity_popup: true,
                    opacity_field: typed.focused_field(),
                    ..plain
                },
            ),
        ] {
            let mut pixmap = canvas(config.size);
            paint_toolbar(&mut pixmap, config.view(), &layout, visual);
            writeln!(lines, "{} {name}={:016x}", config.key(), fnv(pixmap.data())).unwrap();
        }
    }
    check("toolbar", include_str!("shell_pins/toolbar.txt"), lines);
}

/// Each view's bars, thin and thick, in both themes, at the document's ends and between them.
#[test]
fn phase15_the_scrollbar_pixels_stay_pinned() {
    let mut lines = String::new();
    for config in Config::all() {
        let view = config.view();
        write!(lines, "{}", config.key()).unwrap();
        for fraction in [0.0, 0.37, 1.0] {
            let source = SourceScrollMetrics {
                fraction,
                viewport_fraction: 0.2,
            };
            let height = view.preview.map_or(1, |pane| pane.height) as f32;
            let preview = PreviewFacts {
                document_height: height * 5.0,
                scroll_y: height * 4.0 * fraction as f32,
                current: true,
            };
            let bars = view_bars(config.dpi as f32, view, Some(source), Some(preview));
            for (emphasized, dark) in [(false, false), (true, false), (false, true), (true, true)] {
                let mut pixmap = canvas(config.size);
                for bar in bars.iter().flatten() {
                    bar.paint(&mut pixmap, emphasized, dark);
                }
                write!(lines, " {:016x}", fnv(pixmap.data())).unwrap();
            }
        }
        lines.push('\n');
    }
    check(
        "scrollbars",
        include_str!("shell_pins/scrollbars.txt"),
        lines,
    );
}

/// The Search surface in both themes, each field focused, with and without the replacement row and
/// the case toggle. A Preview has no Source pane and so no overlay.
#[test]
fn phase15_the_search_overlay_pixels_stay_pinned() {
    let mut lines = String::new();
    for config in Config::all().filter(|config| config.mode != ViewMode::Preview) {
        let pane = config.view().source.unwrap();
        write!(lines, "{}", config.key()).unwrap();
        for bits in 0..8_u8 {
            let [dark, replace, case] = [0, 1, 2].map(|bit| bits & (1 << bit) != 0);
            let scale = config.dpi as f32;
            let layout = SearchLayout::new(pane, scale, replace);
            let mut search = SearchSession::default();
            search.open = true;
            search.replace_visible = replace;
            search.case_sensitive = case;
            if replace {
                search.focused = SearchField::Replacement;
            }
            let mut pixmap = canvas(config.size);
            paint_search_overlay(&mut pixmap, &mut None, &search, Some(layout), scale, dark);
            write!(lines, " {:016x}", fnv(pixmap.data())).unwrap();
        }
        lines.push('\n');
    }
    check("search", include_str!("shell_pins/search.txt"), lines);
}

/// What is above the note while the map is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Overlay {
    Nothing,
    Popup,
    Search,
}

/// A fixture armed for one configuration.
fn armed(config: Config) -> HeadlessApp {
    let mut app = fixture("canonical text");
    app.press_probe.armed = true;
    place(&mut app, config, Overlay::Nothing);
    app
}

fn place(app: &mut TestApp, config: Config, overlay: Overlay) {
    app.headless_view = Some((config.size, config.dpi));
    app.app
        .config
        .update(|c| c.view_mode = config.mode)
        .unwrap();
    app.controls.opacity_popup_open = overlay == Overlay::Popup;
    app.app.search.open = overlay == Overlay::Search;
    app.app.search.replace_visible = overlay == Overlay::Search;
    app.sync_control_layout();
}

/// Where a press at (`x`, `y`) ends. What the shell changes on the way is put back.
fn route(app: &mut TestApp, overlay: Overlay, x: f64, y: f64) -> Vec<PressRoute> {
    app.cursor_position = PhysicalPosition::new(x, y);
    app.handle_mouse_button(ElementState::Pressed, MouseButton::Left);
    app.controls.opacity_popup_open = overlay == Overlay::Popup;
    app.controls.opacity_dragging = false;
    let mut routes = std::mem::take(&mut app.press_probe.routes);
    // A slider press is one thing wherever on the slider it lands; its values have their own tests.
    for route in &mut routes {
        if let PressRoute::Preference(WindowPreferenceIntent::PreviewOpacity(value)) = route {
            *value = 0;
        }
    }
    routes
}

/// The route as the map states it.
fn route_text(routes: &[PressRoute]) -> String {
    if routes.is_empty() {
        return "Nothing".to_owned();
    }
    let step = |route: &PressRoute| match route {
        PressRoute::Preference(WindowPreferenceIntent::PreviewOpacity(_)) => "Slider".to_owned(),
        PressRoute::Preference(WindowPreferenceIntent::SetSplitScrollSync(_)) => {
            "SplitSync".to_owned()
        }
        PressRoute::Window(WindowPlatformIntent::RequestDrag) => "Drag".to_owned(),
        PressRoute::Window(WindowPlatformIntent::RequestResize(edge)) => format!("Resize{edge:?}"),
        PressRoute::Button(id) => format!("{id:?}Button"),
        other => format!("{other:?}"),
    };
    routes.iter().map(step).collect::<Vec<_>>().join("+")
}

/// The rows a map covers, as integers: every row from above the window through the popup, the
/// bottom resize band and just past it, and a sample of the note between them.
fn rows(config: Config) -> Vec<i64> {
    let height = i64::from(config.size.height);
    let top = (92.0 * config.dpi).ceil() as i64 + 2;
    let bottom = height - (6.0 * config.dpi).ceil() as i64 - 2;
    let mut rows = (-1..=top.min(height + 1)).collect::<Vec<_>>();
    rows.extend((top + 1..bottom).step_by(31));
    rows.extend(bottom.max(top + 1)..=height + 1);
    rows
}

/// One row of a map as `start..end route` runs; `value` reads the point `(x, offset + y)`.
fn row_runs<T: PartialEq>(
    config: Config,
    offset: f64,
    mut value: impl FnMut(f64) -> T,
    text: impl Fn(&T) -> String,
) -> String {
    let mut runs = String::new();
    let mut open: Option<(i64, T)> = None;
    let end = i64::from(config.size.width) + 1;
    for x in -1..=end {
        let here = value(x as f64 + offset);
        match &open {
            Some((_, current)) if *current == here => {}
            _ => {
                if let Some((start, current)) = open.replace((x, here)) {
                    write!(runs, " {start}..{x} {}", text(&current)).unwrap();
                }
            }
        }
    }
    if let Some((start, current)) = open {
        write!(runs, " {start}..{} {}", end + 1, text(&current)).unwrap();
    }
    runs
}

/// `key` and the digest of `map`, with equal consecutive rows merged in the map.
fn digest(name: &str, key: &str, map: &[(i64, String)]) -> String {
    let mut text = format!("{name} {key}\n");
    let mut iter = map.iter().peekable();
    while let Some((first, runs)) = iter.next() {
        let mut last = *first;
        while let Some((row, _)) = iter.peek().filter(|(_, next)| next == runs) {
            last = *row;
            iter.next();
        }
        writeln!(text, "{first}..={last}:{runs}").unwrap();
    }
    let line = format!("{key} {:016x}", fnv(text.as_bytes()));
    MAPS.with_borrow_mut(|maps| maps.push_str(&text));
    line + "\n"
}

std::thread_local! {
    /// The full maps of this thread, for `dump`.
    static MAPS: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// Runs `build` for each DPI on its own thread and joins the lines in DPI order.
fn per_dpi(name: &str, build: impl Fn(f64) -> String + Sync) -> String {
    std::thread::scope(|scope| {
        let threads = DPIS.map(|dpi| {
            let build = &build;
            scope.spawn(move || {
                let lines = build(dpi);
                (lines, MAPS.take())
            })
        });
        let mut lines = String::new();
        let mut maps = String::new();
        for thread in threads {
            let (part, map) = thread.join().unwrap();
            lines.push_str(&part);
            maps.push_str(&map);
        }
        dump(name, &maps);
        lines
    })
}

/// The routed map on the integer grid (`offset` 0) or on pixel centres (`offset` 0.5), under each
/// overlay.
fn press_map(dpi: f64, offset: f64) -> String {
    let mut lines = String::new();
    for config in Config::all_at(dpi) {
        let mut app = armed(config);
        for overlay in [Overlay::Nothing, Overlay::Popup, Overlay::Search] {
            if overlay == Overlay::Search && config.mode == ViewMode::Preview {
                continue;
            }
            place(&mut app, config, overlay);
            let map = rows(config)
                .into_iter()
                .map(|row| {
                    let y = row as f64 + offset;
                    let runs = row_runs(
                        config,
                        offset,
                        |x| route(&mut app, overlay, x, y),
                        |routes| route_text(routes),
                    );
                    (row, runs)
                })
                .collect::<Vec<_>>();
            let key = format!("{} overlay={overlay:?}", config.key());
            lines.push_str(&digest("press", &key, &map));
        }
    }
    lines
}

#[test]
fn phase15_the_routed_press_map_on_the_integer_grid_stays_pinned() {
    let lines = per_dpi("press-integer", |dpi| press_map(dpi, 0.0));
    check(
        "press-integer",
        include_str!("shell_pins/press-integer.txt"),
        lines,
    );
}

#[test]
fn phase15_the_routed_press_map_on_pixel_centres_stays_pinned() {
    let lines = per_dpi("press-centres", |dpi| press_map(dpi, 0.5));
    check(
        "press-centres",
        include_str!("shell_pins/press-centres.txt"),
        lines,
    );
}

/// The edges of every rectangle a press can hit, with the points a quarter pixel either side, each
/// against the other axis's edges and centre.
fn edge_points(config: Config) -> Vec<(f64, f64)> {
    let layout = config.layout();
    let border = 6.0 * config.dpi;
    let (width, height) = (f64::from(config.size.width), f64::from(config.size.height));
    let mut rects = CONTROLS.map(|id| layout.rect(id)).to_vec();
    rects.extend([
        layout.toolbar,
        layout.opacity_popup,
        layout.opacity_slider,
        layout.opacity_input,
        ControlRect {
            x: border,
            y: border,
            width: width - 2.0 * border,
            height: height - 2.0 * border,
        },
    ]);
    rects.extend(layout.split_sync);
    let mut points = Vec::new();
    for rect in rects {
        let axis = |start: f64, length: f64| {
            [start, start + length]
                .into_iter()
                .flat_map(|edge| [edge - 0.25, edge, edge + 0.25])
                .chain([start + length / 2.0])
                .collect::<Vec<_>>()
        };
        for x in axis(rect.x, rect.width) {
            for y in axis(rect.y, rect.height) {
                points.push((x, y));
            }
        }
    }
    points
}

#[test]
fn phase15_the_routed_press_map_on_rectangle_edges_stays_pinned() {
    let lines = per_dpi("press-edges", |dpi| {
        let mut lines = String::new();
        for config in Config::all_at(dpi) {
            let mut app = armed(config);
            for overlay in [Overlay::Nothing, Overlay::Popup] {
                place(&mut app, config, overlay);
                let mut text = format!("edges {} overlay={overlay:?}\n", config.key());
                for (x, y) in edge_points(config) {
                    let routes = route(&mut app, overlay, x, y);
                    writeln!(text, "{x},{y} {}", route_text(&routes)).unwrap();
                }
                writeln!(
                    lines,
                    "{} overlay={overlay:?} {:016x}",
                    config.key(),
                    fnv(text.as_bytes())
                )
                .unwrap();
                MAPS.with_borrow_mut(|maps| maps.push_str(&text));
            }
        }
        lines
    });
    check(
        "press-edges",
        include_str!("shell_pins/press-edges.txt"),
        lines,
    );
}

/// At 125% the control layout's toolbar ends at 42.5 and the view's at 43: a point between them,
/// which Windows' integer positions never give, is neither toolbar nor pane and nothing takes it.
#[test]
fn phase15_a_fractional_point_under_the_toolbar_at_125_percent_stays_pinned() {
    let size = PhysicalSize::new(650, 850);
    let config = Config {
        dpi: 1.25,
        label: "default",
        size,
        mode: ViewMode::Source,
    };
    let mut app = armed(config);
    for (x, y, expected) in [
        (100.0, 42.75, "Nothing"),
        (20.0, 42.75, "Nothing"),
        (140.0, 42.75, "Nothing"),
        (300.0, 42.75, "Nothing"),
        (100.0, 42.0, "Drag"),
        (100.0, 43.0, "Source"),
    ] {
        assert_eq!(
            route_text(&route(&mut app, Overlay::Nothing, x, y)),
            expected,
            "({x},{y})"
        );
    }
}

/// The cursor a still pointer shows, on the integer grid.
#[test]
fn phase15_the_hover_map_stays_pinned() {
    let lines = per_dpi("hover", |dpi| {
        let mut lines = String::new();
        for config in Config::all_at(dpi) {
            let mut app = armed(config);
            for overlay in [Overlay::Nothing, Overlay::Popup] {
                place(&mut app, config, overlay);
                let map = rows(config)
                    .into_iter()
                    .map(|row| {
                        let runs = row_runs(
                            config,
                            0.0,
                            |x| {
                                app.cursor_position = PhysicalPosition::new(x, row as f64);
                                app.cursor_at_pointer()
                            },
                            |cursor| format!("{cursor:?}"),
                        );
                        (row, runs)
                    })
                    .collect::<Vec<_>>();
                let key = format!("{} overlay={overlay:?}", config.key());
                lines.push_str(&digest("hover", &key, &map));
            }
        }
        lines
    });
    check("hover", include_str!("shell_pins/hover.txt"), lines);
}

/// The rest of these presses run unprobed, with their real effects.
fn windowed(text: &str, mode: ViewMode, size: PhysicalSize<u32>) -> HeadlessApp {
    let mut app = fixture(text);
    app.app.editor.focused = true;
    app.headless_view = Some((size, 1.0));
    app.app.config.update(|c| c.view_mode = mode).unwrap();
    app.sync_control_layout();
    app
}

fn press(app: &mut TestApp, x: f64, y: f64) {
    app.cursor_position = PhysicalPosition::new(x, y);
    app.handle_mouse_button(ElementState::Pressed, MouseButton::Left);
}

fn press_control(app: &mut TestApp, id: ControlId) {
    let rect = app.control_layout.as_ref().unwrap().rect(id);
    press(app, rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
}

/// The popup's own surface takes a press and nothing under it sees one: not the note, not the
/// Preview it covers, not the resize border under it at the minimum size, not the old mode branch.
#[test]
fn phase15_a_press_on_the_popup_blank_does_not_fall_through() {
    for (mode, size, point) in [
        (ViewMode::Source, PhysicalSize::new(520, 680), (300.0, 40.0)),
        (
            ViewMode::Preview,
            PhysicalSize::new(520, 680),
            (300.0, 40.0),
        ),
        (ViewMode::Split, PhysicalSize::new(520, 680), (300.0, 40.0)),
        (ViewMode::Source, PhysicalSize::new(220, 120), (1.0, 40.0)),
    ] {
        let mut app = windowed("canonical text", mode, size);
        app.controls.opacity_popup_open = true;
        app.preview_focused = false;
        let selection = app.app.editor.selection;
        let config = app.app.config.current().clone();
        press(&mut app, point.0, point.1);
        assert!(app.controls.opacity_popup_open, "{mode:?} {size:?}");
        assert!(!app.preview_focused, "{mode:?} {size:?}");
        assert!(!app.move_resize_active, "{mode:?} {size:?}");
        assert_eq!(app.app.editor.selection, selection);
        assert_eq!(*app.app.config.current(), config, "{mode:?} {size:?}");
    }
}

/// A press outside the popup closes it and the control under it acts once: each of these moves its
/// setting one step, and the Opacity button, which would reopen the popup, leaves it closed.
#[test]
fn phase15_a_press_that_closes_the_popup_acts_once() {
    let size = PhysicalSize::new(520, 680);
    let open = |mode| {
        let mut app = windowed("canonical text", mode, size);
        app.controls.opacity_popup_open = true;
        app
    };

    let mut app = open(ViewMode::Source);
    press_control(&mut app, ControlId::Topmost);
    assert!(!app.controls.opacity_popup_open);
    assert!(app.app.config.current().always_on_top, "toggled once");

    let mut app = open(ViewMode::Source);
    assert_eq!(app.app.config.current().theme, ThemeMode::Light);
    press_control(&mut app, ControlId::Theme);
    assert!(!app.controls.opacity_popup_open);
    assert_eq!(
        app.app.config.current().theme,
        ThemeMode::System,
        "one step"
    );

    let mut app = open(ViewMode::Source);
    press_control(&mut app, ControlId::Split);
    assert!(!app.controls.opacity_popup_open);
    assert_eq!(app.app.config.current().view_mode, ViewMode::Split);

    let mut app = open(ViewMode::Source);
    press_control(&mut app, ControlId::Opacity);
    assert!(!app.controls.opacity_popup_open, "not reopened");

    let mut app = windowed("canonical text", ViewMode::Source, size);
    press_control(&mut app, ControlId::Opacity);
    assert!(app.controls.opacity_popup_open, "a closed popup opens");
}

/// The native gesture's outcome does not change who took the press: a drag or resize that fails
/// shows why and ends there. It must not reach the old mode branch, which reads a blank at (35, 17)
/// as the Source button, nor the note.
#[test]
fn phase15_a_failed_drag_or_resize_is_consumed() {
    let size = PhysicalSize::new(520, 680);
    for (started, point) in [
        (true, (35.0, 17.0)),
        (false, (35.0, 17.0)),
        (true, (260.0, 1.0)),
        (false, (260.0, 1.0)),
        (true, (1.0, 340.0)),
        (false, (1.0, 340.0)),
    ] {
        let mut app = windowed("canonical text", ViewMode::Preview, size);
        app.headless_window_gesture = Some(started);
        let focused = app.preview_focused;
        press(&mut app, point.0, point.1);
        assert_eq!(
            app.app.config.current().view_mode,
            ViewMode::Preview,
            "{point:?}"
        );
        assert_eq!(app.preview_focused, focused, "{point:?}");
        assert_eq!(app.move_resize_active, started, "{point:?}");
        assert_eq!(app.diagnostic.is_some(), !started, "{point:?}");
    }
}

/// The layout follows what it is laid out for: the window's size and DPI, which a resize brings
/// (a DPI change without a new size too), and the view.
#[test]
fn phase15_the_layout_follows_the_size_the_dpi_and_the_view() {
    let size = PhysicalSize::new(520, 680);
    let mut app = windowed("canonical text", ViewMode::Source, size);
    let close = |app: &TestApp| app.control_layout.as_ref().unwrap().rect(ControlId::Close);
    let at_start = close(&app);

    let wider = PhysicalSize::new(800, 680);
    app.headless_view = Some((wider, 1.0));
    app.resize(wider);
    assert_eq!(close(&app).x, at_start.x + 280.0);

    app.headless_view = Some((wider, 2.0));
    app.resize(wider);
    assert_eq!(close(&app).height, 56.0, "the DPI alone changed");

    assert_eq!(app.control_layout.as_ref().unwrap().split_sync, None);
    press_control(&mut app, ControlId::Split);
    assert_eq!(app.app.config.current().view_mode, ViewMode::Split);
    assert!(app.control_layout.as_ref().unwrap().split_sync.is_some());
}

/// A still pointer on the toolbar shows the hand on a button and the arrow on the blank that moves
/// the window, and the resize cursor where the band takes the button's outer rows.
#[test]
fn phase15_the_toolbar_blank_shows_the_arrow() {
    let mut app = windowed(
        "canonical text",
        ViewMode::Source,
        PhysicalSize::new(520, 680),
    );
    let mut at = |x, y| {
        app.cursor_position = PhysicalPosition::new(x, y);
        app.cursor_at_pointer()
    };
    assert_eq!(at(20.0, 17.0), CursorIcon::Pointer, "the Source button");
    assert_eq!(at(35.0, 17.0), CursorIcon::Default, "a gap between buttons");
    assert_eq!(at(250.0, 17.0), CursorIcon::Default, "the middle");
    assert_eq!(at(250.0, 32.0), CursorIcon::Default, "below the buttons");
    assert_eq!(at(20.0, 4.0), CursorIcon::NResize, "the band over a button");
}

/// With shell input off nothing in the window takes a press, the toolbar included.
#[test]
fn phase15_no_press_acts_while_shell_input_is_off() {
    let config = Config {
        dpi: 1.0,
        label: "default",
        size: PhysicalSize::new(520, 680),
        mode: ViewMode::Preview,
    };
    let mut app = armed(config);
    app.shell_input_enabled = false;
    for (x, y) in [(20.0, 17.0), (35.0, 17.0), (1.0, 1.0), (200.0, 300.0)] {
        assert_eq!(route(&mut app, Overlay::Nothing, x, y), []);
    }
}
