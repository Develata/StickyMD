//! Owned CF references; borrowed array/dictionary entries are retained before escape.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use std::ffi::{c_char, c_void};
pub(super) type Cf = *const c_void;
pub(super) struct Owned(pub Cf);
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(v: Cf);
    fn CFRetain(v: Cf) -> Cf;
    fn CFGetTypeID(v: Cf) -> usize;
    fn CFEqual(a: Cf, b: Cf) -> bool;
    fn CFStringCreateWithBytes(a: Cf, b: *const u8, n: isize, encoding: u32, external: bool) -> Cf;
    fn CFStringGetCString(v: Cf, b: *mut c_char, n: isize, encoding: u32) -> bool;
    fn CFStringGetLength(v: Cf) -> isize;
    fn CFStringGetTypeID() -> usize;
    fn CFArrayGetCount(v: Cf) -> isize;
    fn CFArrayGetValueAtIndex(v: Cf, i: isize) -> Cf;
    fn CFArrayGetTypeID() -> usize;
    fn CFDictionaryGetValue(v: Cf, key: Cf) -> Cf;
    fn CFDictionaryGetTypeID() -> usize;
    fn CFNumberGetValue(v: Cf, kind: i32, out: *mut c_void) -> bool;
    fn CFNumberGetTypeID() -> usize;
    fn CFBooleanGetValue(v: Cf) -> bool;
    fn CFBooleanGetTypeID() -> usize;
    pub(super) static kCFBooleanTrue: Cf;
}
impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: Owned holds a non-null Create/Copy or retained reference.
        unsafe { CFRelease(self.0) }
    }
}
impl Owned {
    pub(super) fn is_type(&self, expected: usize) -> bool {
        // SAFETY: Owned always holds a live, non-null CF reference.
        unsafe { CFGetTypeID(self.0) == expected }
    }
    pub(super) fn same(&self, other: &Self) -> bool {
        // SAFETY: both live CF references remain retained during equality comparison.
        unsafe { CFEqual(self.0, other.0) }
    }
    /// Caller must provide a Create/Copy-rule reference or null. Native-module boundary only.
    pub(super) fn take(v: Cf) -> Result<Self, String> {
        if v.is_null() {
            Err("native API returned null".into())
        } else {
            Ok(Self(v))
        }
    }
    pub(super) fn string(s: &str) -> Result<Self, String> {
        // SAFETY: byte slice lives through the copying UTF-8 constructor; null allocator is default.
        Self::take(unsafe {
            CFStringCreateWithBytes(
                std::ptr::null(),
                s.as_ptr(),
                s.len() as isize,
                0x08000100,
                false,
            )
        })
    }
    pub(super) fn text(&self) -> Option<String> {
        // SAFETY: type is checked before string operations; buffer accommodates UTF-8 maximum.
        unsafe {
            if CFGetTypeID(self.0) != CFStringGetTypeID() {
                return None;
            }
            let n = usize::try_from(CFStringGetLength(self.0))
                .ok()?
                .checked_mul(4)?
                .checked_add(1)?;
            if n > 1_048_576 {
                return None;
            }
            let mut bytes = vec![0; n];
            if !CFStringGetCString(self.0, bytes.as_mut_ptr().cast(), n as isize, 0x08000100) {
                return None;
            }
            bytes.truncate(bytes.iter().position(|b| *b == 0)?);
            String::from_utf8(bytes).ok()
        }
    }
    pub(super) fn array(&self) -> Result<Vec<Self>, String> {
        // SAFETY: array type checked; bounded borrowed entries retained before parent can drop.
        unsafe {
            if CFGetTypeID(self.0) != CFArrayGetTypeID() {
                return Err("expected CFArray".into());
            }
            let n = CFArrayGetCount(self.0);
            if !(0..=10000).contains(&n) {
                return Err("unbounded CFArray".into());
            }
            (0..n)
                .map(|i| {
                    let p = CFArrayGetValueAtIndex(self.0, i);
                    if p.is_null() {
                        Err("null array element".into())
                    } else {
                        Self::take(CFRetain(p))
                    }
                })
                .collect()
        }
    }
    pub(super) fn get(&self, key: &str) -> Result<Self, String> {
        let key = Self::string(key)?;
        // SAFETY: dictionary type checked; non-null borrowed value retained for independent owner.
        unsafe {
            if CFGetTypeID(self.0) != CFDictionaryGetTypeID() {
                return Err("expected CFDictionary".into());
            }
            let p = CFDictionaryGetValue(self.0, key.0);
            if p.is_null() {
                Err("missing dictionary key".into())
            } else {
                Self::take(CFRetain(p))
            }
        }
    }
    pub(super) fn number(&self) -> Result<f64, String> {
        let mut n = 0.0;
        // SAFETY: type checked; CFNumber double kind 13 writes one f64.
        if unsafe {
            CFGetTypeID(self.0) == CFNumberGetTypeID()
                && CFNumberGetValue(self.0, 13, (&raw mut n).cast())
        } && f64::is_finite(n)
        {
            Ok(n)
        } else {
            Err("invalid CFNumber".into())
        }
    }
    pub(super) fn boolean(&self) -> Result<bool, String> {
        // SAFETY: type checked before reading CFBoolean.
        unsafe {
            if CFGetTypeID(self.0) == CFBooleanGetTypeID() {
                Ok(CFBooleanGetValue(self.0))
            } else {
                Err("expected CFBoolean".into())
            }
        }
    }
}
