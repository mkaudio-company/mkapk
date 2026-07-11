//! macOS platform backend: AppKit/NSView windowing, CoreGraphics rendering,
//! CoreText text, and Metal GPU surface support.
#![allow(unexpected_cfgs, deprecated)]

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

pub mod render;
pub use render::{
    CoreGraphicsRenderBackend, render_text_to_view, render_to_view, render_to_view_with_registries,
};

pub mod image;
pub use image::ImageRegistry;

pub mod text;
pub use text::{TextLayout, TextRegistry};

pub mod window;
pub use window::MacWindow;

pub mod paint_view;
pub use paint_view::{
    InputSink, attach_paint_view, clear_input_sink, resize_paint_view, set_input_sink,
    update_paint_view,
};

pub mod accessibility;
pub use accessibility::{AccessibilityElementHandle, attach_to_view, build_accessibility_tree};

#[cfg(feature = "gpu-surface")]
pub mod gpu;
#[cfg(feature = "gpu-surface")]
pub use gpu::{clear_to_color, render_gpu_surface_to_view};

pub fn gui_mac_ready() -> bool {
    true
}
