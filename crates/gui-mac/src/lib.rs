#![allow(unexpected_cfgs)]

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

pub mod window;
pub use window::MacWindow;

pub fn gui_mac_ready() -> bool {
    true
}
