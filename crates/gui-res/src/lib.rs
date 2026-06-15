#![deny(unsafe_code)]

pub mod bundle;
pub mod generated;
pub mod id;

pub use bundle::{EmbeddedBundle, ResourceBundle};
pub use id::ResourceId;
