//! Host abstraction traits for plugin editors and a lock-free parameter gateway
//! between the UI and audio threads.
#![deny(unsafe_code)]

pub mod editor;
pub mod meter;
pub mod parameter;
pub mod processor;

pub use editor::{EditorHost, ParentWindowHandle, PluginEditor, SizeConstraints};
pub use meter::PeakMeter;
pub use parameter::{
    LockFreeParameterGateway, NormalizedValue, ParameterGateway, ParameterId, ParameterInfo,
    ParameterMessage,
};
pub use processor::{ChannelLayout, Processor, apply_pending_parameters};
