use crate::{ResourceId, ResourceRegistry};

/// A collection of embedded resources addressable by [`ResourceId`].
pub trait ResourceBundle {
    /// Look up a resource by ID.
    fn get(&self, id: ResourceId) -> Option<&'static [u8]>;

    /// Returns `true` if the bundle contains a resource with the given ID.
    fn contains(&self, id: ResourceId) -> bool {
        self.get(id).is_some()
    }
}

/// A resource bundle backed by a static slice of `(ResourceId, bytes)` pairs.
#[derive(Copy, Clone, Debug)]
pub struct EmbeddedBundle {
    entries: &'static [(ResourceId, &'static [u8])],
}

impl EmbeddedBundle {
    /// Create a new embedded bundle from a static slice of entries.
    pub const fn new(entries: &'static [(ResourceId, &'static [u8])]) -> Self {
        Self { entries }
    }

    pub fn register_with(&self, registry: &mut ResourceRegistry) {
        for (id, bytes) in self.entries {
            registry.register_bytes(*id, bytes);
        }
    }
}

impl ResourceBundle for EmbeddedBundle {
    fn get(&self, id: ResourceId) -> Option<&'static [u8]> {
        for (entry_id, bytes) in self.entries {
            if *entry_id == id {
                return Some(bytes);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::EMBEDDED;

    #[test]
    fn embedded_png_roundtrip() {
        let expected: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xD7, 0x63, 0xF8, 0x0F, 0x00, 0x00, 0x01, 0x01, 0x00, 0x05, 0x18, 0xD8, 0x49, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        let id = ResourceId::from_bytes_le(b"test.png");
        let bytes = EMBEDDED.get(id).expect("test.png should be embedded");
        assert_eq!(bytes, expected);
    }
}
