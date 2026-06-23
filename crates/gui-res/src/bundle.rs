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
        let expected: &[u8] =
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/test.png"));
        let id = ResourceId::from_bytes_le(b"test.png");
        let bytes = EMBEDDED.get(id).expect("test.png should be embedded");
        assert_eq!(bytes, expected);
    }
}
