use std::any::Any;
use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::Arc;
use std::vec::Vec;

use crate::ResourceId;

#[derive(Debug)]
pub struct ResourceHandle<T>(Arc<T>);

impl<T> Clone for ResourceHandle<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Deref for ResourceHandle<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> From<Arc<T>> for ResourceHandle<T> {
    fn from(arc: Arc<T>) -> Self {
        Self(arc)
    }
}

pub trait Resource: Send + Sync + 'static {
    fn decode(bytes: &[u8]) -> Option<Self>
    where
        Self: Sized;
}

#[derive(Clone, Debug, PartialEq)]
pub struct Image(pub Vec<u8>);

impl Resource for Image {
    fn decode(bytes: &[u8]) -> Option<Self> {
        Some(Image(bytes.to_vec()))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Svg(pub String);

impl Resource for Svg {
    fn decode(bytes: &[u8]) -> Option<Self> {
        Some(Svg(String::from_utf8(bytes.to_vec()).ok()?))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Font(pub Vec<u8>);

impl Resource for Font {
    fn decode(bytes: &[u8]) -> Option<Self> {
        Some(Font(bytes.to_vec()))
    }
}

#[derive(Debug, Default)]
pub struct ResourceRegistry {
    typed: BTreeMap<ResourceId, Arc<dyn Any + Send + Sync>>,
    raw: BTreeMap<ResourceId, &'static [u8]>,
}

impl ResourceRegistry {
    pub fn new() -> Self {
        Self {
            typed: BTreeMap::new(),
            raw: BTreeMap::new(),
        }
    }

    pub fn register_bytes(&mut self, id: ResourceId, bytes: &'static [u8]) {
        self.raw.insert(id, bytes);
    }

    pub fn load<T: Resource>(&mut self, id: ResourceId) -> Option<ResourceHandle<T>> {
        if let Some(handle) = self.get(id) {
            return Some(handle);
        }

        let bytes = self.raw.get(&id).copied()?;
        let decoded = T::decode(bytes)?;
        let arc = Arc::new(decoded);
        self.typed
            .insert(id, arc.clone() as Arc<dyn Any + Send + Sync>);
        Some(ResourceHandle::from(arc))
    }

    pub fn get<T: Resource>(&self, id: ResourceId) -> Option<ResourceHandle<T>> {
        let arc = self.typed.get(&id)?;
        arc.clone().downcast::<T>().ok().map(ResourceHandle::from)
    }

    pub fn evict(&mut self, id: ResourceId) {
        self.typed.remove(&id);
        self.raw.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::EMBEDDED;

    #[test]
    fn load_image_from_raw_bytes() {
        let bytes: &'static [u8] = b"image-bytes";
        let id = ResourceId::new(1);

        let mut registry = ResourceRegistry::new();
        registry.register_bytes(id, bytes);

        let handle = registry.load::<Image>(id).unwrap();
        assert_eq!(&*handle, &Image(bytes.to_vec()));
    }

    #[test]
    fn load_caches_and_reuses_arc() {
        let bytes: &'static [u8] = b"shared-bytes";
        let id = ResourceId::new(2);

        let mut registry = ResourceRegistry::new();
        registry.register_bytes(id, bytes);

        let handle1 = registry.load::<Image>(id).unwrap();
        let handle2 = registry.load::<Image>(id).unwrap();

        assert!(core::ptr::eq(&*handle1, &*handle2));
    }

    #[test]
    fn evict_removes_cached_and_raw() {
        let bytes: &'static [u8] = b"evict-me";
        let id = ResourceId::new(3);

        let mut registry = ResourceRegistry::new();
        registry.register_bytes(id, bytes);
        registry.load::<Image>(id).unwrap();

        registry.evict(id);

        assert!(registry.get::<Image>(id).is_none());
        assert!(registry.load::<Image>(id).is_none());
    }

    #[test]
    fn register_from_embedded_bundle_and_load() {
        let mut registry = ResourceRegistry::new();
        EMBEDDED.register_with(&mut registry);

        let id = ResourceId::from_bytes_le(b"test.png");
        let handle = registry.load::<Image>(id).unwrap();
        assert!(!(&*handle).0.is_empty());
    }
}
