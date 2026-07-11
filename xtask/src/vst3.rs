//! VST3 build helper for `cargo xtask bundle-vst3`.
//!
//! Unlike the previous pure-Rust `vst3-sys`-based implementation (a plain
//! cdylib `bundle.rs` could wrap directly), the real VST3 entry point is
//! now a C++ shim (`crates/mkapk-vst3/cpp/`) built against the vendored,
//! MIT-licensed Steinberg VST3 SDK (`crates/mkapk-vst3/thirdparty/`) --
//! mirroring `aax.rs`: `cargo build` produces a Rust *staticlib* exporting
//! the generic `mkapk_vst3_*` bridge functions, then CMake compiles the
//! shim, links it against that staticlib, and produces a shared library
//! this module wraps in the actual `.vst3` bundle shape (reusing
//! `bundle.rs`'s existing Info.plist/PkgInfo/codesign assembly, since we
//! don't vendor the VST3 SDK's own `cmake/` bundle-assembly helpers --
//! see `crates/mkapk-vst3/cpp/CMakeLists.txt`).
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::bundle::{BundleStatus, assemble_cdylib_bundle};
use crate::plugin_target::PluginTarget;

pub fn bundle_vst3() -> std::process::ExitCode {
    bundle_vst3_status().into_exit_code()
}

pub fn bundle_vst3_status() -> BundleStatus {
    if !cfg!(target_os = "macos") {
        return BundleStatus::Skip(
            ".vst3 is a macOS/Windows bundle format; this workspace only assembles it on macOS \
             today."
                .to_string(),
        );
    }

    let sdk_marker =
        workspace_root().join("crates/mkapk-vst3/thirdparty/pluginterfaces/base/funknown.h");
    if !sdk_marker.exists() {
        return BundleStatus::Skip(
            "crates/mkapk-vst3/thirdparty submodules not populated; run `git submodule update \
             --init crates/mkapk-vst3/thirdparty/pluginterfaces crates/mkapk-vst3/thirdparty/base \
             crates/mkapk-vst3/thirdparty/public.sdk` first."
                .to_string(),
        );
    }

    let target = PluginTarget::resolve();

    println!(
        "Running: cargo build -p {} --features vst3",
        target.package_name
    );
    let status = Command::new("cargo")
        .args(["build", "-p", &target.package_name, "--features", "vst3"])
        .status()
        .expect("failed to run cargo build");
    if !status.success() {
        return BundleStatus::Fail(format!(
            "cargo build -p {} --features vst3 did not succeed",
            target.package_name
        ));
    }

    let workspace_root = workspace_root();
    let staticlib_path = workspace_root.join(format!("target/debug/lib{}.a", target.lib_name));
    if !staticlib_path.exists() {
        return BundleStatus::Fail(format!(
            "expected built Rust staticlib at {}",
            staticlib_path.display()
        ));
    }

    let cpp_dir = workspace_root.join("crates/mkapk-vst3/cpp");
    let build_dir = workspace_root.join("target/vst3-build");
    let plugin_name =
        crate::setup::resolve_plugin_name(&crate::new_plugin::titlecase(&target.slug));

    println!(
        "Running: cmake -S {} -B {} -DGUI_VST3_RUST_STATICLIB={} -DGUI_VST3_OUTPUT_NAME={} \
         -DCMAKE_BUILD_TYPE=Release",
        cpp_dir.display(),
        build_dir.display(),
        staticlib_path.display(),
        plugin_name
    );
    let configure_status = Command::new("cmake")
        .arg("-S")
        .arg(&cpp_dir)
        .arg("-B")
        .arg(&build_dir)
        .arg(format!(
            "-DGUI_VST3_RUST_STATICLIB={}",
            staticlib_path.display()
        ))
        .arg(format!("-DGUI_VST3_OUTPUT_NAME={plugin_name}"))
        .arg("-DCMAKE_BUILD_TYPE=Release")
        .status()
        .expect("failed to run cmake configure");
    if !configure_status.success() {
        return BundleStatus::Fail(
            "cmake configure of crates/mkapk-vst3/cpp did not succeed".into(),
        );
    }

    println!(
        "Running: cmake --build {} --config Release -j 8",
        build_dir.display()
    );
    let build_status = Command::new("cmake")
        .args(["--build"])
        .arg(&build_dir)
        .args(["--config", "Release", "-j", "8"])
        .status()
        .expect("failed to run cmake --build");
    if !build_status.success() {
        return BundleStatus::Fail("cmake --build of crates/mkapk-vst3/cpp did not succeed".into());
    }

    let dylib_path = build_dir.join(format!("{plugin_name}.dylib"));
    if !dylib_path.exists() {
        return BundleStatus::Fail(format!(
            "expected built VST3 shared library at {}",
            dylib_path.display()
        ));
    }

    assemble_cdylib_bundle(
        &dylib_path,
        &plugin_name,
        "vst3",
        "GetPluginFactory",
        "VST3",
        "",
    )
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask should live under the workspace root")
        .to_path_buf()
}
