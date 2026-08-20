//! Software framebuffer presentation adapter.
//!
//! plan_ref: docs/plan/09_windows_shell.md#platform-adapter-boundary

use std::num::NonZeroU32;
use std::sync::Arc;

use raw_window_handle::{HandleError, HasDisplayHandle, HasWindowHandle};
use softbuffer::{Context, Surface};
use thiserror::Error;
use tiny_skia::Pixmap;
use winit::window::Window;

struct WindowHandleSource(Arc<Window>);

impl HasWindowHandle for WindowHandleSource {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        self.0.window_handle()
    }
}

impl HasDisplayHandle for WindowHandleSource {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, HandleError> {
        self.0.display_handle()
    }
}

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

pub struct SoftwareSurface {
    _context: Context<WindowHandleSource>,
    surface: Surface<WindowHandleSource, WindowHandleSource>,
    pixmap: Pixmap,
}

impl SoftwareSurface {
    pub fn new(window: Arc<Window>) -> Result<Self, SurfaceError> {
        let size = window.inner_size();
        let context = Context::new(WindowHandleSource(Arc::clone(&window)))
            .map_err(|error| SurfaceError::Context(error.to_string()))?;
        let mut surface = Surface::new(&context, WindowHandleSource(window))
            .map_err(|error| SurfaceError::Surface(error.to_string()))?;
        let width = NonZeroU32::new(size.width).ok_or(SurfaceError::ZeroSize)?;
        let height = NonZeroU32::new(size.height).ok_or(SurfaceError::ZeroSize)?;
        surface
            .resize(width, height)
            .map_err(|error| SurfaceError::Surface(error.to_string()))?;
        let pixmap =
            Pixmap::new(size.width, size.height).ok_or(SurfaceError::PixmapAllocation {
                width: size.width,
                height: size.height,
            })?;
        Ok(Self {
            _context: context,
            surface,
            pixmap,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), SurfaceError> {
        let width_nonzero = NonZeroU32::new(width).ok_or(SurfaceError::ZeroSize)?;
        let height_nonzero = NonZeroU32::new(height).ok_or(SurfaceError::ZeroSize)?;
        self.surface
            .resize(width_nonzero, height_nonzero)
            .map_err(|error| SurfaceError::Surface(error.to_string()))?;
        if self.pixmap.width() != width || self.pixmap.height() != height {
            self.pixmap = Pixmap::new(width, height)
                .ok_or(SurfaceError::PixmapAllocation { width, height })?;
        }
        Ok(())
    }

    pub fn pixmap_mut(&mut self) -> &mut Pixmap {
        &mut self.pixmap
    }

    pub fn present(&mut self) -> Result<(), SurfaceError> {
        let mut buffer = self
            .surface
            .buffer_mut()
            .map_err(|error| SurfaceError::Surface(error.to_string()))?;
        if buffer.len().checked_mul(4) != Some(self.pixmap.data().len()) {
            return Err(SurfaceError::LengthMismatch);
        }
        for (destination, rgba) in buffer.iter_mut().zip(self.pixmap.data().chunks_exact(4)) {
            *destination = ((rgba[0] as u32) << 16) | ((rgba[1] as u32) << 8) | rgba[2] as u32;
        }
        buffer
            .present()
            .map_err(|error| SurfaceError::Present(error.to_string()))
    }
}
