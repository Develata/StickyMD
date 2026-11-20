//! Scenario steps judged from captured pixels: W02's collapsed strip as the user sees it, and
//! W10's caret blink with the presenter counters around it.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    model::{Edge, Point, Rect, Verdict, Window},
    pixels::{self, Image, Shot},
    scenarios::{Backend, Driver, Op, Value},
};

fn record<B: Backend>(d: &mut Driver<'_, B>, name: &str, verdict: Verdict, detail: String) {
    if verdict == Verdict::NotTested {
        d.row.skip(name, &detail);
    } else {
        d.row.check(name, verdict == Verdict::Pass, detail);
    }
}

/// A capture, or the reason the backend could not take one.
fn capture<B: Backend>(
    d: &mut Driver<'_, B>,
    rect: Rect,
    shot: Shot,
) -> Result<Result<Image, String>, String> {
    match d.op(Op::Capture(rect, shot))? {
        Value::Image(image) => Ok(Ok(image)),
        Value::Text(reason) => Ok(Err(reason)),
        v => Err(format!("expected an image, got {v:?}")),
    }
}

/// `kCGDockWindowLevel`.
const DOCK_LEVEL: i32 = 20;

/// Whether a window in front (owner, bounds, layer) may hide part of `strip` on `screen`. The
/// Dock keeps one screen-sized window at the Dock level in front of every application window;
/// only the Dock bar in it is painted (macOS 27.2 VM, 2026-11-19), so it is not counted here. A
/// Dock bar over the strip still shows in the pixels: the composite then differs from the window
/// alone.
pub(super) fn covers(window: &(String, Rect, i32), strip: Rect, screen: Rect) -> bool {
    let (owner, bounds, layer) = window;
    let dock_overlay = owner == "Dock" && *layer == DOCK_LEVEL && *bounds == screen;
    !dock_overlay && intersect(*bounds, strip).is_some()
}

fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let x = a.origin.x.max(b.origin.x);
    let y = a.origin.y.max(b.origin.y);
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    (right > x && bottom > y).then_some(Rect {
        origin: Point { x, y },
        size: super::model::Size {
            width: right - x,
            height: bottom - y,
        },
    })
}

/// W02: the collapsed window's strip on screen is visible: nothing covers it, and it changes
/// what is on screen there (`pixels::strip_visible`).
pub(super) fn strip<B: Backend>(
    d: &mut Driver<'_, B>,
    edge: Edge,
    screen: Rect,
    w: &Window,
) -> Result<(), String> {
    let name = format!("{} strip visible", edge.name());
    let Some(rect) = intersect(w.bounds, screen) else {
        d.row
            .skip(&name, "the collapsed window does not meet the screen");
        return Ok(());
    };
    // Occlusion from the window list, independent of the pixels: a window in front that meets
    // the strip (a notification, or one of the same colour) covers it.
    match d.op(Op::Above)? {
        Value::Covering(list) => {
            let covering: Vec<_> = list.iter().filter(|w| covers(w, rect, screen)).collect();
            if !covering.is_empty() {
                d.row
                    .skip(&name, &format!("{rect:?} is covered by {covering:?}"));
                return Ok(());
            }
        }
        Value::Text(reason) => {
            d.row.skip(&name, &reason);
            return Ok(());
        }
        v => return Err(format!("expected the windows in front, got {v:?}")),
    }
    let mut images = Vec::new();
    for shot in [Shot::Composite, Shot::Alone, Shot::Below] {
        match capture(d, rect, shot)? {
            Ok(image) => images.push(image),
            Err(reason) => {
                d.row.skip(&name, &reason);
                return Ok(());
            }
        }
    }
    // The stacking again after the captures: a window that came in front meanwhile could have
    // been captured as the strip.
    match d.op(Op::Above)? {
        Value::Covering(list) => {
            let covering: Vec<_> = list.iter().filter(|w| covers(w, rect, screen)).collect();
            if !covering.is_empty() {
                d.row.skip(
                    &name,
                    &format!("{rect:?} was covered by {covering:?} after the captures"),
                );
                return Ok(());
            }
        }
        Value::Text(reason) => {
            d.row.skip(&name, &reason);
            return Ok(());
        }
        v => return Err(format!("expected the windows in front, got {v:?}")),
    }
    let (verdict, detail) = pixels::strip_visible(&images[0], &images[1], &images[2]);
    record(d, &name, verdict, format!("{rect:?}; {detail}"));
    Ok(())
}

/// The presenter counters, or the reason they could not be read.
fn counters<B: Backend>(d: &mut Driver<'_, B>) -> Result<Result<[u64; 4], String>, String> {
    match d.op(Op::Presenter)? {
        Value::Counters(c) => Ok(Ok(c)),
        Value::Text(reason) => Ok(Err(reason)),
        v => Err(format!("expected presenter counters, got {v:?}")),
    }
}

/// How long the caret is watched, and how often (ms): several half periods of about 550 ms.
const WATCH_MS: u64 = 3600;
const EVERY_MS: u64 = 60;

/// W10: with the editor focused and the pointer away, the caret blinks in colours inverse to what
/// is under it, and the blinking presents no frame. One typed character first shows the counters
/// are live; then snapshots 300 ms apart until two are equal (at most `QUIET_TRIES`) show the
/// product has gone quiet after the edit and its autosave, and that a snapshot presents nothing
/// itself. Without such a pair the idle check is NOT_TESTED.
pub(super) fn caret<B: Backend>(d: &mut Driver<'_, B>) -> Result<(), String> {
    const QUIET_TRIES: usize = 6;
    d.op(Op::Focus)?;
    d.wait(500)?;
    let s = d.screen()?;
    d.op(Op::Move(Point {
        x: s.right() - 20.0,
        y: s.bottom() - 100.0,
    }))?;
    // A snapshot that cannot be read leaves the idle check NOT_TESTED with its reason.
    let mut unavailable = None;
    let mut snapshot = |d: &mut Driver<'_, B>| -> Result<[u64; 4], String> {
        Ok(counters(d)?.unwrap_or_else(|reason| {
            unavailable.get_or_insert(reason);
            [0; 4]
        }))
    };
    let before_edit = snapshot(d)?;
    d.type_text("x")?;
    // Past the autosave's 650 ms debounce and its save.
    d.wait(1500)?;
    let after_edit = snapshot(d)?;
    let mut still = (after_edit, after_edit);
    for _ in 0..QUIET_TRIES {
        d.wait(300)?;
        still = (still.1, snapshot(d)?);
        if still.0 == still.1 {
            break;
        }
    }
    let idle_start = still.1;
    let w = d.window()?;
    // The text's first lines, below the toolbar: where the caret is after the typed character.
    let region = Rect {
        origin: Point {
            x: w.bounds.origin.x,
            y: w.bounds.origin.y + 34.0,
        },
        size: super::model::Size {
            width: (w.bounds.size.width / 2.0).min(260.0),
            height: 166.0,
        },
    };
    let start = d.backend.now();
    let mut frames = Vec::new();
    // When each capture was asked for and when it returned: the frame was taken in between.
    let mut spans = Vec::new();
    let mut broken = None;
    while d.backend.now() - start < WATCH_MS {
        let at = d.backend.now();
        match capture(d, region, Shot::Alone)? {
            Ok(image) => {
                spans.push((at, d.backend.now()));
                frames.push((at - start, image));
            }
            Err(reason) => {
                broken = Some(reason);
                break;
            }
        }
        d.wait(EVERY_MS)?;
    }
    let end = d.backend.now();
    let idle_end = snapshot(d)?;
    let name = "caret blinks in inverse colours";
    // The longest stretch the frames may leave unseen: from the start to the first return, from
    // one request to the next return, and from the last request to the end of the watch.
    let widest = match (spans.first(), spans.last()) {
        (Some(first), Some(last)) => spans
            .windows(2)
            .map(|w| w[1].1 - w[0].0)
            .chain([first.1 - start, end - last.0])
            .max()
            .unwrap_or(u64::MAX),
        _ => u64::MAX,
    };
    match broken {
        // An interrupted capture judges nothing: the frames taken so far are not the watch.
        Some(reason) => d.row.skip(name, &reason),
        None if widest > pixels::MAX_FRAME_GAP_MS => d.row.skip(
            name,
            &format!(
                "{} frames, two of them up to {widest} ms apart (request to return), over {} ms",
                frames.len(),
                pixels::MAX_FRAME_GAP_MS
            ),
        ),
        None => {
            let (verdict, detail) = pixels::caret_blinks(&frames);
            record(d, name, verdict, format!("{region:?}; {detail}"));
        }
    }
    let (verdict, detail) = match unavailable {
        Some(reason) => (Verdict::NotTested, reason),
        None => {
            pixels::idle_presents_nothing((before_edit, after_edit), still, (idle_start, idle_end))
        }
    };
    record(d, "caret blinking presents no frame", verdict, detail);
    Ok(())
}
