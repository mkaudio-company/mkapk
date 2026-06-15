#![allow(unexpected_cfgs, deprecated)]

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

pub mod render;
pub use render::{CoreGraphicsRenderBackend, render_to_view};

pub mod window;
pub use window::MacWindow;

pub fn gui_mac_ready() -> bool {
    true
}
