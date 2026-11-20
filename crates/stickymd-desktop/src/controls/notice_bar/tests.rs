use stickymd_app::config::{MIN_WINDOW_HEIGHT_DIP, MIN_WINDOW_WIDTH_DIP, ViewMode};
use stickymd_app::flow::notice::{Choice, Choices};

use super::*;
use crate::geometry::geometry;

fn window(width_dip: f32, height_dip: f32, dpi: f32) -> PhysicalSize<u32> {
    PhysicalSize::new(
        (width_dip * dpi).round() as u32,
        (height_dip * dpi).round() as u32,
    )
}

/// P17-N15: the bar is 34 window DIP at the bottom of the window, whatever the Content Zoom (it
/// is not an input), and even the smallest window leaves it inside every pane's viewport.
#[test]
fn the_bar_is_34_window_dip_and_fits_the_smallest_window() {
    for dpi in [1.0_f32, 1.25, 1.5, 2.0, 3.0] {
        let size = window(
            MIN_WINDOW_WIDTH_DIP as f32,
            MIN_WINDOW_HEIGHT_DIP as f32,
            dpi,
        );
        let layout = NoticeBarLayout::new(size, dpi, None);
        assert_eq!(layout.bar.height, 34.0 * dpi);
        assert_eq!(layout.bar.y + layout.bar.height, size.height as f32);
        assert_eq!(layout.bar.width, size.width as f32);
        for mode in [ViewMode::Source, ViewMode::Split, ViewMode::Preview] {
            let view = geometry(mode, size, dpi);
            for pane in [view.source, view.preview].into_iter().flatten() {
                assert!(
                    layout.bar.height <= pane.height as f32,
                    "{dpi} {mode:?}: the bar is taller than the pane"
                );
                assert!(layout.bar.y >= pane.y as f32, "{dpi} {mode:?}");
            }
        }
    }
}

/// P17-N07: a button per offered choice, F6 in the left slot and F7 in the right one; no
/// choices leave the whole width to the text.
#[test]
fn a_button_per_offered_choice_and_none_without_a_decision() {
    let size = window(400.0, 300.0, 1.0);
    let both = Choices {
        primary: Some(Choice::LoadExternal),
        secondary: Some(Choice::KeepLocal),
    };
    let layout = NoticeBarLayout::new(size, 1.0, Some(both));
    let [Some((first, left)), Some((second, right))] = layout.buttons else {
        panic!("two buttons");
    };
    assert_eq!((first, second), (Choice::LoadExternal, Choice::KeepLocal));
    assert!(left.x < right.x);
    let y = f64::from(left.y + left.height / 2.0);
    assert_eq!(
        layout.choice_at(f64::from(left.x + 1.0), y),
        Some(Choice::LoadExternal)
    );
    assert_eq!(
        layout.choice_at(f64::from(right.x + 1.0), y),
        Some(Choice::KeepLocal)
    );
    assert_eq!(layout.choice_at(5.0, y), None, "the text is not a button");
    assert!(layout.text.x + layout.text.width <= left.x);

    let only_f7 = Choices {
        primary: None,
        secondary: Some(Choice::KeepLocal),
    };
    let layout = NoticeBarLayout::new(size, 1.0, Some(only_f7));
    assert!(layout.buttons[0].is_none());
    assert_eq!(
        layout.buttons[1].map(|(choice, _)| choice),
        Some(Choice::KeepLocal)
    );
    assert_eq!(layout.choice_at(f64::from(left.x + 1.0), y), None);

    // The buttons end before the rightmost scrollbar gutter, which stays the scrollbar's.
    assert!(right.x + right.width <= 400.0 - 20.0);

    let layout = NoticeBarLayout::new(size, 1.0, None);
    assert_eq!(layout.buttons, [None, None]);
    assert!(layout.text.x + layout.text.width > 400.0 * ACTIONS_START);
}
