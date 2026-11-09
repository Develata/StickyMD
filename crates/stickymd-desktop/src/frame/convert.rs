//! RGBA pixmap to native `0RGB` conversion, the whole frame or one rectangle.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#frame-presentation

use super::plan::clamp_damage;
use super::{DamageRect, PresentPlan, SurfaceError};

/// Writes `plan` into `native` and returns the RGBA bytes it read.
pub(super) fn write_plan(
    plan: PresentPlan,
    rgba: &[u8],
    native: &mut [u32],
    width: u32,
    height: u32,
) -> Result<usize, SurfaceError> {
    match plan {
        PresentPlan::Full => {
            copy_rgba_full(rgba, native);
            Ok(rgba.len())
        }
        PresentPlan::Damage(damage) => {
            copy_rgba_damage(rgba, native, width, height, damage)?;
            Ok(damage.width as usize * damage.height as usize * 4)
        }
        PresentPlan::Nothing => Ok(0),
    }
}

pub(super) fn copy_rgba_full(rgba: &[u8], native: &mut [u32]) {
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

#[cfg(test)]
mod tests {
    use super::super::plan_present;
    use super::{DamageRect, PresentPlan, SurfaceError, copy_rgba_damage, write_plan};

    const RGBA: [u8; 16] = [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255];
    const SENTINEL: u32 = u32::MAX;

    fn plan(age: u8, damage: DamageRect) -> PresentPlan {
        plan_present(age, 4, RGBA.len(), damage, 2, 2).unwrap()
    }

    #[test]
    fn phase9_caret_damage_conversion_does_not_touch_other_pixels() {
        let mut native = [SENTINEL; 4];

        copy_rgba_damage(&RGBA, &mut native, 2, 2, DamageRect::new(1, 0, 1, 2))
            .expect("valid damage rectangle");

        assert_eq!(native, [SENTINEL, 0x0004_0506, SENTINEL, 0x000a_0b0c]);
    }

    /// Plan 06: a buffer that is the previous frame takes only the damage.
    #[test]
    fn age_one_writes_only_the_damage() {
        let damage = DamageRect::new(1, 0, 1, 2);
        assert_eq!(plan(1, damage), PresentPlan::Damage(damage));
        let mut native = [SENTINEL; 4];
        let written = write_plan(plan(1, damage), &RGBA, &mut native, 2, 2).unwrap();
        assert_eq!(native, [SENTINEL, 0x0004_0506, SENTINEL, 0x000a_0b0c]);
        assert_eq!(written, 8);
    }

    /// Plan 06: a new buffer, or an older one from double buffering, is filled whole, so nothing of
    /// a stale frame survives outside the damage.
    #[test]
    fn every_other_age_fills_the_whole_buffer() {
        for age in [0, 2, 3] {
            let damaged = plan(age, DamageRect::new(1, 0, 1, 2));
            assert_eq!(damaged, PresentPlan::Full, "age {age}");
            let mut native = [SENTINEL; 4];
            let written = write_plan(damaged, &RGBA, &mut native, 2, 2).unwrap();
            assert!(!native.contains(&SENTINEL), "age {age}");
            assert_eq!(written, RGBA.len());
        }
    }

    #[test]
    fn the_age_decides_before_the_damage_is_clipped() {
        let outside = DamageRect::new(9, 9, 1, 1);
        assert_eq!(plan(1, outside), PresentPlan::Nothing);
        for age in [0, 2, 3] {
            assert_eq!(plan(age, outside), PresentPlan::Full, "age {age}");
        }
        let mut native = [SENTINEL; 4];
        let written = write_plan(PresentPlan::Nothing, &RGBA, &mut native, 2, 2).unwrap();
        assert_eq!((native, written), ([SENTINEL; 4], 0));
    }

    #[test]
    fn a_length_mismatch_is_an_error_at_every_age() {
        for age in [0, 1, 2] {
            let result = plan_present(age, 3, RGBA.len(), DamageRect::new(0, 0, 1, 1), 2, 2);
            assert!(matches!(result, Err(SurfaceError::LengthMismatch)));
        }
    }
}
