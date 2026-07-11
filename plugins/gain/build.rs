/// Mirrors `mkapk-vst3`'s and `mkapk-aax`'s own `build.rs`: `cargo:rustc-cfg`
/// set by one crate's build script only applies to that crate's own
/// compilation, not downstream dependents, so this crate needs the exact
/// same checks to know, at *its own* compile time, whether
/// `mkapk_vst3::vst3_entry!`/`mkapk_aax::aax_entry!` were actually exported
/// by the `mkapk-vst3`/`mkapk-aax` it's linking against -- without this,
/// building with `--features vst3`/`aax` but the SDK not available fails
/// with "cannot find macro" instead of gracefully compiling without it.
fn main() {
    println!(
        "cargo:rerun-if-changed=../../crates/mkapk-vst3/thirdparty/pluginterfaces/base/funknown.h"
    );
    println!("cargo:rerun-if-env-changed=AAX_SDK_PATH");
    println!("cargo:rustc-check-cfg=cfg(vst3_sdk)");
    println!("cargo:rustc-check-cfg=cfg(aax_sdk)");
    // Mirrors `mkapk-vst3/build.rs`'s own vendored-submodule check (the
    // path is relative to *this* crate's manifest dir, two levels up from
    // `plugins/gain`).
    let vst3_sdk_marker = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/mkapk-vst3/thirdparty/pluginterfaces/base/funknown.h");
    if vst3_sdk_marker.exists() {
        println!("cargo:rustc-cfg=vst3_sdk");
    }
    if std::env::var("AAX_SDK_PATH").is_ok() {
        println!("cargo:rustc-cfg=aax_sdk");
    }
}
