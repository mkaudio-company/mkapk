//! AAX build helper for `cargo xtask bundle-aax`.
//!
//! `AAX_SDK_PATH` (already read by `gui-aax`'s build.rs, and re-checked by
//! `plugins/gain`'s own build.rs -- see its comment) gates the real AAX
//! entry point in. `AAX_SDK_CMAKE_DIR` is the *installed* SDK package
//! directory containing `AAX_SDKConfig.cmake` (produced by the AAX SDK's
//! own `cmake --install`), used so this crate's C++ shim
//! (`crates/gui-aax/cpp/`) can `find_package(AAX_SDK REQUIRED)`. Falls back
//! to `<AAX_SDK_PATH>/INSTALL` if unset, which is where this workspace's own
//! from-source SDK build was installed.
//!
//! `AAX_VALIDATOR_FRAMEWORKS_DIR` (the `Frameworks/` directory of a real AAX
//! Plug-In Validator install, e.g. Avid's `aax-validator` package) drives
//! genuine validation via `AAXValidator.framework`'s C API (see
//! `cpp/aaxval_harness.cpp`) -- reported honestly when unset.
//!
//! Unlike VST3/AU (a single `cargo build` producing a cdylib that
//! `bundle.rs` wraps in a hand-assembled bundle), AAX needs several steps:
//! `cargo build` produces a Rust *staticlib* exporting the generic
//! `gui_aax_*` bridge functions; a small plugin-owned binary (the plugin's
//! `<slug>-aax-page-table`) generates this specific plugin's page-table XML
//! from the same parameter metadata; and the AAX SDK's own CMake tooling
//! (`aax_plugin()` in `crates/gui-aax/cpp/CMakeLists.txt`) compiles the
//! generic C++ shim, links it against that staticlib, and assembles the
//! real `.aaxplugin` bundle. Which `plugins/<slug>` crate is targeted comes
//! from [`PluginTarget::resolve`] (`PLUGIN_CRATE` env var, default `"gain"`),
//! same as `bundle.rs`.
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::bundle::BundleStatus;
use crate::plugin_target::PluginTarget;
use crate::setup::{resolve_company_name, resolve_plugin_name, slugify};

pub fn bundle_aax() -> ExitCode {
    bundle_aax_status().into_exit_code()
}

pub fn bundle_aax_status() -> BundleStatus {
    if !cfg!(target_os = "macos") {
        return BundleStatus::Skip(
            ".aaxplugin is a macOS/Windows bundle format; this workspace only assembles it on \
             macOS today."
                .to_string(),
        );
    }

    let Some(aax_sdk_path) = env::var("AAX_SDK_PATH").ok() else {
        return BundleStatus::Skip(
            "AAX_SDK_PATH not set; gui-aax builds as a no-op stub and there is no SDK to build \
             the C++ shim against."
                .to_string(),
        );
    };
    println!("AAX_SDK_PATH={aax_sdk_path}");

    let cmake_dir =
        env::var("AAX_SDK_CMAKE_DIR").unwrap_or_else(|_| format!("{aax_sdk_path}/INSTALL"));
    if !Path::new(&cmake_dir).exists() {
        return BundleStatus::Fail(format!(
            "AAX_SDK_CMAKE_DIR (or the default <AAX_SDK_PATH>/INSTALL) does not exist: \
             {cmake_dir}. Run `cmake --install` on the AAX SDK first (see gui-aax's README)."
        ));
    }

    let target = PluginTarget::resolve();
    let plugin_name = resolve_plugin_name(&crate::new_plugin::titlecase(&target.slug));
    let company_name = resolve_company_name();

    println!(
        "Running: cargo build -p {} --features aax",
        target.package_name
    );
    let status = Command::new("cargo")
        .env("AAX_SDK_PATH", &aax_sdk_path)
        .args(["build", "-p", &target.package_name, "--features", "aax"])
        .status()
        .expect("failed to run cargo build");
    if !status.success() {
        return BundleStatus::Fail(format!(
            "cargo build -p {} --features aax did not succeed",
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

    let cpp_dir = workspace_root.join("crates/gui-aax/cpp");

    println!(
        "Running: cargo run -p {} --bin {} --features aax",
        target.package_name, target.aax_page_table_bin
    );
    let page_table_output = Command::new("cargo")
        .args([
            "run",
            "-p",
            &target.package_name,
            "--bin",
            &target.aax_page_table_bin,
            "--features",
            "aax",
        ])
        .output()
        .expect("failed to run the plugin's aax-page-table binary");
    if !page_table_output.status.success() {
        return BundleStatus::Fail(format!(
            "cargo run -p {} --bin {} did not succeed",
            target.package_name, target.aax_page_table_bin
        ));
    }
    let generated_pages_path = cpp_dir.join("GeneratedPages.xml");
    if let Err(error) = fs::write(&generated_pages_path, &page_table_output.stdout) {
        return BundleStatus::Fail(format!(
            "failed to write {}: {error}",
            generated_pages_path.display()
        ));
    }

    let build_dir = workspace_root.join("target/aax-build");
    let bundle_identifier = format!(
        "com.{}.aax.{}",
        slugify(&company_name),
        slugify(&target.slug)
    );

    println!(
        "Running: cmake -S {} -B {} -DCMAKE_PREFIX_PATH={} -DGUI_AAX_RUST_STATICLIB={} \
         -DAAX_SDK_SOURCE_DIR={} -DGUI_AAX_OUTPUT_NAME={} -DGUI_AAX_BUNDLE_IDENTIFIER={} \
         -DCMAKE_BUILD_TYPE=Release",
        cpp_dir.display(),
        build_dir.display(),
        cmake_dir,
        staticlib_path.display(),
        aax_sdk_path,
        plugin_name,
        bundle_identifier
    );
    let configure_status = Command::new("cmake")
        .arg("-S")
        .arg(&cpp_dir)
        .arg("-B")
        .arg(&build_dir)
        .arg(format!("-DCMAKE_PREFIX_PATH={cmake_dir}"))
        .arg(format!(
            "-DGUI_AAX_RUST_STATICLIB={}",
            staticlib_path.display()
        ))
        .arg(format!("-DAAX_SDK_SOURCE_DIR={aax_sdk_path}"))
        .arg(format!("-DGUI_AAX_OUTPUT_NAME={plugin_name}"))
        .arg(format!("-DGUI_AAX_BUNDLE_IDENTIFIER={bundle_identifier}"))
        .arg(format!(
            "-DGUI_AAX_COPYRIGHT_STRING=Copyright 2026 {company_name}"
        ))
        .arg("-DCMAKE_BUILD_TYPE=Release")
        .status()
        .expect("failed to run cmake configure");
    if !configure_status.success() {
        return BundleStatus::Fail("cmake configure of crates/gui-aax/cpp did not succeed".into());
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
        return BundleStatus::Fail("cmake --build of crates/gui-aax/cpp did not succeed".into());
    }

    let bundle_path = build_dir.join(format!("{plugin_name}.aaxplugin"));
    if !bundle_path.exists() {
        return BundleStatus::Fail(format!(
            "expected assembled AAX bundle at {}",
            bundle_path.display()
        ));
    }
    println!("Assembled bundle: {}", bundle_path.display());

    match env::var("AAX_VALIDATOR_FRAMEWORKS_DIR").ok() {
        Some(frameworks_dir) => run_real_validator(&workspace_root, &frameworks_dir, &bundle_path),
        None => BundleStatus::Skip(format!(
            "AAX bundle built with a real AAX_CMain entry point at {}. \
             AAX_VALIDATOR_FRAMEWORKS_DIR not set; set it to a real AAX Plug-In Validator \
             install's Frameworks/ directory to have this run through the real validator.",
            bundle_path.display()
        )),
    }
}

/// Builds `cpp/aaxval_harness.cpp` against a real AAX Plug-In Validator
/// install's `AAXValidator.framework` and runs it against `bundle_path`.
///
/// The framework's own binary has `@executable_path/../Frameworks` baked
/// into its install name (not overridable via rpath), so the harness must
/// physically live in a directory whose *parent* contains a `Frameworks`
/// folder (matching the vendor's own layout: `CommandLineTools/dsh` next to
/// a sibling `Frameworks/`) -- i.e. the harness goes in `.../bin/`, one
/// level below where the `Frameworks` symlink lives, not next to it.
/// Rather than writing into the vendor's own install tree, this creates
/// that `Frameworks` symlink inside our own `target/` pointing at
/// `frameworks_dir` and builds the harness a directory below it.
fn run_real_validator(
    workspace_root: &Path,
    frameworks_dir: &str,
    bundle_path: &Path,
) -> BundleStatus {
    let harness_root = workspace_root.join("target/aax-validator-harness");
    let harness_bin_dir = harness_root.join("bin");
    if let Err(error) = fs::create_dir_all(&harness_bin_dir) {
        return BundleStatus::Fail(format!(
            "failed to create {}: {error}",
            harness_bin_dir.display()
        ));
    }
    let frameworks_link = harness_root.join("Frameworks");
    if !frameworks_link.exists() {
        #[cfg(unix)]
        if let Err(error) = std::os::unix::fs::symlink(frameworks_dir, &frameworks_link) {
            return BundleStatus::Fail(format!(
                "failed to symlink {} -> {frameworks_dir}: {error}",
                frameworks_link.display()
            ));
        }
    }

    let harness_source = workspace_root.join("crates/gui-aax/cpp/aaxval_harness.cpp");
    let harness_binary = harness_bin_dir.join("aaxval_harness");
    let headers_dir = format!("{frameworks_dir}/AAXValidator.framework/Versions/A/Headers");

    println!(
        "Running: clang++ -o {} {}",
        harness_binary.display(),
        harness_source.display()
    );
    let compile_status = Command::new("clang++")
        .args(["-std=c++17"])
        .arg("-F")
        .arg(frameworks_dir)
        .arg("-I")
        .arg(&headers_dir)
        .args(["-framework", "AAXValidator"])
        .args(["-Wl,-rpath,@executable_path/../Frameworks"])
        .arg("-o")
        .arg(&harness_binary)
        .arg(&harness_source)
        .status()
        .expect("failed to run clang++");
    if !compile_status.success() {
        return BundleStatus::Fail("failed to compile cpp/aaxval_harness.cpp".to_string());
    }

    println!(
        "Running: {} {}",
        harness_binary.display(),
        bundle_path.display()
    );
    let run_output = Command::new(&harness_binary)
        .arg(bundle_path)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("failed to run aaxval_harness");
    let stdout = String::from_utf8_lossy(&run_output.stdout);
    let stderr = String::from_utf8_lossy(&run_output.stderr);
    println!("{stdout}");
    if !stderr.is_empty() {
        println!("stderr: {stderr}");
    }

    let summary = stdout
        .lines()
        .find(|line| line.starts_with("AAXVAL_SUMMARY"))
        .unwrap_or("AAXVAL_SUMMARY (not found)");

    if run_output.status.success() {
        BundleStatus::Pass(format!(
            "Real AAX Plug-In Validator: {summary} against {}",
            bundle_path.display()
        ))
    } else {
        BundleStatus::Fail(format!(
            "Real AAX Plug-In Validator: {summary} against {}",
            bundle_path.display()
        ))
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask should live under the workspace root")
        .to_path_buf()
}
