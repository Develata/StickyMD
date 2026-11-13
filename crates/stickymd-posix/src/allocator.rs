//! A global allocator for a host process. Every allocation of at least [`THRESHOLD`] bytes is its
//! own anonymous mapping, unmapped when it is freed. Everything else goes to the system allocator.
//!
//! Why: the macOS allocator keeps a freed large block as a dirty, empty region that still counts
//! in the process footprint, and `malloc_zone_pressure_relief` did not return those regions
//! (plan 10, macOS gates). A frame buffer freed when the window hides must leave the footprint at
//! once.
//!
//! The spare: a presenter that allocates a new frame for every present and frees the previous one
//! (softbuffer 0.4.8 on macOS) would pay fresh page faults on every frame. So the most recently
//! freed mapped block is kept as the one spare (a newer one replaces it) and handed to the next
//! allocation of the same page count; zeroing a resident frame costs about what the system allocator's reuse costs. The host
//! turns reuse off while its window is hidden ([`set_block_reuse`]), which also unmaps the spare.
//!
//! Invariant of a mapped block: a block of layout size `s` is exactly the pages that cover
//! `[ptr, ptr + s)`, starting at the page-aligned `ptr`. `munmap(ptr, s)` therefore unmaps all of
//! it, because the kernel rounds the length up to whole pages.
//!
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates

use std::alloc::{GlobalAlloc, Layout, System};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};

/// Allocations at least this large are mapped directly. Frame and pane buffers are megabytes;
/// below this size the system allocator's reuse is worth more than returning pages.
pub const THRESHOLD: usize = 1 << 20;

/// The largest alignment a mapping promises: every POSIX page is at least 4 KiB.
const MAX_MAPPED_ALIGN: usize = 4096;

/// The one freed mapped block kept for reuse, or null. Its first word holds its page count.
static SPARE: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());
/// Whether a freed mapped block may become the spare.
static REUSE: AtomicBool = AtomicBool::new(true);

/// Turns keeping a spare block on or off. Turning it off unmaps the current spare, so every
/// mapped block freed from then on leaves the footprint at once.
pub fn set_block_reuse(enabled: bool) {
    REUSE.store(enabled, Ordering::SeqCst);
    if !enabled {
        release_spare();
    }
}

/// The global allocator; install it with `#[global_allocator]` in a host binary.
#[derive(Debug, Default, Clone, Copy)]
pub struct LargeMappedAllocator;

/// Whether a block of this size and alignment is a mapping. Without a known page size nothing is,
/// so a block's kind still depends only on its layout.
fn mapped(size: usize, align: usize) -> bool {
    size >= THRESHOLD && align <= MAX_MAPPED_ALIGN && page_size() != UNKNOWN_PAGE
}

const UNKNOWN_PAGE: usize = usize::MAX;

/// The system page size, or `UNKNOWN_PAGE` when the system cannot say; never a guess. The first
/// answer is published once and kept, so every caller sees the same value. `sysconf` does not
/// allocate, so calling it from inside the allocator cannot recurse.
fn page_size() -> usize {
    static PAGE: AtomicUsize = AtomicUsize::new(0);
    let cached = PAGE.load(Ordering::Acquire);
    if cached != 0 {
        return cached;
    }
    // SAFETY: `sysconf` reads a constant system value and has no preconditions.
    let page = match unsafe { libc::sysconf(libc::_SC_PAGESIZE) } {
        size if size > 0 && (size as usize).is_power_of_two() => size as usize,
        _ => UNKNOWN_PAGE,
    };
    match PAGE.compare_exchange(0, page, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => page,
        Err(published) => published,
    }
}

fn pages(size: usize) -> usize {
    size.div_ceil(page_size())
}

/// A fresh zeroed, readable and writable mapping of `size` bytes, or null.
fn map(size: usize) -> *mut u8 {
    // SAFETY: an anonymous private mapping with no address hint touches no existing memory.
    let address = unsafe {
        libc::mmap(
            ptr::null_mut(),
            size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANON,
            -1,
            0,
        )
    };
    if address == libc::MAP_FAILED {
        ptr::null_mut()
    } else {
        address.cast()
    }
}

/// # Safety
///
/// `address` and `size` must describe a mapped block (the module invariant) that nothing uses
/// any more.
unsafe fn unmap(address: *mut u8, size: usize) {
    // SAFETY: by the caller's contract the pages covering `[address, address + size)` are one of
    // our mappings and unused. `munmap` fails only on arguments the invariant rules out.
    let result = unsafe { libc::munmap(address.cast(), size) };
    // A failure means the invariant broke. An allocator must not unwind, so a debug build aborts;
    // a release build leaks the block rather than stop the note.
    if result != 0 && cfg!(debug_assertions) {
        std::process::abort();
    }
}

fn release_spare() {
    let spare = SPARE.swap(ptr::null_mut(), Ordering::SeqCst);
    if !spare.is_null() {
        // SAFETY: the swap made this block ours alone; `retire` wrote its page count first.
        unsafe {
            let spare_pages = spare.cast::<usize>().read();
            unmap(spare, spare_pages * page_size());
        }
    }
}

/// A mapped block of `size` bytes: the spare when it has the same page count, else a new mapping.
fn obtain(size: usize, zeroed: bool) -> *mut u8 {
    if !REUSE.load(Ordering::SeqCst) {
        return map(size);
    }
    let spare = SPARE.swap(ptr::null_mut(), Ordering::SeqCst);
    if spare.is_null() {
        return map(size);
    }
    // SAFETY: the swap made the spare ours alone; `retire` wrote its page count first.
    let spare_pages = unsafe { spare.cast::<usize>().read() };
    if spare_pages != pages(size) {
        // SAFETY: the spare satisfies the invariant for its own page count and is unused.
        unsafe { unmap(spare, spare_pages * page_size()) };
        return map(size);
    }
    if zeroed {
        // SAFETY: the spare covers at least `size` writable bytes.
        unsafe { ptr::write_bytes(spare, 0, size) };
    }
    spare
}

/// Gives up a mapped block of `size` bytes: with reuse on it becomes the spare, replacing (and
/// unmapping) the one before; with reuse off it is unmapped.
///
/// # Safety
///
/// `block` and `size` must describe a mapped block that nothing uses any more.
unsafe fn retire(block: *mut u8, size: usize) {
    if !REUSE.load(Ordering::SeqCst) {
        // SAFETY: by the caller's contract.
        unsafe { unmap(block, size) };
        return;
    }
    // SAFETY: the block is ours, page aligned and at least a page long.
    unsafe { block.cast::<usize>().write(pages(size)) };
    let replaced = SPARE.swap(block, Ordering::SeqCst);
    if !replaced.is_null() {
        // SAFETY: the swap made the replaced spare ours alone; `retire` wrote its page count.
        unsafe {
            let replaced_pages = replaced.cast::<usize>().read();
            unmap(replaced, replaced_pages * page_size());
        }
    }
    // Reuse may have been turned off after the check above, its sweep missing this block.
    if !REUSE.load(Ordering::SeqCst) {
        release_spare();
    }
}

// SAFETY: every block is either the system allocator's, handled by it with the same layout, or
// one of our mappings: page aligned (so any alignment up to 4 KiB holds), zero filled when asked,
// at least `size` bytes, owned by one holder at a time (a caller or the spare slot, handed over by
// atomic swap), and unmapped exactly once. Which kind a block is depends only on
// its layout, which the caller passes back unchanged.
unsafe impl GlobalAlloc for LargeMappedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if mapped(layout.size(), layout.align()) {
            obtain(layout.size(), false)
        } else {
            // SAFETY: the caller's layout contract passes through unchanged.
            unsafe { System.alloc(layout) }
        }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if mapped(layout.size(), layout.align()) {
            obtain(layout.size(), true)
        } else {
            // SAFETY: the caller's layout contract passes through unchanged.
            unsafe { System.alloc_zeroed(layout) }
        }
    }

    unsafe fn dealloc(&self, block: *mut u8, layout: Layout) {
        if mapped(layout.size(), layout.align()) {
            // SAFETY: a mapped layout means `alloc` or `realloc` mapped this block with this
            // size, and the caller gives it up.
            unsafe { retire(block, layout.size()) }
        } else {
            // SAFETY: the system allocator allocated it with this layout.
            unsafe { System.dealloc(block, layout) }
        }
    }

    unsafe fn realloc(&self, block: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let align = layout.align();
        let (old_mapped, new_mapped) = (mapped(layout.size(), align), mapped(new_size, align));
        if !old_mapped && !new_mapped {
            // SAFETY: both sides belong to the system allocator.
            return unsafe { System.realloc(block, layout, new_size) };
        }
        if old_mapped && new_mapped {
            let (old_pages, new_pages) = (pages(layout.size()), pages(new_size));
            if new_pages == old_pages {
                // The same pages cover both sizes, so the block keeps the invariant in place.
                return block;
            }
            if new_pages < old_pages {
                let page = page_size();
                // SAFETY: the tail pages belong to this block and lie past `new_size`, which the
                // caller keeps; what is left again satisfies the invariant.
                unsafe { unmap(block.add(new_pages * page), (old_pages - new_pages) * page) };
                return block;
            }
        }
        // SAFETY: `realloc`'s contract makes `new_size` with this alignment a valid layout.
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, align) };
        // SAFETY: `new_layout` has a non-zero size.
        let moved = unsafe { self.alloc(new_layout) };
        if !moved.is_null() {
            // SAFETY: both blocks are live, distinct and at least this long.
            unsafe { ptr::copy_nonoverlapping(block, moved, layout.size().min(new_size)) };
            // SAFETY: the old block, with its own layout, is no longer used.
            unsafe { self.dealloc(block, layout) };
        }
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALLOCATOR: LargeMappedAllocator = LargeMappedAllocator;

    /// The spare is process-wide; tests that allocate mapped blocks take turns.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn serial() -> std::sync::MutexGuard<'static, ()> {
        SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn layout(size: usize, align: usize) -> Layout {
        Layout::from_size_align(size, align).unwrap()
    }

    /// Fills `len` bytes from `block` with a pattern derived from the offset and `seed`.
    fn fill(block: *mut u8, len: usize, seed: u8) {
        // SAFETY: the tests only pass live blocks at least `len` long.
        let bytes = unsafe { std::slice::from_raw_parts_mut(block, len) };
        for (offset, byte) in bytes.iter_mut().enumerate() {
            *byte = (offset as u8).wrapping_mul(31).wrapping_add(seed);
        }
    }

    fn holds(block: *const u8, len: usize, seed: u8) -> bool {
        // SAFETY: the tests only pass live blocks at least `len` long.
        let bytes = unsafe { std::slice::from_raw_parts(block, len) };
        bytes
            .iter()
            .enumerate()
            .all(|(offset, byte)| *byte == (offset as u8).wrapping_mul(31).wrapping_add(seed))
    }

    fn footprint() -> u64 {
        let mut info = std::mem::MaybeUninit::<libc::rusage_info_v2>::zeroed();
        // SAFETY: the buffer is a `rusage_info_v2`, the flavor asked for.
        let result = unsafe {
            libc::proc_pid_rusage(
                std::process::id() as libc::c_int,
                libc::RUSAGE_INFO_V2,
                info.as_mut_ptr().cast(),
            )
        };
        assert_eq!(result, 0, "proc_pid_rusage failed");
        // SAFETY: `proc_pid_rusage` filled the structure.
        unsafe { info.assume_init() }.ri_phys_footprint
    }

    #[test]
    fn only_large_blocks_with_page_alignment_are_mapped() {
        assert!(!mapped(THRESHOLD - 1, 8));
        assert!(mapped(THRESHOLD, 8));
        assert!(mapped(THRESHOLD, MAX_MAPPED_ALIGN));
        assert!(!mapped(THRESHOLD, MAX_MAPPED_ALIGN * 2));
    }

    #[test]
    fn a_mapped_block_is_page_aligned_zeroed_and_usable() {
        let _serial = serial();
        let large = layout(THRESHOLD + 123, 8);
        // SAFETY: a non-zero layout; the block is freed with it.
        unsafe {
            let zeroed = ALLOCATOR.alloc_zeroed(large);
            assert!(!zeroed.is_null());
            assert_eq!(zeroed as usize % page_size(), 0);
            assert!(
                std::slice::from_raw_parts(zeroed, large.size())
                    .iter()
                    .all(|b| *b == 0)
            );
            fill(zeroed, large.size(), 7);
            assert!(holds(zeroed, large.size(), 7));
            ALLOCATOR.dealloc(zeroed, large);
        }
    }

    #[test]
    fn an_alignment_beyond_a_page_stays_with_the_system_allocator() {
        let _serial = serial();
        let large = layout(THRESHOLD, MAX_MAPPED_ALIGN * 4);
        // SAFETY: a non-zero layout; the block is freed with it.
        unsafe {
            let block = ALLOCATOR.alloc(large);
            assert!(!block.is_null());
            assert_eq!(block as usize % large.align(), 0);
            fill(block, large.size(), 3);
            assert!(holds(block, large.size(), 3));
            ALLOCATOR.dealloc(block, large);
        }
    }

    #[test]
    fn realloc_keeps_the_contents_across_every_kind_of_move() {
        let _serial = serial();
        let page = page_size();
        // (from, to): small to small, small to mapped, mapped growing past its pages, mapped
        // growing within its last page, mapped shrinking by whole pages, mapped back to small.
        let steps = [
            (1000, 4000),
            (4000, THRESHOLD + 1),
            (THRESHOLD + 1, THRESHOLD + 5 * page),
            (THRESHOLD + 5 * page, THRESHOLD + 5 * page + 1),
            (THRESHOLD + 5 * page + 1, THRESHOLD + page / 2),
            (THRESHOLD + page / 2, 512),
        ];
        // SAFETY: each step reallocates the live block with the layout it was last given.
        unsafe {
            let mut block = ALLOCATOR.alloc(layout(steps[0].0, 8));
            fill(block, steps[0].0, 11);
            for (from, to) in steps {
                block = ALLOCATOR.realloc(block, layout(from, 8), to);
                assert!(!block.is_null(), "realloc {from} -> {to}");
                assert!(
                    holds(block, from.min(to), 11),
                    "contents lost {from} -> {to}"
                );
                fill(block, to, 11);
            }
            ALLOCATOR.dealloc(block, layout(512, 8));
        }
    }

    #[test]
    fn a_shrink_in_place_keeps_the_address_and_a_growth_within_the_page_too() {
        let _serial = serial();
        let page = page_size();
        let big = layout(THRESHOLD + 8 * page, 8);
        // Both sizes end inside the same last page, two pages past the threshold.
        let (shrunk_size, grown_size) = (THRESHOLD + 2 * page - 100, THRESHOLD + 2 * page - 1);
        // SAFETY: the block is reallocated with the layout it was last given, then freed.
        unsafe {
            let block = ALLOCATOR.alloc(big);
            fill(block, big.size(), 5);
            let shrunk = ALLOCATOR.realloc(block, big, shrunk_size);
            assert_eq!(shrunk, block);
            assert!(holds(shrunk, shrunk_size, 5));
            let grown = ALLOCATOR.realloc(shrunk, layout(shrunk_size, 8), grown_size);
            assert_eq!(grown, block, "the growth stays inside the mapped last page");
            assert!(holds(grown, shrunk_size, 5));
            ALLOCATOR.dealloc(grown, layout(grown_size, 8));
        }
    }

    #[test]
    fn the_last_freed_block_is_reused_zeroed_for_the_same_page_count() {
        let _serial = serial();
        // Sweep a spare another test left, then start with reuse on.
        set_block_reuse(false);
        set_block_reuse(true);
        let frame = layout(1040 * 1360 * 4, 4);
        // SAFETY: each block is freed once with its layout.
        unsafe {
            let first = ALLOCATOR.alloc_zeroed(frame);
            fill(first, frame.size(), 9);
            ALLOCATOR.dealloc(first, frame);
            let second = ALLOCATOR.alloc_zeroed(frame);
            assert_eq!(second, first, "the spare comes back");
            assert!(
                std::slice::from_raw_parts(second, frame.size())
                    .iter()
                    .all(|b| *b == 0)
            );
            // Another page count does not take it: the spare is unmapped and a new block mapped.
            ALLOCATOR.dealloc(second, frame);
            let other = layout(frame.size() + 4 * page_size(), 4);
            let third = ALLOCATOR.alloc_zeroed(other);
            assert!(
                std::slice::from_raw_parts(third, other.size())
                    .iter()
                    .all(|b| *b == 0)
            );
            ALLOCATOR.dealloc(third, other);
        }
        set_block_reuse(false);
    }

    #[test]
    fn the_most_recently_freed_block_is_the_spare() {
        let _serial = serial();
        set_block_reuse(false);
        set_block_reuse(true);
        let small = layout(THRESHOLD, 8);
        let big = layout(THRESHOLD + 8 * page_size(), 8);
        // SAFETY: each block is freed once with its layout.
        unsafe {
            let first = ALLOCATOR.alloc(small);
            let second = ALLOCATOR.alloc(big);
            ALLOCATOR.dealloc(first, small);
            ALLOCATOR.dealloc(second, big);
            let again = ALLOCATOR.alloc(big);
            assert_eq!(again, second, "the later block replaced the earlier spare");
            ALLOCATOR.dealloc(again, big);
        }
        set_block_reuse(false);
    }

    /// The property the allocator exists for: a freed large block stops counting at once, and the
    /// spare stops counting when reuse is turned off.
    #[test]
    fn freed_blocks_leave_the_footprint() {
        const SIZE: usize = 64 << 20;
        let _serial = serial();
        let large = layout(SIZE, 8);
        set_block_reuse(false);
        let before = footprint();
        // SAFETY: a non-zero layout; the block is freed with it.
        unsafe {
            let block = ALLOCATOR.alloc(large);
            assert!(!block.is_null());
            fill(block, SIZE, 1);
            assert!(
                footprint() >= before + SIZE as u64 / 2,
                "touched pages should count"
            );
            ALLOCATOR.dealloc(block, large);
        }
        // Other tests may allocate meanwhile; a 64 MiB block must still be clearly gone.
        assert!(
            footprint() < before + SIZE as u64 / 4,
            "freed without reuse"
        );
        set_block_reuse(true);
        // SAFETY: as above.
        unsafe {
            let block = ALLOCATOR.alloc(large);
            fill(block, SIZE, 2);
            ALLOCATOR.dealloc(block, large);
        }
        assert!(
            footprint() >= before + SIZE as u64 / 2,
            "the spare still counts"
        );
        set_block_reuse(false);
        assert!(
            footprint() < before + SIZE as u64 / 4,
            "turning reuse off unmaps the spare"
        );
    }
}
