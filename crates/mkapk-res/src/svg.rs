use crate::Resource;

pub struct SvgImage {
    tree: usvg::Tree,
    cache: Option<tiny_skia::Pixmap>,
}

impl Resource for SvgImage {
    fn decode(bytes: &[u8]) -> Option<Self>
    where
        Self: Sized,
    {
        let text = std::str::from_utf8(bytes).ok()?;
        let tree = usvg::Tree::from_str(text, &usvg::Options::default()).ok()?;
        Some(Self { tree, cache: None })
    }
}

impl SvgImage {
    pub fn tree(&self) -> &usvg::Tree {
        &self.tree
    }

    pub fn width(&self) -> u32 {
        self.tree.size().to_int_size().width()
    }

    pub fn height(&self) -> u32 {
        self.tree.size().to_int_size().height()
    }

    pub fn render(&mut self, size: (u32, u32)) -> Option<&[u8]> {
        let (width, height) = size;
        if width == 0 || height == 0 {
            return None;
        }

        let needs_render = self
            .cache
            .as_ref()
            .is_none_or(|pixmap| pixmap.width() != width || pixmap.height() != height);

        if needs_render {
            let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
            resvg::render(
                &self.tree,
                tiny_skia::Transform::default(),
                &mut pixmap.as_mut(),
            );
            self.cache = Some(pixmap);
        }

        Some(self.cache.as_ref().unwrap().data())
    }

    pub fn render_rgba(&mut self, size: (u32, u32)) -> Option<(u32, u32, Vec<u8>)> {
        let (width, height) = size;
        self.render(size)?;
        let pixmap = self.cache.as_ref().unwrap();
        Some((width, height, pixmap.data().to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED_SQUARE: &[u8] = b"<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'><rect width='10' height='10' fill='red'/></svg>";

    fn decode_sample() -> SvgImage {
        SvgImage::decode(RED_SQUARE).unwrap()
    }

    #[test]
    fn decode_intrinsic_size() {
        let svg = decode_sample();
        assert_eq!(svg.width(), 10);
        assert_eq!(svg.height(), 10);
    }

    #[test]
    fn render_produces_rgba_bytes() {
        let mut svg = decode_sample();
        let bytes = svg.render((20, 20)).unwrap();
        assert_eq!(bytes.len(), 20 * 20 * 4);
        assert!(bytes[3] > 0);
    }

    #[test]
    fn render_caches_pixmap() {
        let mut svg = decode_sample();
        let first = svg.render((20, 20)).unwrap().as_ptr();
        let second = svg.render((20, 20)).unwrap().as_ptr();
        assert_eq!(first, second);
    }
}
