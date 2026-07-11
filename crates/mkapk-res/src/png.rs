extern crate alloc;

use crate::Resource;

pub struct PngImage {
    width: u32,
    height: u32,
    rgba: alloc::vec::Vec<u8>,
}

impl Resource for PngImage {
    fn decode(bytes: &[u8]) -> Option<Self>
    where
        Self: Sized,
    {
        let image = image::load_from_memory(bytes).ok()?;
        let rgba = image.into_rgba8();
        let width = rgba.width();
        let height = rgba.height();
        Some(Self {
            width,
            height,
            rgba: rgba.into_raw(),
        })
    }
}

impl PngImage {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn rgba_premultiplied(&self) -> alloc::vec::Vec<u8> {
        let mut out = alloc::vec::Vec::with_capacity(self.rgba.len());
        for chunk in self.rgba.chunks_exact(4) {
            let r = chunk[0];
            let g = chunk[1];
            let b = chunk[2];
            let a = chunk[3];
            out.push(((r as u16 * a as u16) / 255) as u8);
            out.push(((g as u16 * a as u16) / 255) as u8);
            out.push(((b as u16 * a as u16) / 255) as u8);
            out.push(a);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::EMBEDDED;
    use crate::{ResourceId, ResourceRegistry};

    #[test]
    fn decode_embedded_png() {
        let mut registry = ResourceRegistry::new();
        EMBEDDED.register_with(&mut registry);

        let id = ResourceId::from_bytes_le(b"test.png");
        let png = registry.load::<PngImage>(id).unwrap();
        assert_eq!(png.width(), 1);
        assert_eq!(png.height(), 1);
        assert_eq!(png.rgba(), &[255, 0, 0, 255]);
    }

    #[test]
    fn load_caches_and_reuses_arc() {
        let mut registry = ResourceRegistry::new();
        EMBEDDED.register_with(&mut registry);

        let id = ResourceId::from_bytes_le(b"test.png");
        let png1 = registry.load::<PngImage>(id).unwrap();
        let png2 = registry.load::<PngImage>(id).unwrap();

        assert!(core::ptr::eq(&*png1, &*png2));
    }

    #[test]
    fn rgba_premultiplied_scales_by_alpha() {
        let png = PngImage {
            width: 1,
            height: 1,
            rgba: alloc::vec::Vec::from([128, 64, 32, 128]),
        };
        let premul = png.rgba_premultiplied();
        assert_eq!(premul.len(), 4);
        assert_eq!(premul[0], 64);
        assert_eq!(premul[1], 32);
        assert_eq!(premul[2], 16);
        assert_eq!(premul[3], 128);
    }
}
