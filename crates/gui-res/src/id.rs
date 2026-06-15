use core::fmt;

/// A lightweight identifier for a resource.
///
/// `ResourceId` is intentionally a small, `Copy`, hashable newtype around a `u32`.
/// It can be constructed from an explicit raw value or derived from a byte slice
/// at compile time via [`ResourceId::from_bytes_le`].
///
/// # Collision warning
///
/// The [`ResourceId::from_bytes_le`] constructor uses a simple hash of the input
/// bytes. Collisions are possible, so it is only recommended for development
/// convenience. Production code should use explicit, pre-assigned resource IDs
/// via [`ResourceId::new`].
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
pub struct ResourceId(u32);

impl ResourceId {
    /// Create a resource ID from an explicit raw value.
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Return the underlying raw `u32` value.
    pub const fn get(&self) -> u32 {
        self.0
    }

    /// Derive a resource ID from a byte slice using a simple FNV-1a-like hash.
    ///
    /// This function is `const`, so it can be used to generate resource IDs at
    /// compile time. Because it is a hash, collisions are possible; use explicit
    /// IDs in production.
    pub const fn from_bytes_le(bytes: &[u8]) -> Self {
        const FNV_OFFSET_BASIS: u32 = 0x811c_9dc5;
        const FNV_PRIME: u32 = 0x0100_0193;

        let mut hash = FNV_OFFSET_BASIS;
        let mut i = 0;
        while i < bytes.len() {
            hash ^= bytes[i] as u32;
            hash = hash.wrapping_mul(FNV_PRIME);
            i += 1;
        }
        Self(hash)
    }
}

impl fmt::Debug for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ResourceId({:#010x})", self.0)
    }
}
