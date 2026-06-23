//! macOS platform backend: AppKit/NSView windowing, CoreGraphics rendering,
//! CoreText text, and Metal GPU surface support.
#![allow(unexpected_cfgs, deprecated)]

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

pub mod render;
pub use render::{CoreGraphicsRenderBackend, render_text_to_view, render_to_view};

pub mod text;
pub use text::TextLayout;

pub mod window;
pub use window::MacWindow;

#[cfg(feature = "gpu-surface")]
pub mod gpu;
#[cfg(feature = "gpu-surface")]
pub use gpu::{clear_to_color, render_gpu_surface_to_view};

pub fn gui_mac_ready() -> bool {
    true
}
