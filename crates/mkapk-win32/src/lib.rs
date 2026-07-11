//! Windows platform backend: Win32 windowing, Direct2D rendering, DirectWrite
//! text, and D3D11 GPU surface support.
#![allow(unsafe_op_in_unsafe_fn)]

pub mod image;
pub mod render;
pub mod text;
pub mod window;

#[cfg(feature = "gpu-surface")]
pub mod gpu;

#[cfg(feature = "gpu-surface")]
pub use gpu::{clear_render_target, render_gpu_surface_to_hwnd};
pub use image::ImageRegistry;
pub use render::{D2DRenderBackend, render_to_hwnd, render_to_hwnd_with_registries};
pub use text::{TextCache, TextLayout, TextRegistry, draw_text_to_hwnd, font_key_for_bytes};
pub use window::Win32Window;
