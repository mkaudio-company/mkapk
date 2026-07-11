use std::collections::BTreeMap;

use mkapk_core::ImageId;

/// Maps `ImageId`s to decoded premultiplied-alpha RGBA8 bytes so the render
/// backend can draw `PaintCommand::DrawImage` commands. Populate this from
/// decoded `mkapk-res` resources: `PngImage::rgba_premultiplied()`, or
/// `SvgImage::render_rgba()` (already premultiplied, since `tiny-skia`
/// stores pixels premultiplied internally).
///
/// The Direct2D backend is reconstructed every frame (see `render_to_hwnd`),
/// so `ID2D1Bitmap`s can't be cached across frames against a stale device
/// context; this registry stores plain pixel data and the backend creates a
/// transient bitmap from it each time a `DrawImage` command references it.
#[derive(Default)]
pub struct ImageRegistry {
    images: BTreeMap<u32, (u32, u32, Vec<u8>)>,
}

impl ImageRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a non-premultiplied RGBA8 image under `id`, replacing any existing entry.
    ///
    /// Panics if `rgba.len() != width * height * 4`.
    pub fn register_rgba(&mut self, id: ImageId, width: u32, height: u32, rgba: &[u8]) {
        assert_eq!(rgba.len(), width as usize * height as usize * 4);
        self.images.insert(id.0, (width, height, rgba.to_vec()));
    }

    pub fn get(&self, id: ImageId) -> Option<(u32, u32, &[u8])> {
        self.images
            .get(&id.0)
            .map(|(w, h, bytes)| (*w, *h, bytes.as_slice()))
    }

    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_fetch_rgba_image() {
        let mut registry = ImageRegistry::new();
        assert!(registry.is_empty());
        let pixels = [
            255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        registry.register_rgba(ImageId(1), 2, 2, &pixels);
        assert_eq!(registry.len(), 1);
        let (w, h, bytes) = registry
            .get(ImageId(1))
            .expect("image should be registered");
        assert_eq!((w, h), (2, 2));
        assert_eq!(bytes, pixels);
    }
}
