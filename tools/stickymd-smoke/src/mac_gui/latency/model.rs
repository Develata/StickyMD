//! The recording's format, the key-to-frame pairing and the statistics, with no I/O.
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

/// One key-down as the product saw it: the event's own timestamp and when the main thread got it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Key {
    pub event: u64,
    pub seen: u64,
}

/// One presented frame: when `present` returned, when its transaction committed (0 = never seen
/// to commit) and how long drawing it took.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Frame {
    pub presented: u64,
    pub committed: u64,
    pub drawn: u64,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Recording {
    pub keys: Vec<Key>,
    pub frames: Vec<Frame>,
    pub dropped_keys: u64,
    pub dropped_frames: u64,
}

/// Reads the product's file (`stickymd-input-latency 2`). Anything else is an error, not a guess.
pub(super) fn parse(text: &str) -> Result<Recording, String> {
    let mut lines = text.lines();
    if lines.next() != Some("stickymd-input-latency 2") {
        return Err("not a version 2 input-latency recording".into());
    }
    let mut recording = Recording::default();
    for line in lines {
        let fields = line.split(' ').collect::<Vec<_>>();
        let number = |index: usize| -> Result<u64, String> {
            fields
                .get(index)
                .and_then(|field| field.parse().ok())
                .ok_or_else(|| format!("bad recording line: {line}"))
        };
        match (fields.first().copied(), fields.len()) {
            (Some("dropped"), 3) => {
                recording.dropped_keys = number(1)?;
                recording.dropped_frames = number(2)?;
            }
            (Some("key"), 3) => recording.keys.push(Key {
                event: number(1)?,
                seen: number(2)?,
            }),
            (Some("frame"), 4) => recording.frames.push(Frame {
                presented: number(1)?,
                committed: number(2)?,
                drawn: number(3)?,
            }),
            _ => return Err(format!("bad recording line: {line}")),
        }
    }
    Ok(recording)
}

/// Per-key stage durations in nanoseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Sample {
    /// Event timestamp to the main thread seeing it.
    pub queue: u64,
    /// Main thread seeing it to the start of drawing its frame: handling and waiting for the
    /// redraw.
    pub wait: u64,
    /// Drawing the frame through `present` returning: painting, converting, presenting.
    pub draw: u64,
    /// `present` returned to the transaction's commit; `None` when the commit was not seen.
    pub commit: Option<u64>,
    /// Event to commit (or to `present` when the commit was not seen).
    pub total: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Paired {
    pub samples: Vec<Sample>,
    /// Keys in the window with no frame presented after them.
    pub unpaired: usize,
    /// Keys whose frame never reported a commit.
    pub uncommitted: usize,
}

/// Pairs each key whose event lies in `[from, to]` with the first frame presented at or after the
/// main thread saw it.
///
/// Pre: frames are in recording order, which is presentation order (the main thread appends
/// them). Post: one sample per paired key; every key in the window is counted exactly once, as
/// a sample or as unpaired. A frame whose drawing started before the key was seen still carries
/// it (the key arrived mid-draw and waits zero). O((k + f) log f).
pub(super) fn pair(recording: &Recording, from: u64, to: u64) -> Paired {
    let frames = &recording.frames;
    debug_assert!(frames.windows(2).all(|w| w[0].presented <= w[1].presented));
    let mut paired = Paired {
        samples: Vec::new(),
        unpaired: 0,
        uncommitted: 0,
    };
    for key in recording
        .keys
        .iter()
        .filter(|key| (from..=to).contains(&key.event))
    {
        let index = frames.partition_point(|frame| frame.presented < key.seen);
        let Some(frame) = frames.get(index) else {
            paired.unpaired += 1;
            continue;
        };
        let commit =
            (frame.committed != 0).then(|| frame.committed.saturating_sub(frame.presented));
        if commit.is_none() {
            paired.uncommitted += 1;
        }
        let end = if frame.committed != 0 {
            frame.committed
        } else {
            frame.presented
        };
        let draw_start = frame.presented.saturating_sub(frame.drawn);
        paired.samples.push(Sample {
            queue: key.seen.saturating_sub(key.event),
            wait: draw_start.saturating_sub(key.seen),
            draw: frame.drawn,
            commit,
            total: end.saturating_sub(key.event),
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
        "{{\"queue\":{},\"wait\":{},\"draw\":{},\"commit\":{},\"total\":{}}}",
        stats_json(stats(samples.iter().map(|s| s.queue).collect())),
        stats_json(stats(samples.iter().map(|s| s.wait).collect())),
        stats_json(stats(samples.iter().map(|s| s.draw).collect())),
        stats_json(stats(samples.iter().filter_map(|s| s.commit).collect())),
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

    fn frame(presented: u64, committed: u64, drawn: u64) -> Frame {
        Frame {
            presented,
            committed,
            drawn,
        }
    }

    #[test]
    fn the_recording_parses_and_rejects_anything_else() {
        let text =
            "stickymd-input-latency 2\ndropped 0 2\nkey 10 12\nframe 20 25 3\nframe 30 0 4\n";
        let recording = parse(text).unwrap();
        assert_eq!(
            recording.keys,
            [Key {
                event: 10,
                seen: 12
            }]
        );
        assert_eq!(recording.frames, [frame(20, 25, 3), frame(30, 0, 4)]);
        assert_eq!((recording.dropped_keys, recording.dropped_frames), (0, 2));
        assert!(parse("stickymd-input-latency 1\n").is_err());
        assert!(parse("stickymd-input-latency 2\nframe 1 2\n").is_err());
        assert!(parse("stickymd-input-latency 2\nnote x y\n").is_err());
    }

    #[test]
    fn each_key_in_the_window_meets_the_first_frame_after_it() {
        let recording = Recording {
            keys: vec![
                // Before the window.
                Key {
                    event: MS,
                    seen: 2 * MS,
                },
                // Frame drawn 12..14 ms.
                Key {
                    event: 10 * MS,
                    seen: 11 * MS,
                },
                // Seen mid-draw of that frame: it waits zero.
                Key {
                    event: 12 * MS,
                    seen: 13 * MS,
                },
                // Frame presented at 21, never committed.
                Key {
                    event: 20 * MS,
                    seen: 21 * MS,
                },
                // No frame after it.
                Key {
                    event: 30 * MS,
                    seen: 31 * MS,
                },
            ],
            frames: vec![
                frame(5 * MS, 6 * MS, MS),
                frame(14 * MS, 16 * MS, 2 * MS),
                frame(21 * MS, 0, MS),
            ],
            ..Recording::default()
        };
        let paired = pair(&recording, 5 * MS, 40 * MS);
        assert_eq!(paired.unpaired, 1);
        assert_eq!(paired.uncommitted, 1);
        let sample = |queue, wait, draw, commit, total| Sample {
            queue,
            wait,
            draw,
            commit,
            total,
        };
        assert_eq!(
            paired.samples,
            [
                sample(MS, MS, 2 * MS, Some(2 * MS), 6 * MS),
                sample(MS, 0, 2 * MS, Some(2 * MS), 4 * MS),
                sample(MS, 0, MS, None, MS),
            ]
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
