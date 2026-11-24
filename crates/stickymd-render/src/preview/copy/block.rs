//! Code and Raw HTML blocks: how a display position inside one maps to its source, and whether
//! the range Comrak reported can be trusted for that.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-copy-semantics

use super::display::Piece;
use super::endpoint::Side;

/// Whether the block's source range holds at least the lines its text shows, plus the open fence
/// line when fenced. Comrak has reported a block cut short (0.59: a fenced block in a list item at
/// the end of the document), and such a range cannot be mapped through.
pub(super) fn holds_its_lines(text: &str, source: &str, piece: Piece, fenced: bool) -> bool {
    let (Some(shown), Some(range)) = (text.get(piece.display()), source.get(piece.source())) else {
        return false;
    };
    let lines = |text: &str| text.strip_suffix('\n').unwrap_or(text).split('\n').count();
    lines(range) >= lines(shown) + usize::from(fenced)
}

/// The source position of display boundary `boundary` inside a code or Raw HTML block piece.
/// Display line `k` of the block's literal is a suffix of source line `k` of the block (after the
/// open fence line when fenced), since Comrak only strips container prefixes and indentation.
/// When that does not hold the boundary rounds outward to its source line. `None` when the line
/// does not exist. O(block bytes).
pub(super) fn position(
    text: &str,
    source: &str,
    piece: Piece,
    fenced: bool,
    boundary: usize,
    side: Side,
) -> Option<usize> {
    let literal = text.get(piece.display())?;
    let block = source.get(piece.source())?;
    let offset = boundary - piece.start as usize;
    // A char boundary on the display line holding the bound byte: the byte after the boundary
    // for a start, the one before it for an end. They differ only when the end follows a newline,
    // which then binds the end to the line that newline closes.
    let byte = match side {
        Side::End if literal[..offset].ends_with('\n') => offset - 1,
        Side::Start | Side::End => offset,
    };
    let line_start = literal[..byte].rfind('\n').map_or(0, |index| index + 1);
    let line_end = literal[byte..]
        .find('\n')
        .map_or(literal.len(), |index| byte + index);
    let line = &literal[line_start..line_end];
    let number = literal[..line_start].matches('\n').count();
    let mut source_start = if fenced { block.find('\n')? + 1 } else { 0 };
    for _ in 0..number {
        source_start += block[source_start..].find('\n')? + 1;
    }
    let source_end = block[source_start..]
        .find('\n')
        .map_or(block.len(), |index| source_start + index);
    let base = piece.source_start as usize;
    let source_line = &block[source_start..source_end];
    if source_line.ends_with(line) {
        Some(base + source_end - line.len() + (offset - line_start))
    } else {
        Some(
            base + match side {
                Side::Start => source_start,
                Side::End => source_end,
            },
        )
    }
}
