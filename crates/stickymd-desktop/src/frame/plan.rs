//! How a damaged frame reaches the native buffer, decided from the buffer age alone.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#frame-presentation

use super::SurfaceError;

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

/// How a damaged frame reaches the native buffer (plan 06 frame presentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentPlan {
    /// The buffer does not hold the previous frame: fill all of it and present it whole.
    Full,
    /// The buffer is the previous frame: write and present only this rectangle.
    Damage(DamageRect),
    /// The buffer is the previous frame and the damage is empty.
    Nothing,
}

/// Only a buffer that is exactly the previous frame (`age == 1`) may be updated in part. The age is
/// read before the damage is clipped: any other buffer needs a whole frame even with no damage.
pub fn plan_present(
    age: u8,
    buffer_len: usize,
    pixmap_len: usize,
    damage: DamageRect,
    width: u32,
    height: u32,
) -> Result<PresentPlan, SurfaceError> {
    if buffer_len.checked_mul(4) != Some(pixmap_len) {
        return Err(SurfaceError::LengthMismatch);
    }
    if age != 1 {
        return Ok(PresentPlan::Full);
    }
    Ok(clamp_damage(damage, width, height).map_or(PresentPlan::Nothing, PresentPlan::Damage))
}

pub(super) fn clamp_damage(damage: DamageRect, width: u32, height: u32) -> Option<DamageRect> {
    let x = damage.x.min(width);
    let y = damage.y.min(height);
    let right = damage.x.saturating_add(damage.width).min(width);
    let bottom = damage.y.saturating_add(damage.height).min(height);
    let width = right.saturating_sub(x);
    let height = bottom.saturating_sub(y);
    (width != 0 && height != 0).then_some(DamageRect::new(x, y, width, height))
}
