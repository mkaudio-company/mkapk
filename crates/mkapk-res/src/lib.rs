//! Resource system: resource IDs, static embedding, typed registry, and SVG/PNG
//! decoders.
#![deny(unsafe_code)]

pub mod bundle;
pub mod generated;
pub mod id;
pub mod png;
pub mod registry;
pub mod svg;

pub use bundle::{EmbeddedBundle, ResourceBundle};
pub use id::ResourceId;
pub use png::PngImage;
pub use registry::{Font, Image, Resource, ResourceHandle, ResourceRegistry, Svg};
pub use svg::SvgImage;
