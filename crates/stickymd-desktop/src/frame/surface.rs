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

/// Where a finished frame goes: softbuffer's surface, or the host's own presenter, which takes
/// the canvas itself (`Host::presents_frames`, plan 06 frame presentation).
enum Output {
    Softbuffer {
        _context: Context<WindowHandleSource>,
        surface: Surface<WindowHandleSource, WindowHandleSource>,
    },
    Host,
}

/// The softbuffer surface, borrowing only `output` so the canvas and ledger stay usable beside it.
fn softbuffer(
    output: &mut Output,
) -> Result<&mut Surface<WindowHandleSource, WindowHandleSource>, SurfaceError> {
    match output {
        Output::Softbuffer { surface, .. } => Ok(surface),
        Output::Host => Err(SurfaceError::Present(
            "the host presents this canvas".into(),
        )),
    }
}

pub struct SoftwareSurface {
    output: Output,
    pixmap: Pixmap,
    ledger: FrameLedger,
}

impl SoftwareSurface {
    /// Only the canvas: the host presents what is drawn into it. Needs no window.
    pub fn for_host(width: u32, height: u32) -> Result<Self, SurfaceError> {
        if width == 0 || height == 0 {
            return Err(SurfaceError::ZeroSize);
        }
        let pixmap =
            Pixmap::new(width, height).ok_or(SurfaceError::PixmapAllocation { width, height })?;
        Ok(Self {
            output: Output::Host,
            pixmap,
            ledger: FrameLedger::default(),
        })
    }

    /// Whether the host presents this surface's canvas (`for_host`).
    pub fn presented_by_host(&self) -> bool {
        matches!(self.output, Output::Host)
    }

    /// The canvas, for a host that presents it.
    pub fn pixmap(&self) -> &Pixmap {
        &self.pixmap
    }

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
            output: Output::Softbuffer {
                _context: context,
                surface,
            },
            pixmap,
            ledger: FrameLedger::default(),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), SurfaceError> {
        let width_nonzero = NonZeroU32::new(width).ok_or(SurfaceError::ZeroSize)?;
        let height_nonzero = NonZeroU32::new(height).ok_or(SurfaceError::ZeroSize)?;
        if let Output::Softbuffer { surface, .. } = &mut self.output {
            surface
                .resize(width_nonzero, height_nonzero)
                .map_err(|error| SurfaceError::Surface(error.to_string()))?;
        }
        if self.pixmap.width() != width || self.pixmap.height() != height {
            self.pixmap = Pixmap::new(width, height)
                .ok_or(SurfaceError::PixmapAllocation { width, height })?;
            self.ledger.pixmap_allocs += 1;
        }
        Ok(())
    }

    /// Gives up the window-sized storage while the window is hidden: the canvas and the
    /// presenter shrink to one pixel, and presenting that pixel replaces the layer's image, so
    /// the presenter's buffer and the compositor's copy of it go too. A host presenter releases
    /// its own buffers (`Host::hidden_frames_released`); only the canvas shrinks here. `resize`
    /// to the window's size brings it back; the caller repaints before showing the window.
    pub fn release_frames(&mut self) -> Result<(), SurfaceError> {
        self.resize(1, 1)?;
        if self.presented_by_host() {
            return Ok(());
        }
        self.present()
    }

    pub fn pixmap_mut(&mut self) -> &mut Pixmap {
        &mut self.pixmap
    }

    pub fn ledger_mut(&mut self) -> &mut FrameLedger {
        &mut self.ledger
    }

    pub fn present(&mut self) -> Result<(), SurfaceError> {
        let mut buffer = softbuffer(&mut self.output)?
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
        let mut buffer = softbuffer(&mut self.output)?
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
