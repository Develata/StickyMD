//! Pure judgements over captured pixels (the captures themselves are native). Each judge gives
//! FAIL only for an observed violation; captures that cannot be compared, or a scene that changed
//! under the check, are NOT_TESTED.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::model::Verdict;

/// What a capture shows of a rectangle of the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Shot {
    /// The product window alone: other windows cannot appear in it, so it suits judging the
    /// window's own content.
    Alone,
    /// The on-screen composite: what the user sees.
    Composite,
    /// The on-screen scene below the product window.
    Below,
}

/// RGBA pixels, rows from top to bottom, as captured.
#[derive(Clone, PartialEq)]
pub(super) struct Image {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// Traces name the size only; the pixels would flood the evidence.
impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Image {}x{}", self.width, self.height)
    }
}

impl Image {
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.width + x) * 4;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]]
    }
    /// The fraction of pixels with any opacity. CoreGraphics returns a transparent black image
    /// for a scene it cannot read.
    pub fn opaque_share(&self) -> f64 {
        let n = self.width * self.height;
        if n == 0 {
            return 0.0;
        }
        self.rgba.chunks_exact(4).filter(|p| p[3] > 0).count() as f64 / n as f64
    }
    fn pixels(&self) -> impl Iterator<Item = [u8; 3]> + '_ {
        self.rgba.chunks_exact(4).map(|p| [p[0], p[1], p[2]])
    }
    /// The fraction of pixels whose largest channel difference from `other` exceeds `tolerance`;
    /// `None` when the two do not have the same size (they cannot be compared).
    pub fn differing(&self, other: &Image, tolerance: u8) -> Option<f64> {
        if (self.width, self.height) != (other.width, other.height) || self.width * self.height == 0
        {
            return None;
        }
        let differ = self
            .pixels()
            .zip(other.pixels())
            .filter(|(a, b)| far(*a, *b, tolerance))
            .count();
        Some(differ as f64 / (self.width * self.height) as f64)
    }
}

fn far(a: [u8; 3], b: [u8; 3], tolerance: u8) -> bool {
    a.iter().zip(b).any(|(x, y)| x.abs_diff(y) > tolerance)
}

fn inverse(a: [u8; 3]) -> [u8; 3] {
    [255 - a[0], 255 - a[1], 255 - a[2]]
}

/// Colour noise the capture path may add (colour matching, rounding); measured in the VM.
pub(super) const TOLERANCE: u8 = 6;

/// W02: the collapsed strip is visible when nothing covers it (the on-screen composite of the
/// strip's rectangle equals the window alone there) and it changes the scene (the composite
/// differs from the scene below the window). A covered strip is NOT_TESTED (the scene, e.g. a
/// notification); a strip that changes nothing on screen fails. A scene below that is mostly
/// transparent was not read, so the comparison with it proves nothing (NOT_TESTED).
pub(super) fn strip_visible(composite: &Image, alone: &Image, below: &Image) -> (Verdict, String) {
    let covered = composite.differing(alone, TOLERANCE);
    let drawn = composite.differing(below, TOLERANCE);
    let readable = below.opaque_share() >= 0.5;
    let verdict = match (covered, drawn) {
        _ if !readable => Verdict::NotTested,
        (Some(c), Some(d)) if c <= 0.02 && d >= 0.5 => Verdict::Pass,
        (Some(c), Some(_)) if c <= 0.02 => Verdict::Fail,
        _ => Verdict::NotTested,
    };
    (
        verdict,
        format!(
            "{}x{} px; differs from the window alone in {covered:?} and from the scene below in \
             {drawn:?} of the pixels (tolerance {TOLERANCE}); scene below opaque in {:.2}",
            composite.width,
            composite.height,
            below.opaque_share()
        ),
    )
}

/// Half a blink period must lie in this range (ms); macOS blinks the caret about every 550 ms.
const HALF_PERIOD_MS: (u64, u64) = (300, 900);

/// The caret's shape on a 2x display (px): about 1–2 pt wide, one line tall.
const CARET_WIDTH_PX: usize = 8;
const CARET_HEIGHT_PX: (usize, usize) = (16, 120);

/// The longest gap between two frames that still times a half period of the caret.
pub(super) const MAX_FRAME_GAP_MS: u64 = HALF_PERIOD_MS.0 / 2;

/// W10: frames of one window region at increasing times. The pixels that alternate between two
/// states must form one caret-shaped region (at most `CARET_WIDTH_PX` wide, a line tall, mostly
/// filled), each pixel's two states must be each other's inverse (反色), the caret must be seen
/// shown and hidden at least twice, and each complete half period must lie in `HALF_PERIOD_MS`.
/// A steady or absent caret, an alternating region that is not caret-shaped (a whole layer
/// flipping), or alternation in colours that are not inverse fails; pixels that change through
/// more than two states, or a frame gap too long to time a half period, are NOT_TESTED; an
/// observed non-inverse or misshapen alternation fails whatever the gaps, and a missing
/// alternation fails only when the frames cover the watch.
pub(super) fn caret_blinks(frames: &[(u64, Image)]) -> (Verdict, String) {
    let Some((_, first)) = frames.first() else {
        return (Verdict::NotTested, "no frames".into());
    };
    if frames
        .iter()
        .any(|(_, f)| (f.width, f.height) != (first.width, first.height))
    {
        return (Verdict::NotTested, "frames differ in size".into());
    }
    let longest = frames
        .windows(2)
        .map(|w| w[1].0.saturating_sub(w[0].0))
        .max()
        .unwrap_or(u64::MAX);
    // Pixels that change across the frames: two-state ones (inverse or not), and the others.
    let (mut alternating, mut other) = (Vec::new(), 0usize);
    for i in 0..first.width * first.height {
        let (x, y) = (i % first.width, i / first.width);
        let a = first.pixel(x, y);
        let b = frames
            .iter()
            .map(|(_, f)| f.pixel(x, y))
            .find(|p| far(*p, a, TOLERANCE));
        let Some(b) = b else { continue };
        if frames.iter().any(|(_, f)| {
            let p = f.pixel(x, y);
            far(p, a, TOLERANCE) && far(p, b, TOLERANCE)
        }) {
            other += 1;
        } else {
            alternating.push((x, y, a, !far(b, inverse(a), TOLERANCE * 2)));
        }
    }
    let not_inverse = alternating.iter().filter(|p| !p.3).count();
    let (width, height) = match (
        alternating.iter().map(|p| p.0).min(),
        alternating.iter().map(|p| p.0).max(),
        alternating.iter().map(|p| p.1).min(),
        alternating.iter().map(|p| p.1).max(),
    ) {
        (Some(x0), Some(x1), Some(y0), Some(y1)) => (x1 - x0 + 1, y1 - y0 + 1),
        _ => (0, 0),
    };
    let caret_shaped = width <= CARET_WIDTH_PX
        && (CARET_HEIGHT_PX.0..=CARET_HEIGHT_PX.1).contains(&height)
        && alternating.len() * 2 >= width * height;
    // Per frame: the share of alternating pixels in their first-frame state.
    let states: Vec<(u64, bool)> = frames
        .iter()
        .map(|(t, f)| {
            let same = alternating
                .iter()
                .filter(|(x, y, a, _)| !far(f.pixel(*x, *y), *a, TOLERANCE))
                .count();
            (*t, same * 2 >= alternating.len())
        })
        .collect();
    let changes: Vec<u64> = states
        .windows(2)
        .filter(|w| w[0].1 != w[1].1)
        .map(|w| w[1].0)
        .collect();
    let halves: Vec<u64> = changes.windows(2).map(|w| w[1] - w[0]).collect();
    let detail = format!(
        "{} frames, longest gap {longest}ms; {} alternating pixels in a {width}x{height} px \
         region ({not_inverse} not inverse), {other} other changing; state changes at \
         {changes:?}ms, half periods {halves:?}ms",
        frames.len(),
        alternating.len()
    );
    let verdict = if !alternating.is_empty() && (!caret_shaped || not_inverse > 0) {
        Verdict::Fail
    } else if longest > MAX_FRAME_GAP_MS || other > 0 {
        Verdict::NotTested
    } else if alternating.is_empty() || changes.len() < 4 {
        Verdict::Fail
    } else if halves
        .iter()
        .all(|h| (HALF_PERIOD_MS.0..=HALF_PERIOD_MS.1).contains(h))
    {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    (verdict, detail)
}

/// W10: the presenter's `[presented, deferred, created, failed]` counters around an idle stretch.
/// `edit` are the counters before and after one typed character (they must grow: the counters
/// are live and the editor has focus); `still` two snapshots taken back to back (equal: taking a
/// snapshot presents nothing); `idle` the snapshots around the idle stretch, in which presented,
/// deferred and failed must not grow.
pub(super) fn idle_presents_nothing(
    edit: ([u64; 4], [u64; 4]),
    still: ([u64; 4], [u64; 4]),
    idle: ([u64; 4], [u64; 4]),
) -> (Verdict, String) {
    let grew = |(a, b): ([u64; 4], [u64; 4]), i: usize| b[i] > a[i];
    let detail = format!("edit {edit:?}; back to back {still:?}; idle {idle:?}");
    let verdict = if !grew(edit, 0) || still.0 != still.1 {
        Verdict::NotTested
    } else if [0, 1, 3].iter().any(|i| grew(idle, *i)) {
        Verdict::Fail
    } else {
        Verdict::Pass
    };
    (verdict, detail)
}
