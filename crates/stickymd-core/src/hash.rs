//! Opaque 32-byte content hash carried by the document/persistence contract.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#documentstate
//!
//! `Hash32` is the digest size used for the on-disk content hash (`base_disk_hash`)
//! and for identifying our own writes vs. external file facts. Hashing is a pure
//! byte operation; opening and reading files remains an Execution Domain concern.

/// A fixed-size 32-byte content digest (SHA-256 width).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash32([u8; 32]);

impl Hash32 {
    /// The byte length of the digest.
    pub const LEN: usize = 32;

    /// Wrap a raw 32-byte digest.
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Build a `Hash32` from a slice; returns `None` unless it is exactly 32 bytes.
    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::LEN {
            return None;
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(bytes);
        Some(Self(out))
    }

    /// The raw digest bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Lowercase hex representation (for diagnostics and logging).
    pub fn to_hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut output = [0_u8; 64];
        for (index, byte) in self.0.into_iter().enumerate() {
            output[index * 2] = HEX[(byte >> 4) as usize];
            output[index * 2 + 1] = HEX[(byte & 0x0f) as usize];
        }
        String::from_utf8(output.to_vec()).unwrap_or_default()
    }
}

impl core::fmt::Debug for Hash32 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Hash32({})", self.to_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash32_roundtrip_and_reject() {
        let bytes = [7u8; 32];
        let h = Hash32::new(bytes);
        assert_eq!(h.as_bytes(), &bytes);
        assert_eq!(Hash32::from_slice(&bytes).unwrap(), h);
        assert!(Hash32::from_slice(&bytes[..31]).is_none());
        assert!(Hash32::from_slice(&[0u8; 33]).is_none());
    }

    #[test]
    fn hash32_hex_is_lowercase_64() {
        let h = Hash32::new([0xAB; 32]);
        let hex = h.to_hex();
        assert_eq!(hex.len(), 64);
        assert_eq!(hex, "ab".repeat(32));
    }
}
