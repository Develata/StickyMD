//! What the large-block allocator costs a frame. softbuffer 0.4.8's CoreGraphics backend
//! allocates a zeroed frame on every present, fills it, and frees the previous one when the layer
//! takes the new image. This times that lifecycle under the system allocator and under
//! `LargeMappedAllocator` with its spare (`spare`, the visible window) and without it (`unmap`, as
//! while hidden), at the default and the Split window sizes (scale 2).
//!
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates

#[cfg(target_os = "macos")]
fn main() {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::time::{Duration, Instant};

    use stickymd_posix::allocator::LargeMappedAllocator;

    const FRAMES: usize = 240;
    const WARMUP: usize = 20;

    fn run(allocator: &dyn GlobalAlloc, width: usize, height: usize) -> Vec<Duration> {
        let layout = Layout::array::<u32>(width * height).unwrap();
        let mut previous: Option<*mut u8> = None;
        let mut samples = Vec::with_capacity(FRAMES);
        for frame in 0..WARMUP + FRAMES {
            let started = Instant::now();
            // SAFETY: a non-zero layout; each block is freed once, with the same layout.
            unsafe {
                let block = allocator.alloc_zeroed(layout);
                assert!(!block.is_null());
                let pixels = std::slice::from_raw_parts_mut(block.cast::<u32>(), width * height);
                for (index, pixel) in pixels.iter_mut().enumerate() {
                    *pixel = (index as u32).wrapping_add(frame as u32) | 0xff00_0000;
                }
                std::hint::black_box(&pixels[width * height / 2]);
                if let Some(old) = previous.replace(block) {
                    allocator.dealloc(old, layout);
                }
            }
            if frame >= WARMUP {
                samples.push(started.elapsed());
            }
        }
        if let Some(old) = previous {
            // SAFETY: the last block, with its layout.
            unsafe { allocator.dealloc(old, layout) };
        }
        samples
    }

    fn percentile(sorted: &[Duration], percent: usize) -> Duration {
        // Nearest rank, as plan 10's other cohorts.
        sorted[(sorted.len() * percent).div_ceil(100).max(1) - 1]
    }

    println!(
        "frame lifecycle (alloc_zeroed + fill + free previous), {FRAMES} frames after {WARMUP} warm-up"
    );
    for (label, width, height) in [
        ("default 1040x1360", 1040, 1360),
        ("split 1800x1360", 1800, 1360),
    ] {
        for (name, allocator, reuse) in [
            ("system", &System as &dyn GlobalAlloc, false),
            ("spare", &LargeMappedAllocator as &dyn GlobalAlloc, true),
            ("unmap", &LargeMappedAllocator as &dyn GlobalAlloc, false),
        ] {
            stickymd_posix::allocator::set_block_reuse(reuse);
            let mut samples = run(allocator, width, height);
            samples.sort_unstable();
            println!(
                "{label:<18} {name:<7} p50 {:>7.3} ms  p95 {:>7.3} ms  max {:>7.3} ms",
                percentile(&samples, 50).as_secs_f64() * 1e3,
                percentile(&samples, 95).as_secs_f64() * 1e3,
                samples[samples.len() - 1].as_secs_f64() * 1e3,
            );
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {}
