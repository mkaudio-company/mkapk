fn main() {
    println!("cargo:rerun-if-env-changed=VST3_SDK_PATH");
    println!("cargo:rustc-check-cfg=cfg(vst3_sdk)");
    match std::env::var("VST3_SDK_PATH") {
        Ok(_path) => {
            // `vst3-sys` is a self-contained Rust binding (git dependency);
            // it does not need a local Steinberg VST3 SDK checkout to
            // build. This env var exists purely as an explicit opt-in gate,
            // consistent with how `mkapk-aax` gates on `AAX_SDK_PATH`, so the
            // build system can enable/disable each plugin format uniformly.
            println!("cargo:rustc-cfg=vst3_sdk");
        }
        Err(_) => {
            println!(
                "cargo:warning=VST3_SDK_PATH not set; building mkapk-vst3 as a view-only stub (no real plugin entry point)."
            );
        }
    }
}
