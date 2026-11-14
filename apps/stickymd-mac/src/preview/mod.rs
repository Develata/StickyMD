//! macOS side of Preview: the local image source injected into the shared worker.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-scheduling

#[cfg(test)]
mod image_refresh_tests;
mod image_source;

pub use image_source::LocalImageSource;
