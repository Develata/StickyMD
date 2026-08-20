//! Bounded one-in-flight plus one-latest preview worker.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-scheduling

use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

use stickymd_core::{DocumentSnapshot, Generation};
use stickymd_render::preview::{
    PreviewFrame, PreviewPipeline, PreviewPipelineError, PreviewSelection, PreviewTheme,
};

#[derive(Debug)]
pub enum PreviewJob {
    Build {
        snapshot: DocumentSnapshot,
        viewport: PreviewViewport,
    },
    Relayout {
        generation: Generation,
        viewport: PreviewViewport,
    },
    Paint {
        generation: Generation,
        height_px: u32,
        scroll_y: f32,
        selection: PreviewSelection,
        theme: PreviewTheme,
    },
}

impl PreviewJob {
    pub const fn generation(&self) -> Generation {
        match self {
            Self::Build { snapshot, .. } => snapshot.generation,
            Self::Relayout { generation, .. } | Self::Paint { generation, .. } => *generation,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PreviewViewport {
    pub width_px: u32,
    pub height_px: u32,
    pub scale: f32,
    pub scroll_y: f32,
    pub selection: PreviewSelection,
    pub theme: PreviewTheme,
}

#[derive(Debug)]
pub struct PreviewCompletion {
    pub generation: Generation,
    pub result: Result<PreviewFrame, PreviewPipelineError>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PreviewWorkerMetrics {
    pub submitted: u64,
    pub started: u64,
    pub completed: u64,
    pub coalesced: u64,
    pub raster_releases: u64,
}

#[derive(Default)]
struct Mailbox {
    pending: Option<PreviewJob>,
    release_math_rasters: bool,
    shutdown: bool,
    metrics: PreviewWorkerMetrics,
}

impl Mailbox {
    fn push(&mut self, job: PreviewJob) {
        self.metrics.submitted = self.metrics.submitted.saturating_add(1);
        let next = match self.pending.take() {
            None => job,
            Some(pending) => {
                self.metrics.coalesced = self.metrics.coalesced.saturating_add(1);
                coalesce(pending, job)
            }
        };
        self.pending = Some(next);
    }
}

pub struct PreviewWorker {
    shared: Arc<(Mutex<Mailbox>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}

impl PreviewWorker {
    pub fn start<F>(on_completion: F) -> Result<Self, std::io::Error>
    where
        F: Fn(PreviewCompletion) + Send + Sync + 'static,
    {
        let shared = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let worker_shared = Arc::clone(&shared);
        let on_completion = Arc::new(on_completion);
        let thread = thread::Builder::new()
            .name("stickymd-preview".into())
            .stack_size(512 * 1024)
            .spawn(move || run_worker(worker_shared, on_completion))?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    pub fn submit(&self, job: PreviewJob) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.push(job);
            ready.notify_one();
        }
    }

    pub fn release_math_rasters(&self) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.pending = None;
            mailbox.release_math_rasters = true;
            ready.notify_one();
        }
    }

    #[cfg(test)]
    fn metrics(&self) -> Option<PreviewWorkerMetrics> {
        self.shared.0.lock().ok().map(|mailbox| mailbox.metrics)
    }
}

impl Drop for PreviewWorker {
    fn drop(&mut self) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.shutdown = true;
            mailbox.pending = None;
            ready.notify_one();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_worker<F>(shared: Arc<(Mutex<Mailbox>, Condvar)>, on_completion: Arc<F>)
where
    F: Fn(PreviewCompletion),
{
    let mut pipeline = PreviewPipeline::new();
    loop {
        let job = {
            let (lock, ready) = &*shared;
            let mut mailbox = match lock.lock() {
                Ok(mailbox) => mailbox,
                Err(_) => return,
            };
            while mailbox.pending.is_none() && !mailbox.release_math_rasters && !mailbox.shutdown {
                mailbox = match ready.wait(mailbox) {
                    Ok(mailbox) => mailbox,
                    Err(_) => return,
                };
            }
            if mailbox.shutdown {
                return;
            }
            if mailbox.release_math_rasters {
                mailbox.release_math_rasters = false;
                mailbox.metrics.raster_releases = mailbox.metrics.raster_releases.saturating_add(1);
                None
            } else {
                mailbox.metrics.started = mailbox.metrics.started.saturating_add(1);
                mailbox.pending.take()
            }
        };
        let Some(job) = job else {
            pipeline.release_math_rasters();
            continue;
        };
        let generation = job.generation();
        let result = execute(&mut pipeline, job);
        if let Ok(mut mailbox) = shared.0.lock() {
            mailbox.metrics.completed = mailbox.metrics.completed.saturating_add(1);
        }
        on_completion(PreviewCompletion { generation, result });
    }
}

fn execute(
    pipeline: &mut PreviewPipeline,
    job: PreviewJob,
) -> Result<PreviewFrame, stickymd_render::preview::PreviewPipelineError> {
    match job {
        PreviewJob::Build { snapshot, viewport } => pipeline.build(
            &snapshot,
            viewport.width_px,
            viewport.height_px,
            viewport.scale,
            viewport.scroll_y,
            viewport.selection,
            viewport.theme,
        ),
        PreviewJob::Relayout {
            generation,
            viewport,
        } => pipeline.relayout(
            generation,
            viewport.width_px,
            viewport.height_px,
            viewport.scale,
            viewport.scroll_y,
            viewport.selection,
            viewport.theme,
        ),
        PreviewJob::Paint {
            generation,
            height_px,
            scroll_y,
            selection,
            theme,
        } => pipeline.paint(generation, height_px, scroll_y, selection, theme),
    }
}

fn coalesce(pending: PreviewJob, incoming: PreviewJob) -> PreviewJob {
    match (pending, incoming) {
        (_, incoming @ PreviewJob::Build { .. }) => incoming,
        (
            PreviewJob::Build {
                snapshot,
                viewport: _old,
            },
            PreviewJob::Relayout {
                generation,
                viewport,
            },
        ) if snapshot.generation == generation => PreviewJob::Build { snapshot, viewport },
        (
            PreviewJob::Build {
                snapshot,
                mut viewport,
            },
            PreviewJob::Paint {
                generation,
                height_px,
                scroll_y,
                selection,
                theme,
            },
        ) if snapshot.generation == generation => {
            viewport.height_px = height_px;
            viewport.scroll_y = scroll_y;
            viewport.selection = selection;
            viewport.theme = theme;
            PreviewJob::Build { snapshot, viewport }
        }
        (pending @ PreviewJob::Build { .. }, _) => pending,
        (_, incoming @ PreviewJob::Relayout { .. }) => incoming,
        (
            PreviewJob::Relayout {
                generation,
                mut viewport,
            },
            PreviewJob::Paint {
                generation: paint_generation,
                height_px,
                scroll_y,
                selection,
                theme,
            },
        ) if generation == paint_generation => {
            viewport.height_px = height_px;
            viewport.scroll_y = scroll_y;
            viewport.selection = selection;
            viewport.theme = theme;
            PreviewJob::Relayout {
                generation,
                viewport,
            }
        }
        (pending @ PreviewJob::Relayout { .. }, PreviewJob::Paint { .. }) => pending,
        (_, incoming @ PreviewJob::Paint { .. }) => incoming,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use stickymd_core::{
        CursorSnapshot, DocumentSnapshot, DocumentState, EditKind, EditMeta, EditRequest,
        Generation, LineEnding,
    };

    use super::*;

    fn viewport(width: u32) -> PreviewViewport {
        PreviewViewport {
            width_px: width,
            height_px: 300,
            scale: 1.0,
            scroll_y: 0.0,
            selection: PreviewSelection::default(),
            theme: PreviewTheme::Light,
        }
    }

    fn build(generation: Generation, text: &'static str) -> PreviewJob {
        PreviewJob::Build {
            snapshot: DocumentSnapshot {
                text: Arc::from(text),
                generation,
                line_ending: LineEnding::Lf,
            },
            viewport: viewport(500),
        }
    }

    #[test]
    fn mailbox_is_bounded_to_one_latest_pending_generation() {
        let mut mailbox = Mailbox::default();
        let mut generation = Generation::initial();
        for index in 0..1000 {
            generation = generation.checked_next().unwrap();
            mailbox.push(build(
                generation,
                if index == 999 { "latest" } else { "old" },
            ));
        }
        assert_eq!(mailbox.metrics.submitted, 1000);
        assert_eq!(mailbox.metrics.coalesced, 999);
        assert_eq!(mailbox.pending.as_ref().unwrap().generation(), generation);
    }

    #[test]
    fn newer_build_supersedes_relayout_and_paint() {
        let current = Generation::initial();
        let newer = current.checked_next().unwrap();
        let pending = PreviewJob::Relayout {
            generation: current,
            viewport: viewport(400),
        };
        assert_eq!(coalesce(pending, build(newer, "new")).generation(), newer);
    }

    #[test]
    fn worker_completion_keeps_generation_and_typed_failure() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = PreviewWorker::start(move |completion| {
            let _ = sender.send(completion);
        })
        .unwrap();
        let generation = Generation::initial();
        worker.submit(PreviewJob::Relayout {
            generation,
            viewport: viewport(500),
        });
        let completion = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(completion.generation, generation);
        assert!(matches!(
            completion.result,
            Err(PreviewPipelineError::NoDocument)
        ));
        let metrics = worker.metrics().unwrap();
        assert_eq!(metrics.submitted, 1);
        assert_eq!(metrics.started, 1);
        assert_eq!(metrics.completed, 1);
    }

    #[test]
    fn raster_release_precedes_the_next_pending_relayout() {
        let (sender, receiver) = mpsc::sync_channel(2);
        let worker = PreviewWorker::start(move |completion| {
            let _ = sender.send(completion);
        })
        .unwrap();
        let generation = Generation::initial();
        worker.submit(build(generation, "$x^2$"));
        let first = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(first.result.is_ok());

        worker.release_math_rasters();
        worker.submit(PreviewJob::Relayout {
            generation,
            viewport: viewport(600),
        });
        let after_release = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(after_release.result.is_ok());
        let metrics = worker.metrics().unwrap();
        assert_eq!(metrics.raster_releases, 1);
    }

    #[test]
    #[ignore = "Release-only Phase 6 source latency while math worker is busy"]
    fn phase6_source_edit_during_math_build_release_baseline() {
        let mut source = String::with_capacity(1024 * 1024 + 256);
        for index in 0..500 {
            source.push_str(&format!(
                "Formula {index}: $\\frac{{x_{index}^2}}{{1+y_{index}}}$\n\n"
            ));
        }
        while source.len() < 1024 * 1024 {
            source.push_str("中文 source latency while the math worker owns preview work.\n\n");
        }
        let mut document = DocumentState::loaded(&source, LineEnding::Lf, None);
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = PreviewWorker::start(move |completion| {
            let _ = sender.try_send(completion);
        })
        .unwrap();
        worker.submit(PreviewJob::Build {
            snapshot: document.snapshot(),
            viewport: viewport(900),
        });

        let start_deadline = Instant::now() + Duration::from_secs(2);
        while worker.metrics().is_none_or(|metrics| metrics.started == 0) {
            assert!(Instant::now() < start_deadline, "math worker did not start");
            std::thread::yield_now();
        }

        let mut samples = Vec::with_capacity(100);
        for timestamp_ms in 0..100 {
            let position = document.len_bytes();
            let started = Instant::now();
            document
                .edit(EditRequest::new(
                    document.generation(),
                    position..position,
                    "x",
                    CursorSnapshot::caret(position),
                    CursorSnapshot::caret(position + 1),
                    EditMeta::new(EditKind::Typing, timestamp_ms * 1_000),
                ))
                .unwrap();
            samples.push(started.elapsed());
        }
        samples.sort_unstable();
        let p95 = samples[94];
        let max = *samples.last().unwrap();
        println!("phase6 source_during_math_build edits=100 p95={p95:?} max={max:?}");
        assert!(p95 < Duration::from_millis(50));
        let completion = receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("representative math build completes in the background");
        assert!(completion.result.is_ok());
    }
}
