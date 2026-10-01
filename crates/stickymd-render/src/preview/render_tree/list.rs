//! List markers remain separate from the first child block's semantics.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#owned-ast-projection

use super::{ListNode, RenderBlock, RenderBlockKind, RenderStyle, RenderTreeBuilder, span};

impl RenderTreeBuilder {
    pub(super) fn append_list(
        &self,
        list: &ListNode,
        indent: u16,
        quoted: bool,
        output: &mut Vec<RenderBlock>,
    ) {
        for (index, item) in list.items.iter().enumerate() {
            let marker = match item.checked {
                Some(true) => "☑ ".to_owned(),
                Some(false) => "☐ ".to_owned(),
                None if list.ordered => format!("{}. ", list.start + index),
                None => "• ".to_owned(),
            };
            let before = output.len();
            self.append_blocks(&item.blocks, indent.saturating_add(1), quoted, output);
            if output.len() == before {
                output.push(RenderBlock {
                    kind: RenderBlockKind::ListItem,
                    spans: Vec::new(),
                    indent: indent.saturating_add(1),
                    source_range: item.source_range,
                    list_markers: Vec::new(),
                });
            }
            if let Some(first) = output.get_mut(before) {
                if matches!(first.kind, RenderBlockKind::Paragraph) {
                    first.kind = RenderBlockKind::ListItem;
                }
                first.list_markers.insert(
                    0,
                    span(
                        &marker,
                        &marker,
                        None,
                        RenderStyle {
                            code: matches!(
                                first.kind,
                                RenderBlockKind::CodeBlock { .. } | RenderBlockKind::HtmlLiteral
                            ),
                            ..RenderStyle::default()
                        },
                        None,
                    ),
                );
            }
        }
    }
}
