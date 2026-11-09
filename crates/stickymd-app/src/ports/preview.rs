//! Preview build requests and their receipts.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-scheduling

use stickymd_core::{DocumentSnapshot, Generation};
use stickymd_render::image::PreviewImageSource;
use stickymd_render::preview::{
    PreviewFrame, PreviewHorizontalOffsets, PreviewPipelineError, PreviewSelection, PreviewTheme,
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
}

impl PreviewJob {
    pub const fn generation(&self) -> Generation {
        match self {
            Self::Build { snapshot, .. } => snapshot.generation,
            Self::Relayout { generation, .. } => *generation,
        }
    }

    pub fn viewport(&self) -> PreviewViewport {
        match self {
            Self::Build { viewport, .. } | Self::Relayout { viewport, .. } => viewport.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewViewport {
    pub horizontal: PreviewHorizontalOffsets,
    pub width_px: u32,
    pub height_px: u32,
    pub scale: f32,
    pub scroll_y: f32,
    pub selection: PreviewSelection,
    pub theme: PreviewTheme,
}

impl PreviewViewport {
    /// Scroll and selection may change while the same layout is still usable
    /// for document bounds and semantic scroll anchors.
    pub fn same_layout(&self, other: &Self) -> bool {
        self.width_px == other.width_px
            && self.height_px == other.height_px
            && self.scale == other.scale
            && self.theme == other.theme
    }
}

/// What reads the images a note names, for the Preview worker the shell starts.
pub trait PreviewImages {
    /// Without a source, images stay placeholders.
    fn preview_images(&self) -> Option<Box<dyn PreviewImageSource + Send>>;
}

/// One finished job, with the generation and viewport it was built for: its identity.
#[derive(Debug)]
pub struct PreviewCompletion {
    pub generation: Generation,
    pub viewport: PreviewViewport,
    pub result: Result<PreviewFrame, PreviewPipelineError>,
}
