//! Source-preserving export projection for real Comrak image nodes.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#export

use super::{BlockNode, ImageKind, InlineNode, OwnedDocumentTree, SourceRange};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageOccurrence {
    pub destination: String,
    pub title: String,
    pub alt: String,
    pub kind: ImageKind,
    pub source_range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRewrite {
    pub source_range: SourceRange,
    pub destination: String,
    pub title: String,
    pub alt: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ExportProjectionError {
    #[error("an image node has no stable source range")]
    MissingSourceRange,
    #[error("image rewrite ranges overlap or are out of bounds")]
    InvalidRewriteRange,
}

pub fn collect_image_occurrences(
    tree: &OwnedDocumentTree,
) -> Result<Vec<ImageOccurrence>, ExportProjectionError> {
    let mut output = Vec::new();
    collect_blocks(&tree.blocks, &mut output)?;
    output.sort_by_key(|image| image.source_range.start);
    Ok(output)
}

pub fn rewrite_image_occurrences(
    source: &str,
    mut rewrites: Vec<ImageRewrite>,
) -> Result<String, ExportProjectionError> {
    rewrites.sort_by_key(|rewrite| rewrite.source_range.start);
    let mut previous_end = 0;
    for rewrite in &rewrites {
        if rewrite.source_range.start < previous_end
            || rewrite.source_range.end > source.len()
            || source.get(rewrite.source_range.as_range()).is_none()
        {
            return Err(ExportProjectionError::InvalidRewriteRange);
        }
        previous_end = rewrite.source_range.end;
    }
    let mut output = source.to_owned();
    for rewrite in rewrites.into_iter().rev() {
        let replacement = inline_image(&rewrite.alt, &rewrite.destination, &rewrite.title);
        output.replace_range(rewrite.source_range.as_range(), &replacement);
    }
    Ok(output)
}

fn collect_blocks(
    blocks: &[BlockNode],
    output: &mut Vec<ImageOccurrence>,
) -> Result<(), ExportProjectionError> {
    for block in blocks {
        match block {
            BlockNode::Paragraph { content, .. } | BlockNode::Heading { content, .. } => {
                collect_inlines(content, output)?
            }
            BlockNode::BlockQuote { blocks, .. } => collect_blocks(blocks, output)?,
            BlockNode::List(list) => {
                for item in &list.items {
                    collect_blocks(&item.blocks, output)?;
                }
            }
            BlockNode::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        collect_inlines(&cell.content, output)?;
                    }
                }
            }
            BlockNode::CodeBlock(_)
            | BlockNode::ThematicBreak { .. }
            | BlockNode::HtmlLiteral { .. }
            | BlockNode::DisplayMath(_) => {}
        }
    }
    Ok(())
}

fn collect_inlines(
    inlines: &[InlineNode],
    output: &mut Vec<ImageOccurrence>,
) -> Result<(), ExportProjectionError> {
    for inline in inlines {
        match inline {
            InlineNode::Image {
                destination,
                title,
                alt,
                kind,
                source_range,
            } => output.push(ImageOccurrence {
                destination: destination.clone(),
                title: title.clone(),
                alt: alt.clone(),
                kind: *kind,
                source_range: source_range.ok_or(ExportProjectionError::MissingSourceRange)?,
            }),
            InlineNode::Emphasis { children, .. }
            | InlineNode::Strong { children, .. }
            | InlineNode::Strikethrough { children, .. }
            | InlineNode::Link { children, .. } => collect_inlines(children, output)?,
            _ => {}
        }
    }
    Ok(())
}

fn inline_image(alt: &str, destination: &str, title: &str) -> String {
    let alt = alt.replace('\\', "\\\\").replace(']', "\\]");
    let destination = destination.replace('\\', "/").replace(')', "\\)");
    if title.is_empty() {
        format!("![{alt}]({destination})")
    } else {
        let title = title.replace('\\', "\\\\").replace('"', "\\\"");
        format!("![{alt}]({destination} \"{title}\")")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::PreviewParser;
    use std::sync::Arc;
    use stickymd_core::{DocumentSnapshot, Generation, LineEnding};

    #[test]
    fn only_real_image_nodes_are_collected_and_reference_occurrence_is_localized() {
        let source = "![inline](a.png)\n\n![ref][pic]\n\n[normal][pic]\n\n`![code](x.png)`\n\n<div>![raw](y.png)</div>\n\n[pic]: b.png \"T\"\n";
        let snapshot = DocumentSnapshot {
            text: Arc::from(source),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        let tree = PreviewParser.parse(&snapshot).unwrap();
        let images = collect_image_occurrences(&tree).unwrap();
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].destination, "a.png");
        assert_eq!(images[1].destination, "b.png");
        let rewritten = rewrite_image_occurrences(
            source,
            vec![ImageRewrite {
                source_range: images[1].source_range,
                destination: "note-assets/b.png".into(),
                title: images[1].title.clone(),
                alt: images[1].alt.clone(),
            }],
        )
        .unwrap();
        assert!(rewritten.contains("![ref](note-assets/b.png \"T\")"));
        assert!(rewritten.contains("[normal][pic]"));
        assert!(rewritten.contains("[pic]: b.png \"T\""));
        assert!(rewritten.contains("`![code](x.png)`"));
    }
}
