//! The software frame path: what a damaged frame needs, its conversion into the native buffer, the
//! bytes it cost and the softbuffer surface that presents it.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#frame-presentation

mod convert;
mod ledger;
mod plan;
mod surface;

use thiserror::Error;

pub use ledger::FrameLedger;
pub use plan::{DamageRect, PresentPlan, plan_present};
pub use surface::SoftwareSurface;

#[derive(Debug, Error)]
pub enum SurfaceError {
    #[error("framebuffer context failed: {0}")]
    Context(String),
    #[error("framebuffer surface failed: {0}")]
    Surface(String),
    #[error("invalid zero-sized framebuffer")]
    ZeroSize,
    #[error("pixmap allocation failed for {width}x{height}")]
    PixmapAllocation { width: u32, height: u32 },
    #[error("framebuffer length did not match pixmap dimensions")]
    LengthMismatch,
    #[error("frame presentation failed: {0}")]
    Present(String),
}
