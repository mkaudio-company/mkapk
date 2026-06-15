#![deny(unsafe_code)]

pub mod editor;
pub mod parameter;

pub use editor::{EditorHost, ParentWindowHandle, PluginEditor, SizeConstraints};
pub use parameter::{NormalizedValue, ParameterGateway, ParameterId, ParameterInfo};
