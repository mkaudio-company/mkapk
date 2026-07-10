#![allow(unexpected_cfgs)]

use gui_core::ImageId;

#[cfg(target_os = "macos")]
use std::collections::BTreeMap;
#[cfg(target_os = "macos")]
use std::sync::Arc;

#[cfg(target_os = "macos")]
use core_graphics::base::kCGRenderingIntentDefault;
#[cfg(target_os = "macos")]
use core_graphics::color_space::CGColorSpace;
#[cfg(target_os = "macos")]
use core_graphics::data_provider::CGDataProvider;
#[cfg(target_os = "macos")]
use core_graphics::image::{CGImage, CGImageAlphaInfo};

/// Maps `ImageId`s to decoded images so the render backend can draw
/// `PaintCommand::DrawImage` commands. Populate this from decoded
/// `gui-res` resources: `PngImage::rgba_premultiplied()`, or
/// `SvgImage::render_rgba()` (already premultiplied, since `tiny-skia`
/// stores pixels premultiplied internally).
#[cfg(target_os = "macos")]
#[derive(Default)]
pub struct ImageRegistry {
    images: BTreeMap<u32, CGImage>,
}

#[cfg(target_os = "macos")]
impl ImageRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a premultiplied-alpha RGBA8 image under `id`, replacing any existing entry.
    ///
    /// Panics if `rgba.len() != width * height * 4`.
    pub fn register_rgba(&mut self, id: ImageId, width: u32, height: u32, rgba: &[u8]) {
        assert_eq!(rgba.len(), width as usize * height as usize * 4);
        let provider = CGDataProvider::from_buffer(Arc::new(rgba.to_vec()));
        let color_space = CGColorSpace::create_device_rgb();
        let image = CGImage::new(
            width as usize,
            height as usize,
            8,
            32,
            width as usize * 4,
            &color_space,
            CGImageAlphaInfo::CGImageAlphaPremultipliedLast as u32,
            &provider,
            false,
            kCGRenderingIntentDefault,
        );
        self.images.insert(id.0, image);
    }

    pub fn get(&self, id: ImageId) -> Option<&CGImage> {
        self.images.get(&id.0)
    }

    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
}

#[cfg(not(target_os = "macos"))]
#[derive(Default)]
pub struct ImageRegistry {
    _private: (),
}

#[cfg(not(target_os = "macos"))]
impl ImageRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_rgba(&mut self, _id: ImageId, _width: u32, _height: u32, _rgba: &[u8]) {}

    pub fn get(&self, _id: ImageId) -> Option<&()> {
        None
    }

    pub fn len(&self) -> usize {
        0
    }

    pub fn is_empty(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn register_and_fetch_rgba_image() {
        let mut registry = ImageRegistry::new();
        assert!(registry.is_empty());
        let pixels = [
            255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        registry.register_rgba(ImageId(1), 2, 2, &pixels);
        assert_eq!(registry.len(), 1);
        let image = registry
            .get(ImageId(1))
            .expect("image should be registered");
        assert_eq!(image.width(), 2);
        assert_eq!(image.height(), 2);
    }
}
