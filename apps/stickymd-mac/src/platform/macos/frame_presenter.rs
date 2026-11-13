//! The frame presenter: shows the shell's finished canvas through IOSurfaces set as a sublayer's
//! contents (`06` frame presentation, `09` macOS mapping; USER approved plan B 2026-11-14).
//! Core Animation uses an IOSurface as it is, so a present costs one conversion into the surface
//! and no copy inside Core Animation, and no frame-sized allocation per present.
//!
//! Buffers: at most `CAPACITY` surfaces. A frame is written only into a surface that is neither
//! the current contents nor still read by the compositor (`IOSurfaceIsInUse`); when there is
//! none and the pool is full, the frame is deferred and the shell draws it again shortly. A new
//! size first builds and fills a new surface; the old contents stay until it is ready. Every
//! surface carries the device RGB colour space softbuffer gave its images, so colours are as
//! before.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#frame-presentation
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping

use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, Ordering};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};
use objc2_foundation::{NSPoint, NSRect, NSString};
use stickymd_desktop::host::PresentOutcome;
use tiny_skia::Pixmap;
use winit::window::Window;

use super::window::ns_view;

/// Surfaces kept at most: the contents, one the compositor may still read, one to draw into.
pub const CAPACITY: usize = 3;
/// `kIOSurfaceLockAvoidSync`: no GPU is writing these surfaces.
const LOCK_AVOID_SYNC: u32 = 2;
/// `kCVPixelFormatType_32BGRA`.
const BGRA: u32 = 0x4247_5241;

#[link(name = "IOSurface", kind = "framework")]
unsafe extern "C" {
    fn IOSurfaceCreate(properties: *const c_void) -> *mut c_void;
    fn IOSurfaceLock(surface: *mut c_void, options: u32, seed: *mut u32) -> i32;
    fn IOSurfaceUnlock(surface: *mut c_void, options: u32, seed: *mut u32) -> i32;
    fn IOSurfaceGetBaseAddress(surface: *mut c_void) -> *mut u8;
    fn IOSurfaceGetBytesPerRow(surface: *mut c_void) -> usize;
    fn IOSurfaceIsInUse(surface: *mut c_void) -> u8;
    fn IOSurfaceSetValue(surface: *mut c_void, key: *const c_void, value: *const c_void);
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGColorSpaceCreateDeviceRGB() -> *mut c_void;
    fn CGColorSpaceCopyPropertyList(space: *mut c_void) -> *mut c_void;
    fn CGColorSpaceRelease(space: *mut c_void);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(object: *const c_void);
}

/// How the presenter has fared, for the opt-in latency diagnostic.
static PRESENTED: AtomicU64 = AtomicU64::new(0);
static DEFERRED: AtomicU64 = AtomicU64::new(0);
static CREATED: AtomicU64 = AtomicU64::new(0);
static FAILED: AtomicU64 = AtomicU64::new(0);

/// `[presented, deferred, surfaces created, failed]` since launch.
pub fn counters() -> [u64; 4] {
    [&PRESENTED, &DEFERRED, &CREATED, &FAILED].map(|counter| counter.load(Ordering::Relaxed))
}

/// One IOSurface this presenter owns (+1), released once on drop. Never null.
struct Surface(NonNull<c_void>);

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: created by `IOSurfaceCreate` (+1) and released once here; the layer holds its
        // own reference while the surface is its contents.
        unsafe { CFRelease(self.0.as_ptr()) };
    }
}

impl Surface {
    /// A BGRA surface of `width` × `height` with the device RGB colour space, or `None`.
    fn new(width: u32, height: u32) -> Option<Self> {
        let number = |value: u32| -> Retained<AnyObject> {
            // SAFETY: a class method returning an autoreleased NSNumber, retained here.
            unsafe { msg_send![class!(NSNumber), numberWithUnsignedInt: value] }
        };
        let keys = [
            NSString::from_str("IOSurfaceWidth"),
            NSString::from_str("IOSurfaceHeight"),
            NSString::from_str("IOSurfaceBytesPerElement"),
            NSString::from_str("IOSurfacePixelFormat"),
        ];
        let values = [number(width), number(height), number(4), number(BGRA)];
        let key_pointers = keys
            .iter()
            .map(|key| &**key as *const NSString)
            .collect::<Vec<_>>();
        let value_pointers = values
            .iter()
            .map(|value| &**value as *const AnyObject)
            .collect::<Vec<_>>();
        // SAFETY: arrays of live objects with the stated count; the dictionary copies them, and
        // NSDictionary is toll-free bridged to the CFDictionary IOSurfaceCreate takes. A null
        // result never becomes a `Surface`.
        let surface = unsafe {
            let dictionary: Retained<AnyObject> = msg_send![
                class!(NSDictionary),
                dictionaryWithObjects: value_pointers.as_ptr(),
                forKeys: key_pointers.as_ptr(),
                count: keys.len()
            ];
            NonNull::new(IOSurfaceCreate((&*dictionary as *const AnyObject).cast()))?
        };
        let surface = Self(surface);
        surface.set_device_rgb();
        CREATED.fetch_add(1, Ordering::Relaxed);
        Some(surface)
    }

    fn set_device_rgb(&self) {
        // SAFETY: Create/Copy results are released once; the surface copies the value it is set.
        unsafe {
            let space = CGColorSpaceCreateDeviceRGB();
            if space.is_null() {
                return;
            }
            let list = CGColorSpaceCopyPropertyList(space);
            if !list.is_null() {
                let key = NSString::from_str("IOSurfaceColorSpace");
                IOSurfaceSetValue(self.0.as_ptr(), (&*key as *const NSString).cast(), list);
                CFRelease(list);
            }
            CGColorSpaceRelease(space);
        }
    }

    fn in_use(&self) -> bool {
        // SAFETY: a live surface.
        unsafe { IOSurfaceIsInUse(self.0.as_ptr()) != 0 }
    }

    /// Writes the canvas (premultiplied RGBA, opaque) as BGRA, row by row at the surface's stride.
    fn write(&self, frame: &Pixmap) -> Result<(), String> {
        let (width, height) = (frame.width() as usize, frame.height() as usize);
        // SAFETY: a live surface; the lock is paired with the unlock below on every path.
        if unsafe { IOSurfaceLock(self.0.as_ptr(), LOCK_AVOID_SYNC, std::ptr::null_mut()) } != 0 {
            return Err("IOSurfaceLock failed".into());
        }
        // SAFETY: while locked, the base address maps `stride * height` writable bytes, and
        // `stride >= width * 4` for a BGRA surface of this width.
        let written = unsafe {
            let base = IOSurfaceGetBaseAddress(self.0.as_ptr());
            let stride = IOSurfaceGetBytesPerRow(self.0.as_ptr());
            if base.is_null() || stride < width * 4 {
                Err("IOSurface has no mapping for this frame".to_owned())
            } else {
                let row_bytes = width * 4;
                for (row, source) in frame
                    .data()
                    .chunks_exact(row_bytes)
                    .take(height)
                    .enumerate()
                {
                    let target = std::slice::from_raw_parts_mut(base.add(row * stride), row_bytes);
                    rgba_to_bgra(source, target);
                }
                Ok(())
            }
        };
        // SAFETY: releases the lock taken above.
        if unsafe { IOSurfaceUnlock(self.0.as_ptr(), LOCK_AVOID_SYNC, std::ptr::null_mut()) } != 0 {
            return Err("IOSurfaceUnlock failed".into());
        }
        written
    }
}

/// Swaps red and blue of each 4-byte pixel. Pre: equal lengths, a multiple of 4.
fn rgba_to_bgra(source: &[u8], target: &mut [u8]) {
    debug_assert_eq!(source.len(), target.len());
    for (to, from) in target.chunks_exact_mut(4).zip(source.chunks_exact(4)) {
        let pixel = u32::from_le_bytes([from[0], from[1], from[2], from[3]]);
        let swapped = (pixel & 0xff00_ff00) | ((pixel & 0xff) << 16) | ((pixel >> 16) & 0xff);
        to.copy_from_slice(&swapped.to_le_bytes());
    }
}

/// Where the next frame goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
    Write(usize),
    Create,
    Defer,
}

/// Pre: `in_use[i]` says whether surface `i` is still read by the compositor; `front` is the
/// current contents. Post: never the front, never one in use; a new surface only below `capacity`.
fn choose(in_use: &[bool], front: Option<usize>, capacity: usize) -> Choice {
    if let Some(free) = (0..in_use.len()).find(|&i| Some(i) != front && !in_use[i]) {
        Choice::Write(free)
    } else if in_use.len() < capacity {
        Choice::Create
    } else {
        Choice::Defer
    }
}

/// The frame layer and its surfaces.
#[derive(Default)]
pub struct FramePresenter {
    layer: Option<Retained<AnyObject>>,
    pool: Vec<Surface>,
    /// Index of the surface that is the layer's contents.
    front: Option<usize>,
    size: (u32, u32),
}

impl FramePresenter {
    fn layer(&mut self, window: &Window) -> Option<Retained<AnyObject>> {
        if self.layer.is_none() {
            let view = ns_view(window)?;
            // SAFETY: AppKit and Core Animation calls on live objects, on the main thread.
            unsafe {
                let root: *mut AnyObject = msg_send![view, layer];
                if root.is_null() {
                    return None;
                }
                let layer: Retained<AnyObject> = msg_send![class!(CALayer), new];
                // The canvas's origin is its top-left corner, as softbuffer's layer has it.
                let _: () = msg_send![&*layer, setAnchorPoint: NSPoint::new(0.0, 0.0)];
                let _: () = msg_send![&*layer, setGeometryFlipped: Bool::YES];
                let gravity = NSString::from_str("topLeft");
                let _: () = msg_send![&*layer, setContentsGravity: &*gravity];
                let _: () = msg_send![&*layer, setOpaque: Bool::YES];
                let _: () = msg_send![root, addSublayer: &*layer];
                self.layer = Some(layer);
            }
        }
        self.layer.clone()
    }

    /// Shows `frame` as the window's contents, or defers it, or reports why it cannot.
    pub fn present(&mut self, window: &Window, frame: &Pixmap) -> PresentOutcome {
        let outcome = self.try_present(window, frame);
        match &outcome {
            PresentOutcome::Presented => PRESENTED.fetch_add(1, Ordering::Relaxed),
            PresentOutcome::Deferred => DEFERRED.fetch_add(1, Ordering::Relaxed),
            PresentOutcome::Failed(_) => FAILED.fetch_add(1, Ordering::Relaxed),
        };
        outcome
    }

    fn try_present(&mut self, window: &Window, frame: &Pixmap) -> PresentOutcome {
        let Some(layer) = self.layer(window) else {
            return PresentOutcome::Failed("the window has no layer for frames".into());
        };
        let size = (frame.width(), frame.height());
        if size != self.size {
            // A new size: build and fill the new surface first; the old contents stay meanwhile.
            let Some(fresh) = Surface::new(size.0, size.1) else {
                return PresentOutcome::Failed("IOSurfaceCreate failed".into());
            };
            if let Err(error) = fresh.write(frame) {
                return PresentOutcome::Failed(error);
            }
            show(window, &layer, &fresh);
            self.pool = vec![fresh];
            self.front = Some(0);
            self.size = size;
            return PresentOutcome::Presented;
        }
        let in_use = self.pool.iter().map(Surface::in_use).collect::<Vec<_>>();
        let index = match choose(&in_use, self.front, CAPACITY) {
            Choice::Write(index) => index,
            Choice::Create => {
                let Some(surface) = Surface::new(size.0, size.1) else {
                    return PresentOutcome::Failed("IOSurfaceCreate failed".into());
                };
                self.pool.push(surface);
                self.pool.len() - 1
            }
            Choice::Defer => return PresentOutcome::Deferred,
        };
        if let Err(error) = self.pool[index].write(frame) {
            return PresentOutcome::Failed(error);
        }
        show(window, &layer, &self.pool[index]);
        self.front = Some(index);
        PresentOutcome::Presented
    }

    /// While hidden: the layer shows nothing and no surface is kept.
    pub fn release(&mut self) {
        if let Some(layer) = &self.layer {
            // SAFETY: clearing a live layer's contents on the main thread, without animation.
            unsafe {
                let _: () = msg_send![class!(CATransaction), begin];
                let _: () = msg_send![class!(CATransaction), setDisableActions: Bool::YES];
                let none: *mut AnyObject = std::ptr::null_mut();
                let _: () = msg_send![&**layer, setContents: none];
                let _: () = msg_send![class!(CATransaction), commit];
            }
        }
        self.pool.clear();
        self.front = None;
        self.size = (0, 0);
    }
}

/// Makes `surface` the layer's contents, with the layer matching its window's root layer.
fn show(window: &Window, layer: &AnyObject, surface: &Surface) {
    let Some(view) = ns_view(window) else {
        return;
    };
    // SAFETY: live layers and surface on the main thread. `CALayer.contents` accepts an
    // IOSurfaceRef (CALayer.h), and keeps its own strong reference. The transaction stops the
    // implicit fade.
    unsafe {
        let root: *mut AnyObject = msg_send![view, layer];
        let _: () = msg_send![class!(CATransaction), begin];
        let _: () = msg_send![class!(CATransaction), setDisableActions: Bool::YES];
        if !root.is_null() {
            let bounds: NSRect = msg_send![root, bounds];
            let scale: f64 = msg_send![root, contentsScale];
            let _: () = msg_send![layer, setFrame: bounds];
            let _: () = msg_send![layer, setContentsScale: scale];
        }
        let contents = surface.0.as_ptr().cast::<AnyObject>();
        let _: () = msg_send![layer, setContents: contents];
        let _: () = msg_send![class!(CATransaction), commit];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_next_buffer_is_never_the_front_nor_one_in_use() {
        assert_eq!(choose(&[], None, 3), Choice::Create);
        assert_eq!(choose(&[false], Some(0), 3), Choice::Create);
        assert_eq!(choose(&[false, false], Some(0), 3), Choice::Write(1));
        assert_eq!(choose(&[false, true], Some(0), 3), Choice::Create);
        assert_eq!(choose(&[false, true, false], Some(0), 3), Choice::Write(2));
        assert_eq!(choose(&[true, false, true], Some(1), 3), Choice::Defer);
        assert_eq!(choose(&[false, true, true], Some(0), 3), Choice::Defer);
    }

    #[test]
    fn red_and_blue_swap_and_the_rest_stays() {
        let mut target = [0u8; 8];
        rgba_to_bgra(&[1, 2, 3, 4, 250, 128, 7, 255], &mut target);
        assert_eq!(target, [3, 2, 1, 4, 7, 128, 250, 255]);
    }

    #[test]
    fn a_surface_takes_the_frame_as_bgra_at_its_stride() {
        let mut frame = Pixmap::new(3, 2).unwrap();
        for (index, byte) in frame.data_mut().iter_mut().enumerate() {
            *byte = index as u8;
        }
        let surface = Surface::new(3, 2).expect("an IOSurface");
        surface.write(&frame).unwrap();
        // SAFETY: lock, read the mapped rows, unlock.
        unsafe {
            assert_eq!(
                IOSurfaceLock(surface.0.as_ptr(), 1, std::ptr::null_mut()),
                0
            );
            let base = IOSurfaceGetBaseAddress(surface.0.as_ptr());
            let stride = IOSurfaceGetBytesPerRow(surface.0.as_ptr());
            for row in 0..2usize {
                let read = std::slice::from_raw_parts(base.add(row * stride), 12);
                let mut expected = [0u8; 12];
                rgba_to_bgra(&frame.data()[row * 12..row * 12 + 12], &mut expected);
                assert_eq!(read, expected, "row {row}");
            }
            assert_eq!(
                IOSurfaceUnlock(surface.0.as_ptr(), 1, std::ptr::null_mut()),
                0
            );
        }
    }

    #[test]
    fn a_surface_that_cannot_be_made_is_none_not_a_crash() {
        assert!(Surface::new(0, 0).is_none());
    }
}
