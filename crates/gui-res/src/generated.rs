use crate::{EmbeddedBundle, ResourceId};

pub static ASSET_TEST_PNG: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/test.png"));

pub static EMBEDDED: EmbeddedBundle =
    EmbeddedBundle::new(&[(ResourceId::from_bytes_le(b"test.png"), ASSET_TEST_PNG)]);
