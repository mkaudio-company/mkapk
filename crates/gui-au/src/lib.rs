//! Audio Unit plugin format wrapper providing a Cocoa UI view around
//! `gui-host::PluginEditor`.
#![allow(unexpected_cfgs)]

#[cfg(all(feature = "au", target_os = "macos"))]
#[macro_use]
extern crate objc;

#[cfg(feature = "au")]
pub mod editor;
#[cfg(feature = "au")]
pub use editor::AuEditor;
