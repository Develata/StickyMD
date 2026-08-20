//! Deletable Phase 1 Comrak/RaTeX verification.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#owned-ast-projection

use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, Options, parse_document};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedNode {
    pub kind: String,
    pub source_position: String,
    pub literal: Option<String>,
    pub display_math: Option<bool>,
    pub children: Vec<OwnedNode>,
}

fn options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.math_dollars = true;
    options.extension.math_latex = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options
}

fn literal(value: &NodeValue) -> (Option<String>, Option<bool>) {
    match value {
        NodeValue::Text(text) => (Some(text.to_string()), None),
        NodeValue::Math(math) => (Some(math.literal.clone()), Some(math.display_math)),
        NodeValue::HtmlBlock(html) => (Some(html.literal.clone()), None),
        NodeValue::HtmlInline(html) => (Some(html.clone()), None),
        NodeValue::Code(code) => (Some(code.literal.clone()), None),
        NodeValue::CodeBlock(code) => (Some(code.literal.clone()), None),
        _ => (None, None),
    }
}

fn own<'a>(node: &'a AstNode<'a>) -> OwnedNode {
    let data = node.data.borrow();
    let kind = data.value.xml_node_name().to_string();
    let source_position = data.sourcepos.to_string();
    let (literal, display_math) = literal(&data.value);
    drop(data);
    OwnedNode {
        kind,
        source_position,
        literal,
        display_math,
        children: node.children().map(own).collect(),
    }
}

/// Parses into a project-owned diagnostic tree, then drops the Comrak Arena.
pub fn parse_owned(source: &str) -> OwnedNode {
    let arena = Arena::new();
    let root = parse_document(&arena, source, &options());
    own(root)
}

pub fn visit<'a>(root: &'a OwnedNode, kind: &str, out: &mut Vec<&'a OwnedNode>) {
    if root.kind == kind {
        out.push(root);
    }
    for child in &root.children {
        visit(child, kind, out);
    }
}

pub fn render_math_png(source: &str) -> Result<Vec<u8>, String> {
    let nodes = ratex_parser::parse(source).map_err(|error| format!("{error:?}"))?;
    let layout = ratex_layout::engine::layout(&nodes, &Default::default());
    let display_list = ratex_layout::to_display::to_display_list(&layout);
    ratex_render::render_to_png(&display_list, &Default::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_four_math_delimiters_are_comrak_nodes() {
        let tree = parse_owned("$a$\n\n$$b$$\n\n\\(c\\)\n\n\\[d\\]");
        let mut math = Vec::new();
        visit(&tree, "math", &mut math);
        assert_eq!(math.len(), 4);
        assert_eq!(
            math.iter()
                .filter(|node| node.display_math == Some(true))
                .count(),
            2
        );
        assert_eq!(
            math.iter()
                .filter(|node| node.display_math == Some(false))
                .count(),
            2
        );
    }

    #[test]
    fn code_span_does_not_become_math() {
        let tree = parse_owned("`$not_math$`");
        let mut math = Vec::new();
        visit(&tree, "math", &mut math);
        assert!(math.is_empty());
    }

    #[test]
    fn raw_html_survives_as_literal_nodes() {
        let tree = parse_owned("<span>x</span>\n\n<div>y</div>");
        let mut inline = Vec::new();
        let mut block = Vec::new();
        visit(&tree, "html_inline", &mut inline);
        visit(&tree, "html_block", &mut block);
        assert!(!inline.is_empty());
        assert!(!block.is_empty());
        assert!(inline.iter().all(|node| node.literal.is_some()));
        assert!(block.iter().all(|node| node.literal.is_some()));
    }

    #[test]
    fn owned_tree_outlives_arena() {
        let tree = parse_owned("# title\n\ntext");
        assert_eq!(tree.kind, "document");
        assert!(!tree.children.is_empty());
    }

    #[test]
    fn ratex_baseline_renders_without_webview() {
        for formula in [
            r"x^2",
            r"\frac{a}{b}",
            r"\sqrt{x}",
            r"\sum_{n=1}^{\infty}",
            r"\int_0^1",
            r"\left(\frac{x}{y}\right)",
            r"\begin{matrix}a&b\\c&d\end{matrix}",
            r"\begin{cases}x,&x>0\\-x,&x<0\end{cases}",
            r"\mathbb{R}",
            r"\mathbf{x}",
            r"\operatorname{rank}(A)",
        ] {
            let png = render_math_png(formula)
                .unwrap_or_else(|error| panic!("formula {formula:?} failed: {error}"));
            assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        }
    }

    #[test]
    fn malformed_math_returns_error() {
        assert!(render_math_png(r"\frac{").is_err());
    }
}
