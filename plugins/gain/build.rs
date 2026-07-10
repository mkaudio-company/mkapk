/// Mirrors `gui-vst3`'s own `build.rs`: `cargo:rustc-cfg` set by one
/// crate's build script only applies to that crate's own compilation, not
/// downstream dependents, so this crate needs the exact same
/// `VST3_SDK_PATH` check to know, at *its own* compile time, whether
/// `gui_vst3::vst3_entry!` was actually exported by the `gui-vst3` it's
/// linking against -- without this, building with `--features vst3` but no
/// `VST3_SDK_PATH` fails with "cannot find macro `vst3_entry`" instead of
/// gracefully compiling without it.
fn main() {
    println!("cargo:rerun-if-env-changed=VST3_SDK_PATH");
    println!("cargo:rustc-check-cfg=cfg(vst3_sdk)");
    if std::env::var("VST3_SDK_PATH").is_ok() {
        println!("cargo:rustc-cfg=vst3_sdk");
    }
}
