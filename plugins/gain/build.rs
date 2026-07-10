/// Mirrors `gui-vst3`'s and `gui-aax`'s own `build.rs`: `cargo:rustc-cfg`
/// set by one crate's build script only applies to that crate's own
/// compilation, not downstream dependents, so this crate needs the exact
/// same `VST3_SDK_PATH`/`AAX_SDK_PATH` checks to know, at *its own* compile
/// time, whether `gui_vst3::vst3_entry!`/`gui_aax::aax_entry!` were
/// actually exported by the `gui-vst3`/`gui-aax` it's linking against --
/// without this, building with `--features vst3`/`aax` but no matching
/// `_SDK_PATH` fails with "cannot find macro" instead of gracefully
/// compiling without it.
fn main() {
    println!("cargo:rerun-if-env-changed=VST3_SDK_PATH");
    println!("cargo:rerun-if-env-changed=AAX_SDK_PATH");
    println!("cargo:rustc-check-cfg=cfg(vst3_sdk)");
    println!("cargo:rustc-check-cfg=cfg(aax_sdk)");
    if std::env::var("VST3_SDK_PATH").is_ok() {
        println!("cargo:rustc-cfg=vst3_sdk");
    }
    if std::env::var("AAX_SDK_PATH").is_ok() {
        println!("cargo:rustc-cfg=aax_sdk");
    }
}
