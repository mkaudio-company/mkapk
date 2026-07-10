//! macOS plugin bundle assembly and code signing for `cargo xtask bundle-*`.
//!
//! These commands package the existing test-host `gain` examples into
//! standard macOS plugin bundle shapes (.vst3 / .component) and code-sign
//! them. Note: neither `gui-vst3` nor `gui-au` currently exports a real
//! plugin factory entry point (VST3 `GetPluginFactory` / AU component
//! factory function), so the resulting bundles are packaging/signing
//! plumbing only -- they are not yet scannable/loadable by a real DAW.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::setup::{resolve_codesign_identity, resolve_company_name, resolve_plugin_name, slugify};

struct PluginSpec {
    crate_name: &'static str,
    example_name: &'static str,
    bundle_extension: &'static str,
    package_type: &'static str,
}

pub fn bundle_vst3() -> ExitCode {
    bundle_macos_plugin(PluginSpec {
        crate_name: "gui-vst3",
        example_name: "gain",
        bundle_extension: "vst3",
        package_type: "BNDL",
    })
}

pub fn bundle_au() -> ExitCode {
    bundle_macos_plugin(PluginSpec {
        crate_name: "gui-au",
        example_name: "gain",
        bundle_extension: "component",
        package_type: "BNDL",
    })
}

fn bundle_macos_plugin(spec: PluginSpec) -> ExitCode {
    if !cfg!(target_os = "macos") {
        println!(
            "bundle-{}: .{} is a macOS bundle format; skipping on this platform.",
            spec.bundle_extension, spec.bundle_extension
        );
        return ExitCode::from(0);
    }

    println!(
        "Running: cargo build -p {} --example {}",
        spec.crate_name, spec.example_name
    );
    let status = Command::new("cargo")
        .args([
            "build",
            "-p",
            spec.crate_name,
            "--example",
            spec.example_name,
        ])
        .status()
        .expect("failed to run cargo build");
    if !status.success() {
        println!(
            "FAIL: cargo build -p {} --example {}",
            spec.crate_name, spec.example_name
        );
        return ExitCode::from(1);
    }

    let workspace_root = workspace_root();
    let example_binary = workspace_root
        .join("target/debug/examples")
        .join(spec.example_name);
    if !example_binary.exists() {
        println!(
            "FAIL: expected built example at {}",
            example_binary.display()
        );
        return ExitCode::from(1);
    }

    let plugin_name = resolve_plugin_name();
    let company_name = resolve_company_name();
    let bundle_id = format!(
        "com.{}.{}.{}",
        slugify(&company_name),
        slugify(&plugin_name),
        spec.bundle_extension
    );

    let out_dir = workspace_root.join("target/bundles");
    fs::create_dir_all(&out_dir).expect("failed to create target/bundles");
    let bundle_path = out_dir.join(format!("{plugin_name}.{}", spec.bundle_extension));
    if bundle_path.exists() {
        fs::remove_dir_all(&bundle_path).expect("failed to remove stale bundle");
    }

    let contents = bundle_path.join("Contents");
    let macos_dir = contents.join("MacOS");
    fs::create_dir_all(&macos_dir).expect("failed to create bundle Contents/MacOS");

    let exe_name = &plugin_name;
    fs::copy(&example_binary, macos_dir.join(exe_name)).expect("failed to copy example binary");

    fs::write(
        contents.join("Info.plist"),
        info_plist(exe_name, &plugin_name, &bundle_id, spec.package_type),
    )
    .expect("failed to write Info.plist");
    fs::write(
        contents.join("PkgInfo"),
        format!("{}????", spec.package_type),
    )
    .expect("failed to write PkgInfo");

    println!("Assembled bundle: {}", bundle_path.display());

    match resolve_codesign_identity() {
        Some(identity) => {
            println!(
                "Running: codesign --force --deep --sign \"{identity}\" {}",
                bundle_path.display()
            );
            let status = Command::new("codesign")
                .args(["--force", "--deep", "--sign", &identity])
                .arg(&bundle_path)
                .status()
                .expect("failed to run codesign");
            if !status.success() {
                println!("FAIL: codesign");
                return ExitCode::from(1);
            }

            let verify = Command::new("codesign")
                .args(["--verify", "--verbose"])
                .arg(&bundle_path)
                .status()
                .expect("failed to run codesign --verify");
            if verify.success() {
                println!(
                    "PASS: bundle assembled and code-signed at {}",
                    bundle_path.display()
                );
            } else {
                println!("FAIL: codesign --verify did not pass");
                return ExitCode::from(1);
            }
        }
        None => {
            println!(
                "SKIP: code signing skipped; bundle assembled unsigned at {}",
                bundle_path.display()
            );
        }
    }

    println!(
        "note: this bundle wraps the {} test-host example only. {} does not yet export a real \
         plugin factory entry point, so the bundle is packaging/signing plumbing, not a \
         DAW-scannable plugin.",
        spec.example_name, spec.crate_name
    );

    ExitCode::from(0)
}

fn info_plist(exe_name: &str, display_name: &str, bundle_id: &str, package_type: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>{exe_name}</string>
    <key>CFBundleIdentifier</key>
    <string>{bundle_id}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>{display_name}</string>
    <key>CFBundlePackageType</key>
    <string>{package_type}</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>CFBundleSignature</key>
    <string>????</string>
</dict>
</plist>
"#
    )
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask should live under the workspace root")
        .to_path_buf()
}
