//! The collapsed dock strip as the manual recorder judges it: the window sits past one edge of
//! a work area and leaves exactly the product's sensor strip inside it. Platform-free, so the
//! rule is tested on every host; the Windows probe supplies the rectangles and the DPI.
//! plan_ref: docs/plan/09_windows_shell.md#dock-auto-hide

/// The sensor strip a collapsed window keeps (plan 09 #dock-auto-hide, "保留 2 DIP 感应条"),
/// the same on every host since 0.2.0. The probe judged 3 DIP until the M5 freeze, so a
/// collapsed window was never seen as collapsed (v0.2.0 candidate record).
pub(crate) const COLLAPSED_STRIP_DIP: f64 = 2.0;

/// A rectangle in physical pixels: the window or a monitor's work area.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Area {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

/// The strip in physical pixels at `dpi` (96 is 100%), rounded as the probe's DIP conversion
/// rounds.
pub(crate) fn strip_pixels(dpi: u32) -> i64 {
    (COLLAPSED_STRIP_DIP * f64::from(dpi) / 96.0).round() as i64
}

/// Whether `window` is collapsed against the left, right or top edge of one of `works`, leaving
/// exactly `strip` pixels inside it and lying within that work area along the other axis.
pub(crate) fn collapsed_on(window: Area, works: &[Area], strip: i64) -> bool {
    let right = i64::from(window.x) + i64::from(window.width);
    let bottom = i64::from(window.y) + i64::from(window.height);
    works.iter().any(|w| {
        let wr = i64::from(w.x) + i64::from(w.width);
        let wb = i64::from(w.y) + i64::from(w.height);
        let vertical = window.y >= w.y && bottom <= wb;
        let horizontal = window.x >= w.x && right <= wr;
        (vertical && window.x < w.x && right - i64::from(w.x) == strip)
            || (vertical && right > wr && wr - i64::from(window.x) == strip)
            || (horizontal && window.y < w.y && bottom - i64::from(w.y) == strip)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Area = Area {
        x: 0,
        y: 0,
        width: 1920,
        height: 1040,
    };

    /// A 300×200 window collapsed so that `inside` pixels of it stay on screen at `edge`.
    fn collapsed(edge: &str, inside: i32) -> Area {
        match edge {
            "left" => Area {
                x: inside - 300,
                y: 100,
                width: 300,
                height: 200,
            },
            "right" => Area {
                x: 1920 - inside,
                y: 100,
                width: 300,
                height: 200,
            },
            _ => Area {
                x: 100,
                y: inside - 200,
                width: 300,
                height: 200,
            },
        }
    }

    /// The product's 2 DIP strip is what the recorder sees as collapsed, at 100%, 125% and 200%:
    /// the probe's former 3 DIP refused every one of these (M5 freeze, v0.2.0's deferred fix).
    #[test]
    fn the_products_two_dip_strip_reads_as_collapsed() {
        for (dpi, inside) in [(96, 2), (120, 3), (192, 4)] {
            assert_eq!(strip_pixels(dpi), i64::from(inside));
            for edge in ["left", "right", "top"] {
                assert!(
                    collapsed_on(collapsed(edge, inside), &[WORK], strip_pixels(dpi)),
                    "{edge} at {dpi} DPI"
                );
            }
        }
        // One pixel more or less is not the strip; a window wholly inside is not collapsed.
        for edge in ["left", "right", "top"] {
            assert!(!collapsed_on(collapsed(edge, 3), &[WORK], strip_pixels(96)));
            assert!(!collapsed_on(collapsed(edge, 1), &[WORK], strip_pixels(96)));
        }
        let inside = Area {
            x: 10,
            y: 10,
            width: 300,
            height: 200,
        };
        assert!(!collapsed_on(inside, &[WORK], strip_pixels(96)));
    }
}
