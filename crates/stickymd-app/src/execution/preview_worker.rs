//! Bounded one-in-flight plus one-latest preview worker.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-scheduling

use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

use stickymd_render::image::PreviewImageSource;
use stickymd_render::preview::{PreviewFrame, PreviewPipeline, PreviewPipelineError};

use crate::ports::{PreviewCompletion, PreviewJob};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PreviewWorkerMetrics {
    pub submitted: u64,
    pub started: u64,
    pub completed: u64,
    pub coalesced: u64,
    pub raster_releases: u64,
    pub projection_releases: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ProjectionRelease {
    Rasters,
    Document,
}

#[derive(Default)]
struct Mailbox {
    pending: Option<PreviewJob>,
    release: Option<ProjectionRelease>,
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
    /// `image_source` reads the images a document names; without one, images stay placeholders.
    pub fn start<F>(
        image_source: Option<Box<dyn PreviewImageSource + Send>>,
        on_completion: F,
    ) -> Result<Self, std::io::Error>
    where
        F: Fn(PreviewCompletion) + Send + Sync + 'static,
    {
        let shared = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let worker_shared = Arc::clone(&shared);
        let on_completion = Arc::new(on_completion);
        let thread = thread::Builder::new()
            .name("stickymd-preview".into())
            .stack_size(512 * 1024)
            .spawn(move || run_worker(worker_shared, on_completion, image_source))?;
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

    pub fn release_raster_caches(&self) {
        self.request_release(ProjectionRelease::Rasters);
    }

    pub fn release_document_projection(&self) {
        self.request_release(ProjectionRelease::Document);
    }

    fn request_release(&self, requested: ProjectionRelease) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.pending = None;
            mailbox.release = Some(
                mailbox
                    .release
                    .map_or(requested, |queued| queued.max(requested)),
            );
            ready.notify_one();
        }
    }

    pub fn metrics(&self) -> Option<PreviewWorkerMetrics> {
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

enum WorkerAction {
    Release(ProjectionRelease),
    Job(PreviewJob),
}

fn run_worker<F>(
    shared: Arc<(Mutex<Mailbox>, Condvar)>,
    on_completion: Arc<F>,
    image_source: Option<Box<dyn PreviewImageSource + Send>>,
) where
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
            while mailbox.pending.is_none() && mailbox.release.is_none() && !mailbox.shutdown {
                mailbox = match ready.wait(mailbox) {
                    Ok(mailbox) => mailbox,
                    Err(_) => return,
                };
            }
            if mailbox.shutdown {
                return;
            }
            if let Some(release) = mailbox.release.take() {
                match release {
                    ProjectionRelease::Rasters => {
                        mailbox.metrics.raster_releases =
                            mailbox.metrics.raster_releases.saturating_add(1);
                    }
                    ProjectionRelease::Document => {
                        mailbox.metrics.projection_releases =
                            mailbox.metrics.projection_releases.saturating_add(1);
                    }
                }
                WorkerAction::Release(release)
            } else {
                mailbox.metrics.started = mailbox.metrics.started.saturating_add(1);
                match mailbox.pending.take() {
                    Some(job) => WorkerAction::Job(job),
                    None => continue,
                }
            }
        };
        let job = match job {
            WorkerAction::Release(ProjectionRelease::Rasters) => {
                pipeline.release_raster_caches();
                continue;
            }
            WorkerAction::Release(ProjectionRelease::Document) => {
                pipeline.release_document_projection();
                continue;
            }
            WorkerAction::Job(job) => job,
        };
        let generation = job.generation();
        let viewport = job.viewport();
        let source = image_source
            .as_deref()
            .map(|source| source as &dyn PreviewImageSource);
        let result = execute(&mut pipeline, job, source);
        if let Ok(mut mailbox) = shared.0.lock() {
            mailbox.metrics.completed = mailbox.metrics.completed.saturating_add(1);
        }
        on_completion(PreviewCompletion {
            generation,
            viewport,
            result,
        });
    }
}

fn execute(
    pipeline: &mut PreviewPipeline,
    job: PreviewJob,
    image_source: Option<&dyn PreviewImageSource>,
) -> Result<PreviewFrame, PreviewPipelineError> {
    pipeline.set_horizontal_offsets(job.viewport().horizontal);
    match job {
        PreviewJob::Build { snapshot, viewport } => pipeline.build_with_image_source(
            &snapshot,
            viewport.width_px,
            viewport.height_px,
            viewport.scale,
            viewport.scroll_y,
            viewport.selection,
            viewport.theme,
            image_source,
        ),
        // The pipeline reuses matching geometry and only paints when width,
        // scale and theme are unchanged. Every request still carries the full
        // viewport, so a scroll cannot restore a pre-resize/zoom layout.
        PreviewJob::Relayout {
            generation,
            viewport,
        } => pipeline.relayout_with_image_source(
            generation,
            viewport.width_px,
            viewport.height_px,
            viewport.scale,
            viewport.scroll_y,
            viewport.selection,
            viewport.theme,
            image_source,
        ),
    }
}

fn coalesce(pending: PreviewJob, incoming: PreviewJob) -> PreviewJob {
    match incoming.generation().cmp(&pending.generation()) {
        std::cmp::Ordering::Less => return pending,
        std::cmp::Ordering::Greater => return incoming,
        std::cmp::Ordering::Equal => {}
    }
    match (pending, incoming) {
        (_, incoming @ PreviewJob::Build { .. }) => incoming,
        (PreviewJob::Build { snapshot, .. }, PreviewJob::Relayout { viewport, .. }) => {
            PreviewJob::Build { snapshot, viewport }
        }
        (_, incoming @ PreviewJob::Relayout { .. }) => incoming,
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
    use crate::ports::PreviewViewport;
    use stickymd_render::preview::{PreviewSelection, PreviewTheme};

    fn viewport(width: u32) -> PreviewViewport {
        PreviewViewport {
            horizontal: Default::default(),
            width_px: width,
            height_px: 300,
            scale: 1.0,
            scroll_y: 0.0,
            selection: PreviewSelection::default(),
            theme: PreviewTheme::Light,
        }
    }

    /// A hang guard, not a latency bound: before its first job each worker builds its own
    /// font system from the installed fonts, which can take seconds in a debug build on a
    /// loaded machine.
    const COMPLETION_DEADLINE: Duration = Duration::from_secs(300);

    fn next_completion(receiver: &mpsc::Receiver<PreviewCompletion>) -> PreviewCompletion {
        receiver
            .recv_timeout(COMPLETION_DEADLINE)
            .unwrap_or_else(|error| {
                panic!("no preview completion within {COMPLETION_DEADLINE:?}: {error}")
            })
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
    fn newer_build_supersedes_viewport_updates() {
        let current = Generation::initial();
        let newer = current.checked_next().unwrap();
        let pending = PreviewJob::Relayout {
            generation: current,
            viewport: viewport(400),
        };
        assert_eq!(coalesce(pending, build(newer, "new")).generation(), newer);
    }

    #[test]
    fn older_jobs_never_replace_a_newer_pending_generation() {
        let older = Generation::initial();
        let newer = older.checked_next().unwrap();
        let pending = PreviewJob::Relayout {
            generation: newer,
            viewport: viewport(500),
        };
        let incoming = PreviewJob::Relayout {
            generation: older,
            viewport: viewport(400),
        };
        assert_eq!(coalesce(pending, incoming).generation(), newer);

        let pending = build(newer, "newer snapshot");
        let incoming = PreviewJob::Relayout {
            generation: older,
            viewport: viewport(500),
        };
        assert_eq!(coalesce(pending, incoming).generation(), newer);
    }

    #[test]
    fn worker_completion_keeps_generation_and_typed_failure() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = PreviewWorker::start(None, move |completion| {
            let _ = sender.send(completion);
        })
        .unwrap();
        let generation = Generation::initial();
        worker.submit(PreviewJob::Relayout {
            generation,
            viewport: viewport(500),
        });
        let completion = next_completion(&receiver);
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
    fn scrollbar_completion_preserves_requested_offset_even_after_clamping() {
        let (sender, receiver) = mpsc::channel();
        let worker = PreviewWorker::start(None, move |completion| {
            let _ = sender.send(completion);
        })
        .unwrap();
        let generation = Generation::initial();
        let mut requested = viewport(500);
        requested.scroll_y = 100_000.0;
        worker.submit(PreviewJob::Build {
            snapshot: DocumentSnapshot {
                text: Arc::from("short"),
                generation,
                line_ending: LineEnding::Lf,
            },
            viewport: requested.clone(),
        });
        let first = next_completion(&receiver);
        assert_eq!(first.viewport, requested);
        assert_eq!(first.result.unwrap().scroll_y(), 0.0);
        worker.submit(PreviewJob::Relayout {
            generation,
            viewport: viewport(500),
        });
        let second = next_completion(&receiver);
        assert_eq!(second.viewport, viewport(500));
        assert_eq!(second.result.unwrap().scroll_y(), 0.0);
    }

    #[test]
    fn scrollbar_viewport_identity_rejects_same_offset_with_stale_geometry_or_selection() {
        let current = viewport(500);
        for stale in [
            PreviewViewport {
                width_px: 400,
                ..current.clone()
            },
            PreviewViewport {
                height_px: 200,
                ..current.clone()
            },
            PreviewViewport {
                scale: 1.5,
                ..current.clone()
            },
            PreviewViewport {
                theme: PreviewTheme::Dark,
                ..current.clone()
            },
            PreviewViewport {
                selection: PreviewSelection::caret(1),
                ..current.clone()
            },
        ] {
            assert_eq!(stale.scroll_y, current.scroll_y);
            assert_ne!(
                stale, current,
                "scroll offset alone cannot admit this frame"
            );
        }
        assert!(current.same_layout(&PreviewViewport {
            scroll_y: 100.0,
            selection: PreviewSelection::caret(1),
            ..current.clone()
        }));
        assert!(!current.same_layout(&PreviewViewport {
            scale: 1.5,
            ..current.clone()
        }));
    }

    #[test]
    fn scrollbar_coalesces_resize_zoom_and_reverse_drag_without_reparsing() {
        let generation = Generation::initial();
        let text = "中文 paragraph for wrapped preview text and scrolling.\n\n".repeat(80);
        let mut pending = PreviewJob::Build {
            snapshot: DocumentSnapshot {
                text: Arc::from(text),
                generation,
                line_ending: LineEnding::Lf,
            },
            viewport: viewport(500),
        };
        let mut latest = viewport(320);
        latest.scale = 1.5;
        latest.theme = PreviewTheme::Dark;
        latest.selection = PreviewSelection {
            anchor: 0,
            active: 6,
        };
        for scroll_y in [100.0, 10_000.0, 0.0] {
            latest.scroll_y = scroll_y;
            pending = coalesce(
                pending,
                PreviewJob::Relayout {
                    generation,
                    viewport: latest.clone(),
                },
            );
        }
        assert!(matches!(pending, PreviewJob::Build { .. }));
        assert_eq!(pending.viewport(), latest);
        let mut pipeline = PreviewPipeline::new();
        let first = execute(&mut pipeline, pending, None).unwrap();
        assert_eq!(first.width(), 320);
        let before = pipeline.counters();
        assert_eq!(before.parses, 1);
        for scroll_y in [500.0, 1000.0, 0.0] {
            latest.scroll_y = scroll_y;
            let frame = execute(
                &mut pipeline,
                PreviewJob::Relayout {
                    generation,
                    viewport: latest.clone(),
                },
                None,
            )
            .unwrap();
            assert_eq!(frame.scroll_y(), scroll_y);
        }
        assert_eq!(pipeline.counters().layouts, before.layouts);
        assert_eq!(pipeline.counters().parses, before.parses);
        assert_eq!(pipeline.counters().paints, before.paints + 3);
        latest.scale = 2.0;
        execute(
            &mut pipeline,
            PreviewJob::Relayout {
                generation,
                viewport: latest.clone(),
            },
            None,
        )
        .unwrap();
        assert_eq!(pipeline.counters().layouts, before.layouts + 1);
        assert_eq!(pipeline.counters().parses, before.parses);
    }

    #[test]
    fn raster_release_keeps_semantic_tree_for_fast_preview_return() {
        let (sender, receiver) = mpsc::sync_channel(2);
        let worker = PreviewWorker::start(None, move |completion| {
            let _ = sender.send(completion);
        })
        .unwrap();
        let generation = Generation::initial();
        worker.submit(build(generation, "# heading\n\nbody"));
        assert!(next_completion(&receiver).result.is_ok());

        worker.release_raster_caches();
        worker.submit(PreviewJob::Relayout {
            generation,
            viewport: viewport(600),
        });
        assert!(next_completion(&receiver).result.is_ok());
        assert_eq!(worker.metrics().unwrap().raster_releases, 1);
    }

    #[test]
    fn document_projection_release_forces_the_next_visible_request_to_rebuild() {
        let (sender, receiver) = mpsc::sync_channel(2);
        let worker = PreviewWorker::start(None, move |completion| {
            let _ = sender.send(completion);
        })
        .unwrap();
        let generation = Generation::initial();
        worker.submit(build(generation, "# heading\n\nbody"));
        assert!(next_completion(&receiver).result.is_ok());

        worker.release_document_projection();
        worker.submit(PreviewJob::Relayout {
            generation,
            viewport: viewport(600),
        });
        let after_release = next_completion(&receiver);
        assert!(matches!(
            after_release.result,
            Err(PreviewPipelineError::NoDocument)
        ));
        assert_eq!(worker.metrics().unwrap().projection_releases, 1);
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
        let worker = PreviewWorker::start(None, move |completion| {
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
