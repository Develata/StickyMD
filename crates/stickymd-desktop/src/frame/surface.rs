//! Software framebuffer presentation adapter.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#frame-presentation

use std::num::NonZeroU32;
use std::sync::Arc;

use raw_window_handle::{HandleError, HasDisplayHandle, HasWindowHandle};
use softbuffer::{Context, Surface};
use tiny_skia::Pixmap;
use winit::window::Window;

use super::convert::{copy_rgba_full, write_plan};
use super::{DamageRect, FrameLedger, PresentPlan, SurfaceError, plan_present};

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

pub struct SoftwareSurface {
    _context: Context<WindowHandleSource>,
    surface: Surface<WindowHandleSource, WindowHandleSource>,
    pixmap: Pixmap,
    ledger: FrameLedger,
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
            ledger: FrameLedger::default(),
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
            self.ledger.pixmap_allocs += 1;
        }
        Ok(())
    }

    pub fn pixmap_mut(&mut self) -> &mut Pixmap {
        &mut self.pixmap
    }

    pub fn ledger_mut(&mut self) -> &mut FrameLedger {
        &mut self.ledger
    }

    pub fn present(&mut self) -> Result<(), SurfaceError> {
        let mut buffer = self
            .surface
            .buffer_mut()
            .map_err(|error| SurfaceError::Surface(error.to_string()))?;
        if buffer.len().checked_mul(4) != Some(self.pixmap.data().len()) {
            return Err(SurfaceError::LengthMismatch);
        }
        copy_rgba_full(self.pixmap.data(), &mut buffer);
        self.ledger.record_full(self.pixmap.data().len());
        buffer
            .present()
            .map_err(|error| SurfaceError::Present(error.to_string()))
    }

    /// Converts and presents only a stable framebuffer rectangle.
    ///
    /// Win32 softbuffer keeps a single persistent DIB (`age == 1`), so caret
    /// blinking can update a few dozen pixels instead of converting and
    /// copying the entire window. Any other buffer is populated in full.
    pub fn present_damage(&mut self, damage: DamageRect) -> Result<(), SurfaceError> {
        let mut buffer = self
            .surface
            .buffer_mut()
            .map_err(|error| SurfaceError::Surface(error.to_string()))?;
        let (width, height) = (self.pixmap.width(), self.pixmap.height());
        let plan = plan_present(
            buffer.age(),
            buffer.len(),
            self.pixmap.data().len(),
            damage,
            width,
            height,
        )?;
        let written = write_plan(plan, self.pixmap.data(), &mut buffer, width, height)?;
        match plan {
            PresentPlan::Nothing => Ok(()),
            PresentPlan::Full => {
                self.ledger.record_full(written);
                buffer
                    .present()
                    .map_err(|error| SurfaceError::Present(error.to_string()))
            }
            PresentPlan::Damage(damage) => {
                self.ledger.convert_damage_bytes += written as u64;
                self.ledger.damage_presents += 1;
                let width = NonZeroU32::new(damage.width).ok_or(SurfaceError::ZeroSize)?;
                let height = NonZeroU32::new(damage.height).ok_or(SurfaceError::ZeroSize)?;
                buffer
                    .present_with_damage(&[softbuffer::Rect {
                        x: damage.x,
                        y: damage.y,
                        width,
                        height,
                    }])
                    .map_err(|error| SurfaceError::Present(error.to_string()))
            }
        }
    }
}
