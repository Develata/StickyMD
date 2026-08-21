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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl DamageRect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
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
        copy_rgba_full(self.pixmap.data(), &mut buffer);
        buffer
            .present()
            .map_err(|error| SurfaceError::Present(error.to_string()))
    }

    /// Converts and presents only a stable framebuffer rectangle.
    ///
    /// Win32 softbuffer keeps a single persistent DIB (`age == 1`), so caret
    /// blinking can update a few dozen pixels instead of converting and
    /// copying the entire window. A newly allocated buffer has unspecified
    /// contents and is therefore populated in full before its first present.
    pub fn present_damage(&mut self, damage: DamageRect) -> Result<(), SurfaceError> {
        let mut buffer = self
            .surface
            .buffer_mut()
            .map_err(|error| SurfaceError::Surface(error.to_string()))?;
        if buffer.len().checked_mul(4) != Some(self.pixmap.data().len()) {
            return Err(SurfaceError::LengthMismatch);
        }
        if buffer.age() == 0 {
            copy_rgba_full(self.pixmap.data(), &mut buffer);
            return buffer
                .present()
                .map_err(|error| SurfaceError::Present(error.to_string()));
        }
        let Some(damage) = clamp_damage(damage, self.pixmap.width(), self.pixmap.height()) else {
            return Ok(());
        };
        copy_rgba_damage(
            self.pixmap.data(),
            &mut buffer,
            self.pixmap.width(),
            self.pixmap.height(),
            damage,
        )?;
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

fn copy_rgba_full(rgba: &[u8], native: &mut [u32]) {
    for (destination, rgba) in native.iter_mut().zip(rgba.chunks_exact(4)) {
        *destination = ((rgba[0] as u32) << 16) | ((rgba[1] as u32) << 8) | rgba[2] as u32;
    }
}

fn copy_rgba_damage(
    rgba: &[u8],
    native: &mut [u32],
    width: u32,
    height: u32,
    damage: DamageRect,
) -> Result<(), SurfaceError> {
    if native.len() != width as usize * height as usize
        || rgba.len() != native.len().saturating_mul(4)
    {
        return Err(SurfaceError::LengthMismatch);
    }
    let Some(damage) = clamp_damage(damage, width, height) else {
        return Ok(());
    };
    let stride = width as usize;
    let start_x = damage.x as usize;
    let end_x = start_x + damage.width as usize;
    for y in damage.y as usize..(damage.y + damage.height) as usize {
        let row = y * stride;
        for x in start_x..end_x {
            let pixel = row + x;
            let source = pixel * 4;
            native[pixel] = ((rgba[source] as u32) << 16)
                | ((rgba[source + 1] as u32) << 8)
                | rgba[source + 2] as u32;
        }
    }
    Ok(())
}

fn clamp_damage(damage: DamageRect, width: u32, height: u32) -> Option<DamageRect> {
    let x = damage.x.min(width);
    let y = damage.y.min(height);
    let right = damage.x.saturating_add(damage.width).min(width);
    let bottom = damage.y.saturating_add(damage.height).min(height);
    let width = right.saturating_sub(x);
    let height = bottom.saturating_sub(y);
    (width != 0 && height != 0).then_some(DamageRect::new(x, y, width, height))
}

#[cfg(test)]
mod tests {
    use super::{DamageRect, copy_rgba_damage};

    #[test]
    fn phase9_caret_damage_conversion_does_not_touch_other_pixels() {
        let rgba = [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255];
        let mut native = [u32::MAX; 4];

        copy_rgba_damage(&rgba, &mut native, 2, 2, DamageRect::new(1, 0, 1, 2))
            .expect("valid damage rectangle");

        assert_eq!(native, [u32::MAX, 0x0004_0506, u32::MAX, 0x000a_0b0c]);
    }
}
