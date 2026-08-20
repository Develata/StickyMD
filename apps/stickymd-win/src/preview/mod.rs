//! Single-worker native preview execution boundary.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-scheduling

mod worker;

pub use worker::{PreviewCompletion, PreviewJob, PreviewViewport, PreviewWorker};
