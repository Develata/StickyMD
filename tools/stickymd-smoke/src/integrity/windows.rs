//! Windows CNG adapter for streaming SHA-256; no algorithm implementation or digest cache.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::{
    ffi::c_void,
    fs::File,
    io::{self, Read},
    path::Path,
    ptr,
};

type Handle = *mut c_void;

#[link(name = "bcrypt")]
unsafe extern "system" {
    fn BCryptOpenAlgorithmProvider(
        output: *mut Handle,
        algorithm: *const u16,
        provider: *const u16,
        flags: u32,
    ) -> i32;
    fn BCryptCloseAlgorithmProvider(algorithm: Handle, flags: u32) -> i32;
    fn BCryptCreateHash(
        algorithm: Handle,
        output: *mut Handle,
        object: *mut u8,
        object_size: u32,
        secret: *mut u8,
        secret_size: u32,
        flags: u32,
    ) -> i32;
    fn BCryptDestroyHash(hash: Handle) -> i32;
    fn BCryptHashData(hash: Handle, input: *const u8, length: u32, flags: u32) -> i32;
    fn BCryptFinishHash(hash: Handle, output: *mut u8, length: u32, flags: u32) -> i32;
}

struct Algorithm(Handle);
struct Hash<'a> {
    handle: Handle,
    // Enforce destruction of the hash before its provider, including early errors.
    _algorithm: &'a Algorithm,
}

fn check(status: i32, operation: &str) -> Result<(), String> {
    if status >= 0 {
        Ok(())
    } else {
        Err(format!(
            "{operation} failed with NTSTATUS 0x{:08x}",
            status as u32
        ))
    }
}

impl Algorithm {
    fn open() -> Result<Self, String> {
        let name = "SHA256\0".encode_utf16().collect::<Vec<_>>();
        let mut handle = ptr::null_mut();
        // SAFETY: output points to live writable handle storage, name is NUL-terminated
        // UTF-16, default provider is requested and all pointers live through the call.
        check(
            unsafe { BCryptOpenAlgorithmProvider(&raw mut handle, name.as_ptr(), ptr::null(), 0) },
            "BCryptOpenAlgorithmProvider",
        )?;
        Ok(Self(handle))
    }

    fn hash(&self) -> Result<Hash<'_>, String> {
        let mut handle = ptr::null_mut();
        // SAFETY: provider is live. Null object/zero length requests CNG-managed memory
        // (Windows 7+); DestroyHash frees it. Null secret requests plain SHA-256, not HMAC.
        check(
            unsafe {
                BCryptCreateHash(
                    self.0,
                    &raw mut handle,
                    ptr::null_mut(),
                    0,
                    ptr::null_mut(),
                    0,
                    0,
                )
            },
            "BCryptCreateHash",
        )?;
        Ok(Hash {
            handle,
            _algorithm: self,
        })
    }
}

impl Drop for Hash<'_> {
    fn drop(&mut self) {
        // SAFETY: this guard exclusively owns the successful CreateHash handle;
        // its borrowed provider still exists, and this is its only destruction.
        let status = unsafe { BCryptDestroyHash(self.handle) };
        if let Err(error) = check(status, "BCryptDestroyHash") {
            eprintln!("{error}");
        }
    }
}

impl Drop for Algorithm {
    fn drop(&mut self) {
        // SAFETY: the owned provider is live and all borrowing hash guards have ended.
        let status = unsafe { BCryptCloseAlgorithmProvider(self.0, 0) };
        if let Err(error) = check(status, "BCryptCloseAlgorithmProvider") {
            eprintln!("{error}");
        }
    }
}

pub(super) fn sha256(path: &Path) -> Result<String, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("cannot open hash input {}: {error}", path.display()))?;
    digest(&mut file).map_err(|error| format!("cannot hash {}: {error}", path.display()))
}

fn digest(input: &mut impl Read) -> Result<String, String> {
    let algorithm = Algorithm::open()?;
    let hash = algorithm.hash()?;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let length = match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(length) => length,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(format!("cannot read hash input: {error}")),
        };
        // Read is a safe trait: do not trust a faulty implementation's length at FFI.
        if length > buffer.len() {
            return Err("hash input reader returned an invalid length".into());
        }
        // SAFETY: hash/provider remain live; buffer supplies length readable bytes,
        // bounded by 64 KiB (fits u32). HashData neither mutates nor retains the input.
        check(
            unsafe { BCryptHashData(hash.handle, buffer.as_ptr(), length as u32, 0) },
            "BCryptHashData",
        )?;
    }
    let mut bytes = [0_u8; 32];
    // SAFETY: hash is unfinished and SHA-256 requires exactly 32 output bytes.
    // bytes is writable storage and both it and the provider live through this call.
    check(
        unsafe { BCryptFinishHash(hash.handle, bytes.as_mut_ptr(), 32, 0) },
        "BCryptFinishHash",
    )?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in bytes {
        result.push(HEX[usize::from(byte >> 4)] as char);
        result.push(HEX[usize::from(byte & 15)] as char);
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
