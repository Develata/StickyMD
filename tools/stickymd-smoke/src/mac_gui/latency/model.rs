//! The recording's format, the key-to-frame pairing and the statistics, with no I/O.
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

/// One key-down as the product saw it: the event's own timestamp and when the main thread got it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Key {
    pub event: u64,
    pub seen: u64,
}

/// One presented frame: when `present` returned and how long drawing it took.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Frame {
    pub presented: u64,
    pub drawn: u64,
}

impl Frame {
    fn draw_start(self) -> u64 {
        self.presented.saturating_sub(self.drawn)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Recording {
    pub keys: Vec<Key>,
    pub frames: Vec<Frame>,
    pub dropped_keys: u64,
    pub dropped_frames: u64,
    /// The product's frame presenter, `[presented, deferred, surfaces created, failed]`.
    pub presenter: Option<[u64; 4]>,
}

/// Reads the product's file (`stickymd-input-latency 4`, or 3 from builds before plan B).
/// Anything else is an error, not a guess: each version has exactly one `dropped` line, version 4
/// exactly one `presenter` line and version 3 none.
pub(super) fn parse(text: &str) -> Result<Recording, String> {
    let mut lines = text.lines();
    let with_presenter = match lines.next() {
        Some("stickymd-input-latency 4") => true,
        // The same without the presenter's counters (builds before plan B).
        Some("stickymd-input-latency 3") => false,
        _ => return Err("not a version 3 or 4 input-latency recording".into()),
    };
    let mut recording = Recording::default();
    let mut dropped_lines = 0;
    for line in lines {
        let fields = line.split(' ').collect::<Vec<_>>();
        let number = |index: usize| -> Result<u64, String> {
            fields
                .get(index)
                .and_then(|field| field.parse().ok())
                .ok_or_else(|| format!("bad recording line: {line}"))
        };
        match (fields.first().copied(), fields.len()) {
            (Some("presenter"), 5) if with_presenter && recording.presenter.is_none() => {
                recording.presenter = Some([number(1)?, number(2)?, number(3)?, number(4)?]);
            }
            (Some("dropped"), 3) if dropped_lines == 0 => {
                dropped_lines += 1;
                recording.dropped_keys = number(1)?;
                recording.dropped_frames = number(2)?;
            }
            (Some("key"), 3) => recording.keys.push(Key {
                event: number(1)?,
                seen: number(2)?,
            }),
            (Some("frame"), 3) => recording.frames.push(Frame {
                presented: number(1)?,
                drawn: number(2)?,
            }),
            _ => return Err(format!("bad recording line: {line}")),
        }
    }
    if dropped_lines == 0 || with_presenter != recording.presenter.is_some() {
        return Err("the recording lacks its dropped or presenter line".into());
    }
    Ok(recording)
}

/// Per-key stage durations in nanoseconds. What follows `present` (commit, compositor, display)
/// is not measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Sample {
    /// Event timestamp to the main thread seeing it.
    pub queue: u64,
    /// Main thread seeing it to the start of drawing its frame: handling and waiting for the
    /// redraw.
    pub wait: u64,
    /// Drawing the frame through `present` returning: painting, converting, presenting.
    pub draw: u64,
    /// Event to `present` returned: `queue + wait + draw`.
    pub total: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Paired {
    pub samples: Vec<Sample>,
    /// Keys in the window with no frame drawn after them.
    pub unpaired: usize,
    /// Keys whose timestamps are out of order (seen before the event).
    pub disordered: usize,
}

/// Pairs each key whose event lies in `[from, to]` with the first frame whose drawing started at
/// or after the main thread saw the key. A frame already being drawn cannot hold a key seen
/// during it: winit defers an event that arrives while it is drawing.
///
/// Pre: frames are in recording order, which is presentation order (the main thread appends
/// them), so their draw starts are ordered too. Post: every key in the window is counted exactly
/// once, as a sample, as unpaired or as disordered, and each sample's stages add up to its total.
/// O((k + f) log f).
pub(super) fn pair(recording: &Recording, from: u64, to: u64) -> Paired {
    let frames = &recording.frames;
    debug_assert!(
        frames
            .windows(2)
            .all(|w| w[0].draw_start() <= w[1].draw_start())
    );
    let mut paired = Paired {
        samples: Vec::new(),
        unpaired: 0,
        disordered: 0,
    };
    for key in recording
        .keys
        .iter()
        .filter(|key| (from..=to).contains(&key.event))
    {
        if key.seen < key.event {
            paired.disordered += 1;
            continue;
        }
        let index = frames.partition_point(|frame| frame.draw_start() < key.seen);
        let Some(frame) = frames.get(index) else {
            paired.unpaired += 1;
            continue;
        };
        let (queue, wait) = (key.seen - key.event, frame.draw_start() - key.seen);
        paired.samples.push(Sample {
            queue,
            wait,
            draw: frame.drawn,
            total: queue + wait + frame.drawn,
        });
    }
    paired
}

/// Nearest-rank percentiles of nanosecond samples, reported in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Stats {
    pub count: usize,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
}

pub(super) fn stats(mut values: Vec<u64>) -> Option<Stats> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let rank = |percent: usize| {
        let index = (values.len() * percent).div_ceil(100).max(1) - 1;
        values[index] as f64 / 1e6
    };
    Some(Stats {
        count: values.len(),
        p50: rank(50),
        p95: rank(95),
        p99: rank(99),
        max: *values.last().expect("not empty") as f64 / 1e6,
    })
}

pub(super) fn stats_json(stats: Option<Stats>) -> String {
    stats.map_or("null".into(), |s| {
        format!(
            "{{\"count\":{},\"p50_ms\":{:.3},\"p95_ms\":{:.3},\"p99_ms\":{:.3},\"max_ms\":{:.3}}}",
            s.count, s.p50, s.p95, s.p99, s.max
        )
    })
}

/// The stages of a set of samples, as JSON.
pub(super) fn stages_json(samples: &[Sample]) -> String {
    format!(
        "{{\"queue\":{},\"wait\":{},\"draw\":{},\"total\":{}}}",
        stats_json(stats(samples.iter().map(|s| s.queue).collect())),
        stats_json(stats(samples.iter().map(|s| s.wait).collect())),
        stats_json(stats(samples.iter().map(|s| s.draw).collect())),
        stats_json(stats(samples.iter().map(|s| s.total).collect())),
    )
}

/// The median of `event - injected` per key, in nanoseconds, signed: the product's event clock
/// against the driver's clock at injection. Pairs by order, so it needs equal counts.
pub(super) fn clock_offset(events: &[u64], injected: &[u64]) -> Option<i64> {
    if events.is_empty() || events.len() != injected.len() {
        return None;
    }
    let mut offsets = events
        .iter()
        .zip(injected)
        .map(|(event, injected)| *event as i64 - *injected as i64)
        .collect::<Vec<_>>();
    offsets.sort_unstable();
    Some(offsets[offsets.len() / 2])
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;

    fn frame(presented: u64, drawn: u64) -> Frame {
        Frame { presented, drawn }
    }

    #[test]
    fn the_recording_parses_and_rejects_anything_else() {
        let text = "stickymd-input-latency 4\ndropped 0 2\npresenter 5 1 3 0\nkey 10 12\nframe 20 3\nframe 30 4\n";
        let recording = parse(text).unwrap();
        assert_eq!(
            recording.keys,
            [Key {
                event: 10,
                seen: 12
            }]
        );
        assert_eq!(recording.frames, [frame(20, 3), frame(30, 4)]);
        assert_eq!((recording.dropped_keys, recording.dropped_frames), (0, 2));
        assert!(parse("stickymd-input-latency 2\n").is_err());
        assert_eq!(
            parse("stickymd-input-latency 3\ndropped 0 0\nkey 1 2\n")
                .unwrap()
                .presenter,
            None
        );
        // Each count exactly once, and the presenter's only in version 4.
        for text in [
            "stickymd-input-latency 4\ndropped 0 0\n",
            "stickymd-input-latency 4\npresenter 1 0 1 0\n",
            "stickymd-input-latency 4\ndropped 0 0\npresenter 1 0 1 0\npresenter 1 0 1 9\n",
            "stickymd-input-latency 4\ndropped 0 0\ndropped 0 1\npresenter 1 0 1 0\n",
            "stickymd-input-latency 3\ndropped 0 0\npresenter 1 0 1 0\n",
            "stickymd-input-latency 3\nkey 1 2\n",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
        assert_eq!(recording.presenter, Some([5, 1, 3, 0]));
        assert!(parse("stickymd-input-latency 4\nframe 1 2 3\n").is_err());
        assert!(parse("stickymd-input-latency 4\nnote x y\n").is_err());
        assert!(parse("stickymd-input-latency 4\npresenter 1 2\n").is_err());
    }

    #[test]
    fn each_key_meets_the_first_frame_drawn_after_it() {
        let key = |event, seen| Key { event, seen };
        let recording = Recording {
            keys: vec![
                key(MS, 2 * MS),       // before the window
                key(10 * MS, 11 * MS), // frame drawn 12..14 ms
                key(12 * MS, 13 * MS), // seen while that frame is drawn: the next frame
                key(20 * MS, 19 * MS), // seen before its event: disordered
                key(30 * MS, 31 * MS), // no frame after it
            ],
            frames: vec![
                frame(5 * MS, MS),
                frame(14 * MS, 2 * MS),
                frame(21 * MS, MS),
            ],
            ..Recording::default()
        };
        let paired = pair(&recording, 5 * MS, 40 * MS);
        assert_eq!((paired.unpaired, paired.disordered), (1, 1));
        let sample = |queue, wait, draw| Sample {
            queue,
            wait,
            draw,
            total: queue + wait + draw,
        };
        assert_eq!(
            paired.samples,
            [sample(MS, MS, 2 * MS), sample(MS, 7 * MS, MS)]
        );
    }

    #[test]
    fn percentiles_are_nearest_rank() {
        let s = stats((1..=100).map(|n| n * MS).collect()).unwrap();
        assert_eq!(
            (s.count, s.p50, s.p95, s.p99, s.max),
            (100, 50.0, 95.0, 99.0, 100.0)
        );
        let one = stats(vec![3 * MS]).unwrap();
        assert_eq!((one.p50, one.p95, one.max), (3.0, 3.0, 3.0));
        assert!(stats(Vec::new()).is_none());
        assert_eq!(stats_json(None), "null");
    }

    #[test]
    fn the_clock_offset_is_the_signed_median() {
        assert_eq!(clock_offset(&[105, 210, 290], &[100, 200, 300]), Some(5));
        assert_eq!(clock_offset(&[1], &[1, 2]), None);
    }
}
