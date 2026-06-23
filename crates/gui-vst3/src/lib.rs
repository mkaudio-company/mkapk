//! VST3 plugin format wrapper providing an `IPlugView` implementation around
//! `gui-host::PluginEditor`.
#[cfg(feature = "vst3")]
pub mod view;
#[cfg(feature = "vst3")]
pub use view::PluginView;
