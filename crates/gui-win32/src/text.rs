#![allow(unexpected_cfgs)]

use gui_core::{Pointf, Sizef};

#[cfg(target_os = "windows")]
use gui_core::RenderBackend;

#[cfg(target_os = "windows")]
use windows::{
    Win32::Foundation::{FALSE, HWND},
    Win32::Graphics::DirectWrite::{
        DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
        DWRITE_FONT_WEIGHT_NORMAL, DWRITE_LINE_METRICS, DWRITE_TEXT_METRICS, DWriteCreateFactory,
        IDWriteFactory5, IDWriteFontCollection, IDWriteFontCollection1, IDWriteFontFamily,
        IDWriteInMemoryFontFileLoader, IDWriteTextFormat, IDWriteTextLayout,
    },
    core::{HSTRING, IUnknown, Interface},
};

use crate::render::D2DRenderBackend;

/// DirectWrite-backed text layout.
#[cfg(target_os = "windows")]
pub struct TextLayout {
    factory: IDWriteFactory5,
    loader: Option<IDWriteInMemoryFontFileLoader>,
    #[allow(dead_code)]
    collection: IDWriteFontCollection,
    #[allow(dead_code)]
    format: IDWriteTextFormat,
    layout: IDWriteTextLayout,
    #[allow(dead_code)]
    font_bytes: Vec<u8>,
    size: Sizef,
    baseline: f32,
}

#[cfg(target_os = "windows")]
impl TextLayout {
    /// Creates a text layout using a system font family.
    pub fn new(text: &str, font_size: f32) -> Self {
        unsafe {
            let factory =
                DWriteCreateFactory::<IDWriteFactory5>(DWRITE_FACTORY_TYPE_SHARED).unwrap();

            let mut collection_opt = None;
            factory
                .GetSystemFontCollection(false, &mut collection_opt, false)
                .expect("failed to get system font collection");
            let collection1 = collection_opt.expect("no system font collection returned");
            let collection: IDWriteFontCollection = collection1
                .cast()
                .expect("failed to cast system font collection");

            let family_name = system_family_name(&collection).expect("no usable system font");
            Self::build(
                factory,
                collection,
                None,
                Vec::new(),
                text,
                font_size,
                family_name,
            )
        }
    }

    /// Creates a text layout from an embedded font loaded into an in-memory
    /// DirectWrite font file loader.
    pub fn with_font(text: &str, font_size: f32, font_bytes: &[u8]) -> Self {
        unsafe {
            let factory =
                DWriteCreateFactory::<IDWriteFactory5>(DWRITE_FACTORY_TYPE_SHARED).unwrap();

            let loader = factory
                .CreateInMemoryFontFileLoader()
                .expect("failed to create in-memory font loader");
            factory
                .RegisterFontFileLoader(&loader)
                .expect("failed to register font file loader");

            let font_file = loader
                .CreateInMemoryFontFileReference(
                    &factory,
                    font_bytes.as_ptr() as *const core::ffi::c_void,
                    font_bytes.len() as u32,
                    None::<&IUnknown>,
                )
                .expect("failed to create in-memory font file reference");

            let builder = factory
                .CreateFontSetBuilder()
                .expect("failed to create font set builder");
            builder
                .AddFontFile(&font_file)
                .expect("failed to add font file to font set");

            let font_set = builder.CreateFontSet().expect("failed to create font set");
            let collection1 = factory
                .CreateFontCollectionFromFontSet(&font_set)
                .expect("failed to create custom font collection");
            let family_name = family_name_from_collection(&collection1)
                .expect("failed to read custom font family name");

            let collection: IDWriteFontCollection = collection1
                .cast()
                .expect("failed to cast custom font collection");

            Self::build(
                factory,
                collection,
                Some(loader),
                font_bytes.to_vec(),
                text,
                font_size,
                family_name,
            )
        }
    }

    unsafe fn build(
        factory: IDWriteFactory5,
        collection: IDWriteFontCollection,
        loader: Option<IDWriteInMemoryFontFileLoader>,
        font_bytes: Vec<u8>,
        text: &str,
        font_size: f32,
        family_name: HSTRING,
    ) -> Self {
        let locale = HSTRING::from("en-us");
        let format = factory
            .CreateTextFormat(
                &family_name,
                Some(&collection),
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                font_size,
                &locale,
            )
            .expect("failed to create text format");

        let text_wide: Vec<u16> = text.encode_utf16().collect();
        let layout = factory
            .CreateTextLayout(&text_wide, &format, 1_000_000.0, 1_000_000.0)
            .expect("failed to create text layout");

        let (size, baseline) = measure_text(&layout);

        Self {
            factory,
            loader,
            collection,
            format,
            layout,
            font_bytes,
            size,
            baseline,
        }
    }

    pub fn size(&self) -> Sizef {
        self.size
    }

    pub fn baseline(&self) -> f32 {
        self.baseline
    }

    pub fn draw(&self, backend: &mut D2DRenderBackend, position: Pointf) {
        backend.draw_text_layout(&self.layout, position, gui_core::WHITE);
    }
}

#[cfg(target_os = "windows")]
impl Drop for TextLayout {
    fn drop(&mut self) {
        unsafe {
            if let Some(loader) = self.loader.take() {
                let _ = self.factory.UnregisterFontFileLoader(&loader);
            }
        }
    }
}

#[cfg(target_os = "windows")]
unsafe fn system_family_name(collection: &IDWriteFontCollection) -> Option<HSTRING> {
    for candidate in ["Segoe UI", "Arial", "Microsoft Sans Serif", "Tahoma"] {
        let name = HSTRING::from(candidate);
        let mut index = 0;
        let mut exists = FALSE;
        collection
            .FindFamilyName(&name, &mut index, &mut exists)
            .ok()?;
        if exists.as_bool() {
            return Some(name);
        }
    }

    if collection.GetFontFamilyCount() == 0 {
        return None;
    }
    let family = collection.GetFontFamily(0).ok()?;
    localized_family_name(&family)
}

#[cfg(target_os = "windows")]
unsafe fn family_name_from_collection(collection: &IDWriteFontCollection1) -> Option<HSTRING> {
    if collection.GetFontFamilyCount() == 0 {
        return None;
    }
    let family = collection.GetFontFamily(0).ok()?;
    localized_family_name(&family)
}

#[cfg(target_os = "windows")]
unsafe fn localized_family_name(family: &IDWriteFontFamily) -> Option<HSTRING> {
    let names = family.GetFamilyNames().ok()?;
    let len = names.GetStringLength(0).ok()?;
    let mut buffer = vec![0u16; (len + 1) as usize];
    names.GetString(0, &mut buffer).ok()?;
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(HSTRING::from_wide(&buffer[..end]))
}

#[cfg(target_os = "windows")]
unsafe fn measure_text(layout: &IDWriteTextLayout) -> (Sizef, f32) {
    let mut metrics = DWRITE_TEXT_METRICS::default();
    let _ = layout.GetMetrics(&mut metrics);

    let mut line_count = 0;
    let _ = layout.GetLineMetrics(None, &mut line_count);

    let mut lines = vec![DWRITE_LINE_METRICS::default(); line_count as usize];
    if line_count > 0 {
        let _ = layout.GetLineMetrics(Some(&mut lines), &mut line_count);
    }

    let baseline = lines.first().map(|line| line.baseline).unwrap_or(0.0);
    (Sizef::new(metrics.width, metrics.height), baseline)
}

/// Cache for reusing DirectWrite text layouts.
///
/// The cache key is `(text_hash, font_size_bits, font_key)`.
#[cfg(target_os = "windows")]
pub struct TextCache {
    entries: std::collections::HashMap<(u64, u32, u64), TextLayout>,
}

#[cfg(target_os = "windows")]
impl Default for TextCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "windows")]
impl TextCache {
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    pub fn get(&self, text: &str, font_size: f32, font_key: u64) -> Option<&TextLayout> {
        let key = (hash_text(text), font_size.to_bits(), font_key);
        self.entries.get(&key)
    }

    pub fn insert(&mut self, text: &str, font_size: f32, font_key: u64, layout: TextLayout) {
        let key = (hash_text(text), font_size.to_bits(), font_key);
        self.entries.insert(key, layout);
    }

    pub fn get_or_insert(
        &mut self,
        text: &str,
        font_size: f32,
        font_key: u64,
        build: impl FnOnce() -> TextLayout,
    ) -> &TextLayout {
        let key = (hash_text(text), font_size.to_bits(), font_key);
        self.entries.entry(key).or_insert_with(build)
    }
}

#[cfg(target_os = "windows")]
fn hash_text(text: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// Returns a stable key for a raw font byte slice.
#[cfg(target_os = "windows")]
pub fn font_key_for_bytes(bytes: &[u8]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

/// Draws a prepared text layout directly into an HWND using Direct2D.
#[cfg(target_os = "windows")]
pub fn draw_text_to_hwnd(
    hwnd: *mut core::ffi::c_void,
    size: Sizef,
    scale: f32,
    layout: &TextLayout,
    position: Pointf,
) -> Option<()> {
    if size.width <= 0.0 || size.height <= 0.0 {
        return None;
    }

    let hwnd = HWND(hwnd);
    let mut backend = D2DRenderBackend::new(hwnd)?;
    backend.set_scale(scale);
    backend.begin(size);
    layout.draw(&mut backend, position);
    backend.end();
    Some(())
}

#[cfg(not(target_os = "windows"))]
pub struct TextLayout;

#[cfg(not(target_os = "windows"))]
impl TextLayout {
    pub fn new(_text: &str, _font_size: f32) -> Self {
        Self
    }

    pub fn with_font(_text: &str, _font_size: f32, _font_bytes: &[u8]) -> Self {
        Self
    }

    pub fn size(&self) -> Sizef {
        Sizef::new(0.0, 0.0)
    }

    pub fn baseline(&self) -> f32 {
        0.0
    }

    pub fn draw(&self, _backend: &mut D2DRenderBackend, _position: Pointf) {}
}

#[cfg(not(target_os = "windows"))]
pub struct TextCache {
    _private: (),
}

#[cfg(not(target_os = "windows"))]
impl Default for TextCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_os = "windows"))]
impl TextCache {
    pub fn new() -> Self {
        Self { _private: () }
    }

    pub fn get(&self, _text: &str, _font_size: f32, _font_key: u64) -> Option<&TextLayout> {
        None
    }

    pub fn insert(&mut self, _text: &str, _font_size: f32, _font_key: u64, _layout: TextLayout) {}

    pub fn get_or_insert(
        &mut self,
        _text: &str,
        _font_size: f32,
        _font_key: u64,
        build: impl FnOnce() -> TextLayout,
    ) -> &TextLayout {
        Box::leak(Box::new(build()))
    }
}

#[cfg(not(target_os = "windows"))]
pub fn font_key_for_bytes(_bytes: &[u8]) -> u64 {
    0
}

#[cfg(not(target_os = "windows"))]
pub fn draw_text_to_hwnd(
    _hwnd: *mut core::ffi::c_void,
    _size: Sizef,
    _scale: f32,
    _layout: &TextLayout,
    _position: Pointf,
) -> Option<()> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_layout_exports() {
        let _ = core::mem::size_of::<TextLayout>();
        let _ = core::mem::size_of::<TextCache>();
    }
}
