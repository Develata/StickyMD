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
    let (verdict, detail) = pixels::strip_visible(&images[0], &images[1], &images[2]);
    record(d, &name, verdict, format!("{rect:?}; {detail}"));
    Ok(())
}

fn counters<B: Backend>(d: &mut Driver<'_, B>) -> Result<[u64; 4], String> {
    match d.op(Op::Presenter)? {
        Value::Counters(c) => Ok(c),
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
    let before_edit = counters(d)?;
    d.type_text("x")?;
    // Past the autosave's 650 ms debounce and its save.
    d.wait(1500)?;
    let after_edit = counters(d)?;
    let mut still = (after_edit, after_edit);
    for _ in 0..QUIET_TRIES {
        d.wait(300)?;
        still = (still.1, counters(d)?);
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
    while d.backend.now() - start < WATCH_MS {
        let at = d.backend.now();
        match capture(d, region, Shot::Alone)? {
            Ok(image) => frames.push((at - start, image)),
            Err(reason) => {
                d.row.skip("caret blinks in inverse colours", &reason);
                break;
            }
        }
        d.wait(EVERY_MS)?;
    }
    let idle_end = counters(d)?;
    if !frames.is_empty() {
        let (verdict, detail) = pixels::caret_blinks(&frames);
        record(
            d,
            "caret blinks in inverse colours",
            verdict,
            format!("{region:?}; {detail}"),
        );
    }
    let (verdict, detail) =
        pixels::idle_presents_nothing((before_edit, after_edit), still, (idle_start, idle_end));
    record(d, "caret blinking presents no frame", verdict, detail);
    Ok(())
}
