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
    /// A BGRA surface of `width` × `height` with the device RGB colour space, or why not. A
    /// surface without that colour space is never used.
    fn new(width: u32, height: u32) -> Result<Self, String> {
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
            NonNull::new(IOSurfaceCreate((&*dictionary as *const AnyObject).cast()))
                .ok_or("IOSurfaceCreate failed")?
        };
        // On error the new surface is released here; the caller keeps showing the old one.
        let surface = Self(surface);
        surface.set_device_rgb()?;
        CREATED.fetch_add(1, Ordering::Relaxed);
        Ok(surface)
    }

    fn set_device_rgb(&self) -> Result<(), String> {
        // SAFETY: Create/Copy results are released once each; the surface keeps its own copy of
        // the value it is set.
        unsafe {
            let space = CGColorSpaceCreateDeviceRGB();
            if space.is_null() {
                return Err("CGColorSpaceCreateDeviceRGB failed".into());
            }
            let list = CGColorSpaceCopyPropertyList(space);
            CGColorSpaceRelease(space);
            if list.is_null() {
                return Err("the device RGB colour space has no property list".into());
            }
            let key = NSString::from_str("IOSurfaceColorSpace");
            IOSurfaceSetValue(self.0.as_ptr(), (&*key as *const NSString).cast(), list);
            CFRelease(list);
        }
        Ok(())
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
    /// The frame layer, made once as a sublayer of `root`.
    fn layer(&mut self, root: &AnyObject) -> Retained<AnyObject> {
        if self.layer.is_none() {
            // SAFETY: Core Animation calls on live objects, on the main thread.
            unsafe {
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
        self.layer.clone().expect("made above")
    }

    /// Shows `frame` as the window's contents, or defers it, or reports why it cannot.
    pub fn present(&mut self, window: &Window, frame: &Pixmap) -> PresentOutcome {
        let outcome = match root_layer(window) {
            Some(root) => self.present_into(&root, frame),
            None => PresentOutcome::Failed("the window has no layer for frames".into()),
        };
        match &outcome {
            PresentOutcome::Presented => PRESENTED.fetch_add(1, Ordering::Relaxed),
            PresentOutcome::Deferred => DEFERRED.fetch_add(1, Ordering::Relaxed),
            PresentOutcome::Failed(_) => FAILED.fetch_add(1, Ordering::Relaxed),
        };
        outcome
    }

    /// `present` into a sublayer of the window's root layer `root`.
    fn present_into(&mut self, root: &AnyObject, frame: &Pixmap) -> PresentOutcome {
        let layer = self.layer(root);
        let size = (frame.width(), frame.height());
        if size != self.size {
            // A new size: build and fill the new surface first; the old contents stay meanwhile.
            let fresh = match Surface::new(size.0, size.1) {
                Ok(fresh) => fresh,
                Err(error) => return PresentOutcome::Failed(error),
            };
            if let Err(error) = fresh.write(frame) {
                return PresentOutcome::Failed(error);
            }
            show(root, &layer, &fresh);
            self.pool = vec![fresh];
            self.front = Some(0);
            self.size = size;
            return PresentOutcome::Presented;
        }
        let in_use = self.pool.iter().map(Surface::in_use).collect::<Vec<_>>();
        let index = match choose(&in_use, self.front, CAPACITY) {
            Choice::Write(index) => index,
            Choice::Create => {
                match Surface::new(size.0, size.1) {
                    Ok(surface) => self.pool.push(surface),
                    Err(error) => return PresentOutcome::Failed(error),
                }
                self.pool.len() - 1
            }
            Choice::Defer => return PresentOutcome::Deferred,
        };
        if let Err(error) = self.pool[index].write(frame) {
            return PresentOutcome::Failed(error);
        }
        show(root, &layer, &self.pool[index]);
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

/// The window view's root layer, which the frame layer sits in.
fn root_layer(window: &Window) -> Option<Retained<AnyObject>> {
    let view = ns_view(window)?;
    // SAFETY: the view belongs to the live winit window, on the main thread; `layer` is a
    // getter whose result is retained here, and nil maps to `None`.
    unsafe { msg_send![view, layer] }
}

/// Makes `surface` the layer's contents, with the layer matching the root layer.
fn show(root: &AnyObject, layer: &AnyObject, surface: &Surface) {
    // SAFETY: live layers and surface on the main thread. `CALayer.contents` accepts an
    // IOSurfaceRef (CALayer.h), and keeps its own strong reference. The transaction stops the
    // implicit fade.
    unsafe {
        let _: () = msg_send![class!(CATransaction), begin];
        let _: () = msg_send![class!(CATransaction), setDisableActions: Bool::YES];
        let bounds: NSRect = msg_send![root, bounds];
        let scale: f64 = msg_send![root, contentsScale];
        let _: () = msg_send![layer, setFrame: bounds];
        let _: () = msg_send![layer, setContentsScale: scale];
        let contents = surface.0.as_ptr().cast::<AnyObject>();
        let _: () = msg_send![layer, setContents: contents];
        let _: () = msg_send![class!(CATransaction), commit];
    }
}

#[cfg(test)]
/// The SAFETY notes above say "on the main thread": that is the window's path. These tests
/// drive the same code on a standalone root layer that only the test thread creates and uses,
/// which Core Animation allows; the thread that owns the layers is what matters.
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
    fn a_surface_that_cannot_be_made_is_an_error_not_a_crash() {
        assert_eq!(
            Surface::new(0, 0).err().as_deref(),
            Some("IOSurfaceCreate failed")
        );
    }

    #[link(name = "IOSurface", kind = "framework")]
    unsafe extern "C" {
        fn IOSurfaceIncrementUseCount(surface: *mut c_void);
        fn IOSurfaceDecrementUseCount(surface: *mut c_void);
        fn IOSurfaceCopyValue(surface: *mut c_void, key: *const c_void) -> *const c_void;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGColorSpaceCreateWithPropertyList(list: *const c_void) -> *mut c_void;
        fn CGColorSpaceCreateWithName(name: *const c_void) -> *mut c_void;
        fn CGColorCreate(space: *mut c_void, components: *const f64) -> *mut c_void;
        fn CGColorCreateCopyByMatchingToColorSpace(
            space: *mut c_void,
            intent: i32,
            color: *mut c_void,
            options: *const c_void,
        ) -> *mut c_void;
        fn CGColorGetComponents(color: *mut c_void) -> *const f64;
        fn CGColorRelease(color: *mut c_void);
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRetain(object: *const c_void) -> *const c_void;
    }

    fn contents(layer: &AnyObject) -> *mut c_void {
        // SAFETY: a getter on a live layer; the pointer is only compared.
        let contents: *mut AnyObject = unsafe { msg_send![layer, contents] };
        contents.cast()
    }

    fn surface_of(presenter: &FramePresenter, index: usize) -> *mut c_void {
        presenter.pool[index].0.as_ptr()
    }

    /// A frame whose every pixel is `(tag, 255 - tag, tag / 2, 255)`, so each frame is told apart.
    fn frame(width: u32, height: u32, tag: u8) -> Pixmap {
        let mut frame = Pixmap::new(width, height).unwrap();
        for pixel in frame.data_mut().chunks_exact_mut(4) {
            pixel.copy_from_slice(&[tag, 255 - tag, tag / 2, 255]);
        }
        frame
    }

    /// Which frame a surface holds: the tag of its first pixel, read back as BGRA.
    fn tag_of(surface: *mut c_void) -> u8 {
        // SAFETY: a live surface, locked read-only (1) around the read of its first pixel.
        unsafe {
            assert_eq!(IOSurfaceLock(surface, 1, std::ptr::null_mut()), 0);
            let pixel = std::slice::from_raw_parts(IOSurfaceGetBaseAddress(surface), 4).to_vec();
            assert_eq!(IOSurfaceUnlock(surface, 1, std::ptr::null_mut()), 0);
            assert_eq!(pixel[1], 255 - pixel[2], "a whole frame, not stale bytes");
            pixel[2]
        }
    }

    /// The compositor's hold on a surface, as the test plays it: a reference and a use count,
    /// both given back on drop, also when an assertion fails.
    struct Held(*mut c_void);

    impl Held {
        fn new(surface: *mut c_void) -> Self {
            // SAFETY: a live surface; +1 reference and +1 use count, both undone in `drop`.
            unsafe {
                CFRetain(surface);
                IOSurfaceIncrementUseCount(surface);
            }
            Self(surface)
        }
    }

    impl Drop for Held {
        fn drop(&mut self) {
            // SAFETY: undoes `new`; the test's reference keeps the surface alive until here.
            unsafe {
                IOSurfaceDecrementUseCount(self.0);
                CFRelease(self.0);
            }
        }
    }

    /// The whole presenter on a standalone root layer owned by this test thread: no window, no
    /// compositor. Every frame carries its own tag, read back from the surfaces, so a frame
    /// written into the wrong surface, or not written, fails the test.
    #[test]
    fn a_presenter_writes_only_free_surfaces_defers_when_none_is_free_and_lets_all_go() {
        // SAFETY: Core Animation objects created and used on this thread only.
        let root: Retained<AnyObject> = unsafe {
            let root: Retained<AnyObject> = msg_send![class!(CALayer), new];
            let bounds = NSRect::new(
                NSPoint::new(0.0, 0.0),
                objc2_foundation::NSSize::new(20.0, 15.0),
            );
            let _: () = msg_send![&*root, setBounds: bounds];
            let _: () = msg_send![&*root, setContentsScale: 2.0f64];
            root
        };
        let mut presenter = FramePresenter::default();

        assert_eq!(
            presenter.present_into(&root, &frame(4, 3, 1)),
            PresentOutcome::Presented
        );
        let layer = presenter.layer.clone().unwrap();
        assert_eq!(contents(&layer), surface_of(&presenter, 0));
        assert_eq!(tag_of(surface_of(&presenter, 0)), 1);
        // SAFETY: getters on the live frame layer.
        let (frame_rect, scale): (NSRect, f64) =
            unsafe { (msg_send![&*layer, frame], msg_send![&*layer, contentsScale]) };
        assert_eq!(
            (frame_rect.size.width, frame_rect.size.height, scale),
            (20.0, 15.0, 2.0)
        );

        assert_eq!(
            presenter.present_into(&root, &frame(4, 3, 2)),
            PresentOutcome::Presented
        );
        assert_eq!((presenter.pool.len(), presenter.front), (2, Some(1)));
        assert_eq!(contents(&layer), surface_of(&presenter, 1));
        assert_eq!(tag_of(surface_of(&presenter, 1)), 2);
        assert_eq!(
            tag_of(surface_of(&presenter, 0)),
            1,
            "the old front is not rewritten"
        );

        // Surface 0 is read by the compositor: the frame goes into a third one, 0 untouched.
        let busy = Held::new(surface_of(&presenter, 0));
        assert_eq!(
            presenter.present_into(&root, &frame(4, 3, 3)),
            PresentOutcome::Presented
        );
        assert_eq!((presenter.pool.len(), presenter.front), (3, Some(2)));
        assert_eq!(tag_of(surface_of(&presenter, 2)), 3);
        assert_eq!(tag_of(busy.0), 1, "a busy surface is never written");

        // Surfaces 0 and 1 busy, 2 shown, the pool full: deferred, nothing written or shown.
        let also_busy = Held::new(surface_of(&presenter, 1));
        assert_eq!(
            presenter.present_into(&root, &frame(4, 3, 4)),
            PresentOutcome::Deferred
        );
        assert_eq!(presenter.pool.len(), CAPACITY);
        assert_eq!(contents(&layer), surface_of(&presenter, 2));
        let tags = (0..CAPACITY)
            .map(|i| tag_of(surface_of(&presenter, i)))
            .collect::<Vec<_>>();
        assert_eq!(tags, [1, 2, 3]);

        // Surface 0 is free again: the frame goes into it, and 1 (still busy) is untouched.
        drop(busy);
        assert_eq!(
            presenter.present_into(&root, &frame(4, 3, 5)),
            PresentOutcome::Presented
        );
        assert_eq!(presenter.front, Some(0));
        assert_eq!(contents(&layer), surface_of(&presenter, 0));
        assert_eq!(tag_of(surface_of(&presenter, 0)), 5);
        assert_eq!(tag_of(also_busy.0), 2);

        // A new size replaces the pool with one new surface holding the new frame; the busy old
        // surface lives on through the compositor's (the test's) hold and is not written.
        assert_eq!(
            presenter.present_into(&root, &frame(6, 3, 6)),
            PresentOutcome::Presented
        );
        assert_eq!((presenter.pool.len(), presenter.size), (1, (6, 3)));
        assert_eq!(contents(&layer), surface_of(&presenter, 0));
        assert_eq!(tag_of(surface_of(&presenter, 0)), 6);
        assert_eq!(tag_of(also_busy.0), 2);

        // Released: no surface kept, nothing shown; the next present starts again.
        presenter.release();
        assert!(presenter.pool.is_empty());
        assert!(contents(&layer).is_null());
        assert_eq!(
            presenter.present_into(&root, &frame(4, 3, 7)),
            PresentOutcome::Presented
        );
        assert_eq!(presenter.pool.len(), 1);
        assert_eq!(tag_of(surface_of(&presenter, 0)), 7);
        drop(also_busy);
        // SAFETY: commits the implicit transaction the layers' creation opened on this thread.
        unsafe {
            let _: () = msg_send![class!(CATransaction), flush];
        }
    }

    /// Colour: softbuffer gave its images the device RGB colour space; the surfaces carry the
    /// colour space that device RGB serializes to. Colour matching must treat both alike, into
    /// that space and into a wide-gamut display's.
    #[test]
    fn surface_colours_match_as_device_rgb_does() {
        let surface = Surface::new(1, 1).unwrap();
        let key = NSString::from_str("IOSurfaceColorSpace");
        let p3_name = NSString::from_str("kCGColorSpaceDisplayP3");
        // SAFETY: Create/Copy results are released once each, after their last use.
        unsafe {
            let list = IOSurfaceCopyValue(surface.0.as_ptr(), (&*key as *const NSString).cast());
            assert!(!list.is_null(), "the surface carries a colour space");
            let tagged = CGColorSpaceCreateWithPropertyList(list);
            CFRelease(list);
            assert!(!tagged.is_null());
            let device = CGColorSpaceCreateDeviceRGB();
            let p3 = CGColorSpaceCreateWithName((&*p3_name as *const NSString).cast());
            assert!(!device.is_null() && !p3.is_null());
            let samples = [
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
                [0.2, 0.5, 0.9],
                [0.93, 0.93, 0.93],
                [0.1, 0.1, 0.12],
            ];
            for [r, g, b] in samples {
                for target in [tagged, p3] {
                    let matched = |space| {
                        let color = CGColorCreate(space, [r, g, b, 1.0].as_ptr());
                        let out = CGColorCreateCopyByMatchingToColorSpace(
                            target,
                            0,
                            color,
                            std::ptr::null(),
                        );
                        CGColorRelease(color);
                        assert!(!out.is_null());
                        let c = std::slice::from_raw_parts(CGColorGetComponents(out), 3).to_vec();
                        CGColorRelease(out);
                        c
                    };
                    let (from_device, from_tagged) = (matched(device), matched(tagged));
                    for (a, b) in from_device.iter().zip(&from_tagged) {
                        assert!(
                            (a - b).abs() < 0.5 / 255.0,
                            "{r} {g} {b}: device {from_device:?} tagged {from_tagged:?}"
                        );
                    }
                }
            }
            CGColorSpaceRelease(p3);
            CGColorSpaceRelease(device);
            CGColorSpaceRelease(tagged);
        }
    }
}
