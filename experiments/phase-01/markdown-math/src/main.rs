use std::time::{Duration, Instant};

use spike_markdown_math::{parse_owned, render_math_png, visit};

fn percentile(samples: &mut [Duration], numerator: usize, denominator: usize) -> Duration {
    samples.sort_unstable();
    let rank = (samples.len() * numerator).div_ceil(denominator);
    let index = rank.saturating_sub(1).min(samples.len() - 1);
    samples[index]
}

fn summary(mut samples: Vec<Duration>) -> (Duration, Duration, Duration) {
    let median = percentile(&mut samples.clone(), 1, 2);
    let p95 = percentile(&mut samples, 95, 100);
    let max = samples[samples.len() - 1];
    (median, p95, max)
}

fn synthetic_document(target_bytes: usize) -> String {
    let block = "## 标题\n\n中文 English **bold** $a^2+b^2=c^2$.\n\n- [ ] task\n\n";
    let mut source = String::with_capacity(target_bytes + block.len());
    while source.len() < target_bytes {
        source.push_str(block);
    }
    source
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = include_str!("../fixtures/all.md");
    let tree = parse_owned(fixture);
    let mut math = Vec::new();
    let mut html_inline = Vec::new();
    let mut html_block = Vec::new();
    visit(&tree, "math", &mut math);
    visit(&tree, "html_inline", &mut html_inline);
    visit(&tree, "html_block", &mut html_block);
    println!(
        "fixture: math={} html_inline={} html_block={}",
        math.len(),
        html_inline.len(),
        html_block.len()
    );

    for (label, bytes) in [
        ("20KiB", 20 * 1024),
        ("100KiB", 100 * 1024),
        ("1MiB", 1024 * 1024),
    ] {
        let source = synthetic_document(bytes);
        for _ in 0..3 {
            std::hint::black_box(parse_owned(&source));
        }
        let mut samples = Vec::with_capacity(20);
        for _ in 0..20 {
            let started = Instant::now();
            std::hint::black_box(parse_owned(&source));
            samples.push(started.elapsed());
        }
        let (median, p95, max) = summary(samples);
        println!("{label}: median={median:?} p95={p95:?} max={max:?}");
    }

    let formula = r"\int_{-\infty}^{\infty}e^{-x^2}\,dx=\sqrt{\pi}";
    let mut samples = Vec::with_capacity(20);
    let mut png_bytes = 0usize;
    for _ in 0..20 {
        let started = Instant::now();
        png_bytes = render_math_png(formula)?.len();
        samples.push(started.elapsed());
    }
    let (median, p95, max) = summary(samples);
    println!("formula: median={median:?} p95={p95:?} max={max:?} png_bytes={png_bytes}");
    Ok(())
}
