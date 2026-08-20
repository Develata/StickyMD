//! Platform-independent rendering projections for StickyMD.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#owned-ast-projection
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor
//!
//! `SourceProjection` may duplicate text inside a `cosmic_text::Buffer`, but the
//! duplicate is generation-tagged, replaceable, never persisted, and can never
//! mutate the canonical `DocumentState`.
#![forbid(unsafe_code)]

pub mod source;
