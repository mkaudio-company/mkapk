//! macOS plugin bundle assembly and code signing for `cargo xtask bundle-*`.
//!
//! These commands package `plugins/gain`'s built cdylib (`libgain_plugin.dylib`,
//! built from the workspace root as a normal `cargo build -p gain-plugin
//! --features <format>`) into standard macOS plugin bundle shapes
//! (`.vst3` / `.component`) and code-sign them. Each format's real plugin
//! entry point is gated (VST3 by `VST3_SDK_PATH` at `gui-vst3` build time;
//! AU is ungated on macOS) -- when the gate isn't set, the crate still
//! builds, but exports no factory symbol, so the assembled bundle is
//! packaging/signing plumbing only, not yet DAW-scannable. This is checked
//! for real (via `nm`) rather than assumed, so the reported status is
//! honest either way.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::setup::{resolve_codesign_identity, resolve_company_name, resolve_plugin_name, slugify};

/// The outcome of one `bundle-*` command, detailed enough for
/// [`crate::bundle_all`]'s summary table.
pub enum BundleStatus {
    Pass(String),
    Skip(String),
    Fail(String),
}

impl BundleStatus {
    pub fn into_exit_code(self) -> ExitCode {
        match self {
            BundleStatus::Pass(msg) => {
                println!("PASS: {msg}");
                ExitCode::from(0)
            }
            BundleStatus::Skip(msg) => {
                println!("SKIP: {msg}");
                ExitCode::from(0)
            }
            BundleStatus::Fail(msg) => {
                println!("FAIL: {msg}");
                ExitCode::from(1)
            }
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            BundleStatus::Pass(_) => "PASS",
            BundleStatus::Skip(_) => "SKIP",
            BundleStatus::Fail(_) => "FAIL",
        }
    }

    pub fn detail(&self) -> &str {
        match self {
            BundleStatus::Pass(msg) | BundleStatus::Skip(msg) | BundleStatus::Fail(msg) => msg,
        }
    }
}

struct CdylibPluginSpec {
    /// `plugins/gain`'s cargo feature that gates this format's real entry
    /// point in (`vst3` or `au`).
    feature: &'static str,
    bundle_extension: &'static str,
    package_type: &'static str,
    /// Name of the exported C symbol that only exists when the format's
    /// real entry point actually compiled in (e.g. `GetPluginFactory`,
    /// `gain_au_factory`); used to check honestly whether this bundle is
    /// really loadable, rather than assuming it from the feature flag
    /// alone.
    entry_symbol: &'static str,
    format_name: &'static str,
    /// Additional `Info.plist` dict entries this format needs (e.g. AU's
    /// `AudioComponents` array), inserted verbatim just before `</dict>`.
    info_plist_extra: fn(plugin_name: &str, company_name: &str) -> String,
}

pub fn bundle_vst3() -> ExitCode {
    bundle_vst3_status().into_exit_code()
}

pub fn bundle_vst3_status() -> BundleStatus {
    bundle_cdylib_plugin(CdylibPluginSpec {
        feature: "vst3",
        bundle_extension: "vst3",
        package_type: "BNDL",
        entry_symbol: "GetPluginFactory",
        format_name: "VST3",
        info_plist_extra: |_, _| String::new(),
    })
}

pub fn bundle_au() -> ExitCode {
    bundle_au_status().into_exit_code()
}

pub fn bundle_au_status() -> BundleStatus {
    bundle_cdylib_plugin(CdylibPluginSpec {
        feature: "au",
        bundle_extension: "component",
        package_type: "BNDL",
        entry_symbol: "gain_au_factory",
        format_name: "Audio Unit",
        info_plist_extra: au_components_plist,
    })
}

/// The `AudioComponents` array AU hosts read to discover this plugin
/// without loading it first: type/subtype/manufacturer four-character
/// codes and the `factoryFunction` symbol name, which must match
/// `plugins/gain`'s `gui_au::au_entry!` invocation exactly.
///
/// The manufacturer code must contain at least one non-lowercase
/// character -- `auval` fails validation with "Manufacturer OSType should
/// have at least one non-lower case character" otherwise (confirmed by
/// running it against an all-lowercase `mkau`).
///
/// `version` must be a plist `<integer>`, not a `<string>` -- confirmed via
/// `log show`'s `AudioComponentRegistrar` output ("trouble parsing
/// Info.plist's AudioComponents, key version") after the registrar failed
/// to parse a string value there, which silently dropped the whole entry
/// (so the component was never registered at all, not just misdescribed).
fn au_components_plist(plugin_name: &str, company_name: &str) -> String {
    format!(
        r#"    <key>AudioComponents</key>
    <array>
        <dict>
            <key>name</key>
            <string>{company_name}: {plugin_name}</string>
            <key>description</key>
            <string>{plugin_name}</string>
            <key>factoryFunction</key>
            <string>gain_au_factory</string>
            <key>type</key>
            <string>aufx</string>
            <key>subtype</key>
            <string>gain</string>
            <key>manufacturer</key>
            <string>Mkau</string>
            <key>version</key>
            <integer>1</integer>
        </dict>
    </array>
"#
    )
}

fn bundle_cdylib_plugin(spec: CdylibPluginSpec) -> BundleStatus {
    if !cfg!(target_os = "macos") {
        return BundleStatus::Skip(format!(
            ".{} is a macOS bundle format; skipping on this platform.",
            spec.bundle_extension
        ));
    }

    println!(
        "Running: cargo build -p gain-plugin --features {}",
        spec.feature
    );
    let status = Command::new("cargo")
        .args(["build", "-p", "gain-plugin", "--features", spec.feature])
        .status()
        .expect("failed to run cargo build");
    if !status.success() {
        return BundleStatus::Fail(format!(
            "cargo build -p gain-plugin --features {} did not succeed",
            spec.feature
        ));
    }

    let workspace_root = workspace_root();
    let cdylib_path = workspace_root.join("target/debug/libgain_plugin.dylib");
    if !cdylib_path.exists() {
        return BundleStatus::Fail(format!(
            "expected built cdylib at {}",
            cdylib_path.display()
        ));
    }

    let entry_point_present = dylib_exports_symbol(&cdylib_path, spec.entry_symbol);

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
    fs::copy(&cdylib_path, macos_dir.join(exe_name)).expect("failed to copy plugin cdylib");

    let extra = (spec.info_plist_extra)(&plugin_name, &company_name);
    fs::write(
        contents.join("Info.plist"),
        info_plist(
            exe_name,
            &plugin_name,
            &bundle_id,
            spec.package_type,
            &extra,
        ),
    )
    .expect("failed to write Info.plist");
    fs::write(
        contents.join("PkgInfo"),
        format!("{}????", spec.package_type),
    )
    .expect("failed to write PkgInfo");

    println!("Assembled bundle: {}", bundle_path.display());

    if let Some(fail) = codesign_bundle(&bundle_path) {
        return BundleStatus::Fail(fail);
    }

    if entry_point_present {
        BundleStatus::Pass(format!(
            "{} bundle assembled with a real `{}` entry point at {}",
            spec.format_name,
            spec.entry_symbol,
            bundle_path.display()
        ))
    } else {
        BundleStatus::Skip(format!(
            "{} bundle assembled at {}, but its build didn't export `{}` (its SDK/feature gate \
             wasn't set), so this is packaging/signing plumbing only, not yet DAW-scannable.",
            spec.format_name,
            bundle_path.display(),
            spec.entry_symbol
        ))
    }
}

/// Builds `gain-standalone` and, on macOS, wraps it in a minimal `.app`
/// bundle. On other platforms the plain binary is already the deliverable
/// (no bundle format expected), so this just builds and reports its path.
pub fn bundle_standalone() -> ExitCode {
    bundle_standalone_status().into_exit_code()
}

pub fn bundle_standalone_status() -> BundleStatus {
    println!("Running: cargo build -p gain-plugin --bin gain-standalone --features standalone");
    let status = Command::new("cargo")
        .args([
            "build",
            "-p",
            "gain-plugin",
            "--bin",
            "gain-standalone",
            "--features",
            "standalone",
        ])
        .status()
        .expect("failed to run cargo build");
    if !status.success() {
        return BundleStatus::Fail(
            "cargo build -p gain-plugin --bin gain-standalone --features standalone did not \
             succeed"
                .to_string(),
        );
    }

    let workspace_root = workspace_root();
    let binary_name = if cfg!(target_os = "windows") {
        "gain-standalone.exe"
    } else {
        "gain-standalone"
    };
    let binary_path = workspace_root.join("target/debug").join(binary_name);
    if !binary_path.exists() {
        return BundleStatus::Fail(format!(
            "expected built standalone binary at {}",
            binary_path.display()
        ));
    }

    if !cfg!(target_os = "macos") {
        return BundleStatus::Pass(format!(
            "Standalone binary built at {}",
            binary_path.display()
        ));
    }

    let plugin_name = resolve_plugin_name();
    let company_name = resolve_company_name();
    let bundle_id = format!(
        "com.{}.{}.app",
        slugify(&company_name),
        slugify(&plugin_name)
    );

    let out_dir = workspace_root.join("target/bundles");
    fs::create_dir_all(&out_dir).expect("failed to create target/bundles");
    let bundle_path = out_dir.join(format!("{plugin_name}.app"));
    if bundle_path.exists() {
        fs::remove_dir_all(&bundle_path).expect("failed to remove stale bundle");
    }

    let contents = bundle_path.join("Contents");
    let macos_dir = contents.join("MacOS");
    fs::create_dir_all(&macos_dir).expect("failed to create bundle Contents/MacOS");

    let exe_name = &plugin_name;
    fs::copy(&binary_path, macos_dir.join(exe_name)).expect("failed to copy standalone binary");

    fs::write(
        contents.join("Info.plist"),
        info_plist(exe_name, &plugin_name, &bundle_id, "APPL", ""),
    )
    .expect("failed to write Info.plist");
    fs::write(contents.join("PkgInfo"), "APPL????").expect("failed to write PkgInfo");

    println!("Assembled bundle: {}", bundle_path.display());

    if let Some(fail) = codesign_bundle(&bundle_path) {
        return BundleStatus::Fail(fail);
    }

    BundleStatus::Pass(format!(
        "Standalone app assembled at {}",
        bundle_path.display()
    ))
}

/// Code-signs `bundle_path` if a signing identity can be resolved (see
/// `setup::resolve_codesign_identity`), leaving it unsigned otherwise.
/// Returns `Some(reason)` only if signing was attempted and failed.
fn codesign_bundle(bundle_path: &Path) -> Option<String> {
    match resolve_codesign_identity() {
        Some(identity) => {
            println!(
                "Running: codesign --force --deep --sign \"{identity}\" {}",
                bundle_path.display()
            );
            let status = Command::new("codesign")
                .args(["--force", "--deep", "--sign", &identity])
                .arg(bundle_path)
                .status()
                .expect("failed to run codesign");
            if !status.success() {
                return Some("codesign did not succeed".to_string());
            }

            let verify = Command::new("codesign")
                .args(["--verify", "--verbose"])
                .arg(bundle_path)
                .status()
                .expect("failed to run codesign --verify");
            if !verify.success() {
                return Some("codesign --verify did not pass".to_string());
            }
            println!("Code-signed and verified: {}", bundle_path.display());
            None
        }
        None => {
            println!(
                "note: code signing skipped; bundle assembled unsigned at {}",
                bundle_path.display()
            );
            None
        }
    }
}

/// Checks whether `path`'s exported dynamic symbol table (`nm -gU`)
/// contains `symbol`, rather than assuming it from whichever cargo feature
/// was requested -- SDK/feature gates can silently make a build a no-op
/// stub, and this is the one way to tell honestly.
fn dylib_exports_symbol(path: &Path, symbol: &str) -> bool {
    let Ok(output) = Command::new("nm")
        .args(["-gU", &path.display().to_string()])
        .output()
    else {
        return false;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mangled = format!("_{symbol}");
    text.lines().any(|line| line.trim_end().ends_with(&mangled))
}

fn info_plist(
    exe_name: &str,
    display_name: &str,
    bundle_id: &str,
    package_type: &str,
    extra: &str,
) -> String {
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
{extra}</dict>
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
