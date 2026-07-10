#![allow(unexpected_cfgs)]

use gui_core::{Pointf, Sizef, TextLayoutId};

#[cfg(target_os = "macos")]
use core_foundation::attributed_string::CFMutableAttributedString;
#[cfg(target_os = "macos")]
use core_foundation::base::CFRange;
#[cfg(target_os = "macos")]
use core_foundation::base::TCFType;
#[cfg(target_os = "macos")]
use core_foundation::string::CFString;
#[cfg(target_os = "macos")]
use core_graphics::base::CGFloat;
#[cfg(target_os = "macos")]
use core_text::font::CTFont;
#[cfg(target_os = "macos")]
use core_text::line::CTLine;
#[cfg(target_os = "macos")]
use core_text::string_attributes::kCTFontAttributeName;

#[cfg(target_os = "macos")]
pub struct TextLayout {
    line: CTLine,
    #[allow(dead_code)]
    font: CTFont,
}

#[cfg(target_os = "macos")]
impl TextLayout {
    pub fn new(text: &str, font_size: f32) -> Self {
        let font = create_font(font_size);

        let mut attr_string = CFMutableAttributedString::new();
        let cf_string = CFString::new(text);
        attr_string.replace_str(&cf_string, CFRange::init(0, 0));

        let full_range = CFRange::init(0, attr_string.char_len());
        attr_string.set_attribute(full_range, unsafe { kCTFontAttributeName }, &font);

        let line = CTLine::new_with_attributed_string(attr_string.as_concrete_TypeRef()
            as core_foundation::attributed_string::CFAttributedStringRef);

        Self { line, font }
    }

    pub fn size(&self) -> Sizef {
        let bounds = self.line.get_typographic_bounds();
        Sizef::new(bounds.width as f32, (bounds.ascent + bounds.descent) as f32)
    }

    pub fn baseline(&self) -> f32 {
        self.line.get_typographic_bounds().ascent as f32
    }

    pub fn draw(&self, context: &core_graphics::context::CGContext, position: Pointf) {
        context.save();
        context.set_text_position(
            position.x as CGFloat,
            position.y as CGFloat + self.baseline() as CGFloat,
        );
        self.line.draw(context);
        context.restore();
    }
}

#[cfg(target_os = "macos")]
fn create_font(font_size: f32) -> CTFont {
    core_text::font::new_from_name("Helvetica", font_size as f64)
        .or_else(|_| core_text::font::new_from_name("Arial", font_size as f64))
        .or_else(|_| core_text::font::new_from_name("Helvetica Neue", font_size as f64))
        .expect("failed to create a system font")
}

#[cfg(not(target_os = "macos"))]
pub struct TextLayout;

#[cfg(not(target_os = "macos"))]
impl TextLayout {
    pub fn new(_text: &str, _font_size: f32) -> Self {
        Self
    }

    pub fn size(&self) -> Sizef {
        Sizef::new(0.0, 0.0)
    }

    pub fn baseline(&self) -> f32 {
        0.0
    }

    pub fn draw(&self, _context: &core::ffi::c_void, _position: Pointf) {}
}

/// Maps `TextLayoutId`s to prepared text layouts so the render backend can
/// draw `PaintCommand::DrawText` commands.
#[cfg(target_os = "macos")]
#[derive(Default)]
pub struct TextRegistry {
    layouts: std::collections::BTreeMap<u32, TextLayout>,
}

#[cfg(target_os = "macos")]
impl TextRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: TextLayoutId, layout: TextLayout) {
        self.layouts.insert(id.0, layout);
    }

    pub fn get(&self, id: TextLayoutId) -> Option<&TextLayout> {
        self.layouts.get(&id.0)
    }

    pub fn len(&self) -> usize {
        self.layouts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.layouts.is_empty()
    }
}

#[cfg(not(target_os = "macos"))]
#[derive(Default)]
pub struct TextRegistry {
    _private: (),
}

#[cfg(not(target_os = "macos"))]
impl TextRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, _id: TextLayoutId, _layout: TextLayout) {}

    pub fn get(&self, _id: TextLayoutId) -> Option<&TextLayout> {
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

    #[test]
    fn text_layout_size_is_non_negative() {
        let layout = TextLayout::new("Hello", 16.0);
        let size = layout.size();
        assert!(size.width >= 0.0);
        assert!(size.height >= 0.0);
    }
}
