//! Unicode-grapheme navigation helpers.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor

use unicode_segmentation::UnicodeSegmentation;

pub fn previous_grapheme_boundary(text: &str, byte: usize) -> usize {
    let byte = floor_char_boundary(text, byte);
    text[..byte]
        .grapheme_indices(true)
        .next_back()
        .map_or(0, |(index, _)| index)
}

pub fn next_grapheme_boundary(text: &str, byte: usize) -> usize {
    let byte = floor_char_boundary(text, byte);
    if byte >= text.len() {
        return text.len();
    }
    let suffix = &text[byte..];
    suffix
        .graphemes(true)
        .next()
        .map_or(text.len(), |grapheme| byte + grapheme.len())
}

pub fn logical_line_start(text: &str, byte: usize) -> usize {
    let byte = floor_char_boundary(text, byte);
    text[..byte].rfind('\n').map_or(0, |index| index + 1)
}

pub fn logical_line_end(text: &str, byte: usize) -> usize {
    let byte = floor_char_boundary(text, byte);
    text[byte..]
        .find('\n')
        .map_or(text.len(), |offset| byte + offset)
}

fn floor_char_boundary(text: &str, byte: usize) -> usize {
    let mut byte = byte.min(text.len());
    while !text.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_step_over_complete_graphemes() {
        for grapheme in ["a", "中", "é", "e\u{301}", "🙂", "👨‍👩‍👧‍👦", "🇨🇳"]
        {
            assert_eq!(next_grapheme_boundary(grapheme, 0), grapheme.len());
            assert_eq!(previous_grapheme_boundary(grapheme, grapheme.len()), 0);
        }
    }

    #[test]
    fn logical_line_bounds_exclude_newline() {
        let text = "first\n第二行\nlast";
        let position = text.find('行').unwrap();
        assert_eq!(
            &text[logical_line_start(text, position)..logical_line_end(text, position)],
            "第二行"
        );
    }

    #[test]
    fn invalid_mid_codepoint_positions_are_clamped_without_panicking() {
        let text = "a中b";
        assert_eq!(previous_grapheme_boundary(text, 2), 0);
        assert_eq!(next_grapheme_boundary(text, 2), 4);
        assert_eq!(logical_line_start(text, 2), 0);
        assert_eq!(logical_line_end(text, 2), text.len());
    }
}
