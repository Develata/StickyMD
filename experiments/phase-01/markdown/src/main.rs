//! Phase 1D spike: markdown (Comrak) + math (RaTeX) pipeline.
//!
//! plan_ref: docs/plan/06_markdown_semantics.md ; docs/plan/07_math_semantics.md
//!
//! Goal (Phase 1 prompt 1D): prove the frozen parsing/rendering direction:
//!   * Comrak owns ALL markdown semantics (CommonMark + GFM + math_dollars +
//!     math_latex). Comrak's `Arena` is converted to an owned `SpikeNode` tree
//!     (with sourcepos) and then the Arena is dropped — proving we do not leak
//!     the arena into the runtime model.
//!   * RaTeX owns ALL math semantics: parse -> layout -> DisplayList -> PNG.
//!     Both the happy path and a deliberate error case (`\frac{`) are exercised.
//!   * Benchmarks at 20/100/1024 KiB (>=20 repeats, median/p95/max) + memory.
//!
//! This is an experiment. Deliberately NOT production-shaped; deletable.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use comrak::nodes::{AstNode, NodeValue};
use comrak::{parse_document, Arena, Options};

// ── Allocation tracking (memory baseline) ──────────────────────────────────
static LIVE_BYTES: AtomicU64 = AtomicU64::new(0);
static PEAK_BYTES: AtomicU64 = AtomicU64::new(0);

struct CountingAlloc;
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let live = LIVE_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed)
                + layout.size() as u64;
            let mut peak = PEAK_BYTES.load(Ordering::Relaxed);
            while live > peak {
                match PEAK_BYTES.compare_exchange_weak(peak, live, Ordering::Relaxed, Ordering::Relaxed) {
                    Ok(_) => break,
                    Err(p2) => peak = p2,
                }
            }
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        LIVE_BYTES.fetch_sub(layout.size() as u64, Ordering::Relaxed);
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

// ── Owned projection of the Comrak arena ───────────────────────────────────
#[derive(Debug)]
struct SpikeNode {
    tag: String,
    sourcepos: String,
    literal: Option<String>,
    math_kind: Option<&'static str>,
    children: Vec<SpikeNode>,
}

// NOTE: Comrak hardcodes `dollar_math: true` for ALL math nodes, including the
// latex `\(...\)`/`\[...\]` forms (parser/inlines.rs:526). Therefore the flags
// only distinguish inline-vs-display; the dollar-vs-latex *style* is collapsed
// and can only be recovered from sourcepos + original text. See COMRAK_NOTES.md.
fn math_kind(m: &comrak::nodes::NodeMath) -> &'static str {
    if m.display_math {
        "display ($$ or \\[…\\])"
    } else {
        "inline ($ or \\(…\\))"
    }
}

fn node_literal(value: &NodeValue) -> (Option<String>, Option<&'static str>) {
    match value {
        NodeValue::Text(t) => (Some(t.to_string()), None),
        NodeValue::Math(m) => (Some(m.literal.clone()), Some(math_kind(m))),
        NodeValue::HtmlBlock(h) => (Some(h.literal.clone()), None),
        NodeValue::HtmlInline(s) => (Some(s.clone()), None),
        NodeValue::Code(c) => (Some(c.literal.clone()), None),
        NodeValue::CodeBlock(c) => (Some(c.literal.clone()), None),
        _ => (None, None),
    }
}

fn to_owned<'a>(node: &'a AstNode<'a>) -> SpikeNode {
    let data = node.data.borrow();
    let tag = data.value.xml_node_name().to_string();
    let sourcepos = data.sourcepos.to_string();
    let (literal, math_kind) = node_literal(&data.value);
    drop(data);
    let children = node.children().map(to_owned).collect();
    SpikeNode { tag, sourcepos, literal, math_kind, children }
}

fn count_nodes(n: &SpikeNode) -> usize {
    1 + n.children.iter().map(count_nodes).sum::<usize>()
}

/// Walk the owned tree collecting math literals + raw HTML literals with sourcepos.
fn collect(
    n: &SpikeNode,
    math: &mut Vec<(String, String, Option<&'static str>)>,
    html: &mut Vec<(String, String)>,
) {
    if n.tag == "math" {
        if let Some(l) = &n.literal {
            math.push((l.clone(), n.sourcepos.clone(), n.math_kind));
        }
    }
    if n.tag == "html_block" || n.tag == "html_inline" {
        if let Some(l) = &n.literal {
            html.push((l.clone(), n.sourcepos.clone()));
        }
    }
    for c in &n.children {
        collect(c, math, html);
    }
}

fn comrak_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.math_dollars = true;
    options.extension.math_latex = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options
}

// ── Stats helpers ──────────────────────────────────────────────────────────
fn stats(samples: &mut [f64]) -> (f64, f64, f64) {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = samples.len();
    let median = samples[n / 2];
    let p95 = samples[((n as f64) * 0.95).ceil() as usize - 1];
    let max = samples[n - 1];
    (median, p95, max)
}

fn bench_markdown_parse(md: &str, repeats: usize) -> (f64, f64, f64, usize) {
    let options = comrak_options();
    let mut samples = vec![0f64; repeats];
    let mut nodes = 0;
    for s in samples.iter_mut() {
        let t0 = Instant::now();
        let arena = Arena::new();
        let root = parse_document(&arena, md, &options);
        let owned = to_owned(root);
        nodes = count_nodes(&owned);
        std::hint::black_box(&owned);
        *s = t0.elapsed().as_secs_f64() * 1000.0;
        // arena dropped here -> proves we don't retain it
    }
    let (med, p95, max) = stats(&mut samples);
    (med, p95, max, nodes)
}

fn bench_math_render(expr: &str, repeats: usize) -> (f64, f64, f64) {
    let mut samples = vec![0f64; repeats];
    for s in samples.iter_mut() {
        let t0 = Instant::now();
        if let Ok(nodes) = ratex_parser::parse(expr) {
            let layout_box = ratex_layout::engine::layout(&nodes, &Default::default());
            let dl = ratex_layout::to_display::to_display_list(&layout_box);
            let png = ratex_render::render_to_png(&dl, &Default::default());
            std::hint::black_box(&png);
        }
        *s = t0.elapsed().as_secs_f64() * 1000.0;
    }
    stats(&mut samples)
}

fn render_math_once(expr: &str) -> Result<(usize, usize, f64, f64, usize), String> {
    let nodes = ratex_parser::parse(expr).map_err(|e| format!("{e:?}"))?;
    let layout_box = ratex_layout::engine::layout(&nodes, &Default::default());
    let dl = ratex_layout::to_display::to_display_list(&layout_box);
    let items = dl.items.len();
    let w = dl.width;
    let h = dl.height;
    let png = ratex_render::render_to_png(&dl, &Default::default())?;
    Ok((nodes.len(), items, w, h, png.len()))
}

fn main() {
    println!("=== Phase 1D markdown/math spike ===");

    // 1. Load + parse fixture.
    let fixture = std::fs::read_to_string("fixtures/all.md")
        .expect("fixtures/all.md must exist (run from experiments/phase-01/markdown)");
    println!("[fixture] bytes={}", fixture.len());

    let t0 = Instant::now();
    let tree = {
        let arena = Arena::new();
        let root = parse_document(&arena, &fixture, &comrak_options());
        to_owned(root)
        // arena + borrowed root dropped here; `tree` is fully owned
    };
    let parse_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let total_nodes = count_nodes(&tree);
    println!("[comrak] parse+project={parse_ms:.3}ms nodes={total_nodes} (arena dropped)");

    let mut math: Vec<(String, String, Option<&'static str>)> = Vec::new();
    let mut html: Vec<(String, String)> = Vec::new();
    collect(&tree, &mut math, &mut html);
    println!("[comrak] math_literals={} raw_html_literals={}", math.len(), html.len());
    for (i, (m, sp, kind)) in math.iter().enumerate() {
        println!("  math[{i}] [{}] @{sp} = {}", kind.unwrap_or("?"), m.trim());
    }
    for (i, (h, sp)) in html.iter().enumerate() {
        let first = h.lines().next().unwrap_or("").trim();
        println!("  html[{i}] @{sp} = {first}");
    }

    // 2. RaTeX happy path for each math literal + one error case.
    println!("[ratex] rendering {} math literals", math.len());
    let mut ok = 0usize;
    for (i, (m, _sp, _kind)) in math.iter().enumerate() {
        match render_math_once(m.trim()) {
            Ok((nodes, items, w, h, png_bytes)) => {
                ok += 1;
                println!(
                    "  ok[{i}] parse_nodes={nodes} display_items={items} box={w:.1}x{h:.1} png={png_bytes}B"
                );
            }
            Err(e) => println!("  FAIL[{i}] {} => {e}", m.trim()),
        }
    }
    // Deliberate error case: unbalanced brace.
    match ratex_parser::parse(r"\frac{") {
        Ok(_) => println!("[ratex] error-case \\frac{{ unexpectedly parsed"),
        Err(e) => println!("[ratex] error-case \\frac{{ -> Err({e:?}) (expected)"),
    }

    // 3. Benchmarks at 20/100/1024 KiB.
    let repeats = 24;
    for (label, target_kb) in [("20KiB", 20usize), ("100KiB", 100), ("1024KiB", 1024)] {
        let md = make_doc(target_kb * 1024);
        let live0 = LIVE_BYTES.load(Ordering::Relaxed);
        PEAK_BYTES.store(live0, Ordering::Relaxed);
        let (med, p95, max, nodes) = bench_markdown_parse(&md, repeats);
        let peak_delta = PEAK_BYTES.load(Ordering::Relaxed).saturating_sub(live0);
        println!(
            "[bench:{label}] doc={}B median={med:.2}ms p95={p95:.2}ms max={max:.2}ms nodes={nodes} peak_alloc≈{}KiB",
            md.len(),
            peak_delta / 1024
        );
    }

    // 4. Math render benchmark (a representative fraction expression).
    let expr = r"\int_{-\infty}^{\infty} e^{-x^2}\,dx = \sqrt{\pi}";
    let (med, p95, max) = bench_math_render(expr, repeats);
    println!("[bench:math-render] median={med:.2}ms p95={p95:.2}ms max={max:.2}ms");

    println!("[summary] math_ok={ok}/{} fixture_parse=OK arena_dropped=OK", math.len());
}

/// Build a synthetic markdown document of roughly `bytes` size, mixing prose,
/// GFM tables and both math delimiter styles.
fn make_doc(bytes: usize) -> String {
    let mut s = String::new();
    let mut i = 0;
    while s.len() < bytes {
        s.push_str(&format!("## Section {i}\n\n"));
        s.push_str("Some **bold** and *italic* prose with inline math $a^2+b^2=c^2$ and a link [x](https://example.com).\n\n");
        s.push_str("| col | val |\n| --- | --- |\n| a | 1 |\n| b | 2 |\n\n");
        s.push_str("$$\n\\sum_{n=1}^{\\infty} \\frac{1}{n^2} = \\frac{\\pi^2}{6}\n$$\n\n");
        s.push_str("- [x] done\n- [ ] open\n\n");
        i += 1;
    }
    s
}
