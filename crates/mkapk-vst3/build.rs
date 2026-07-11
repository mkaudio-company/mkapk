use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=thirdparty/pluginterfaces/base/funknown.h");
    println!("cargo:rustc-check-cfg=cfg(vst3_sdk)");

    // The vendored VST3 SDK submodules (`thirdparty/{pluginterfaces,base,
    // public.sdk}`, all MIT-licensed) are always present in this repo, but a
    // fresh clone doesn't populate them until `git submodule update --init`
    // runs -- this checks for that rather than assuming it, so the crate
    // still builds (as a view-only stub) if a contributor forgot.
    let marker =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("thirdparty/pluginterfaces/base/funknown.h");
    if marker.exists() {
        println!("cargo:rustc-cfg=vst3_sdk");
    } else {
        println!(
            "cargo:warning=crates/mkapk-vst3/thirdparty submodules not populated; building \
             mkapk-vst3 as a view-only stub (no real plugin entry point). Run `git submodule \
             update --init crates/mkapk-vst3/thirdparty/pluginterfaces \
             crates/mkapk-vst3/thirdparty/base crates/mkapk-vst3/thirdparty/public.sdk` to fix."
        );
    }
}
