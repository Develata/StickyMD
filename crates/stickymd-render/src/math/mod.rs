//! RaTeX adapter, bounded formula caches, and native DisplayList painting.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#ratex-native-math

mod cache;
mod engine;
mod painter;
mod path_painter;

pub(crate) use engine::{
    MAX_DOCUMENT_FORMULAS, MathEngine, MathEngineCounters, MathError, MathKind, MathRaster,
};
