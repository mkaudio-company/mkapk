use crate::{EmbeddedBundle, ResourceId};

pub static ASSET_KNOB_SVG: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/knob.svg"));

pub static ASSET_LOGO_PNG: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/logo.png"));

pub static ASSET_TEST_PNG: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/test.png"));

pub static EMBEDDED: EmbeddedBundle = EmbeddedBundle::new(&[
    (ResourceId::from_bytes_le(b"knob.svg"), ASSET_KNOB_SVG),
    (ResourceId::from_bytes_le(b"logo.png"), ASSET_LOGO_PNG),
    (ResourceId::from_bytes_le(b"test.png"), ASSET_TEST_PNG),
]);
