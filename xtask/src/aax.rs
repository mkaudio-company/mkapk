//! AAX build helper for `cargo xtask bundle-aax`.
//!
//! `AAX_SDK` (already read by `gui-aax`'s build.rs) and `AAX_VALIDATOR`
//! (the AAX Plug-In Validator binary, e.g.
//! `.../AAXValidator.framework/AAXValidator`) are both read from the
//! environment so this works with any local SDK/validator install location.
//!
//! `gui-aax` only wraps the editor view (no `AAX_CMain`, effect/parameter
//! description, or `.aaxplugin` bundle), so there is nothing yet for the
//! real AAX Plug-In Validator to check. This command builds against the
//! real SDK when available and reports validator status honestly instead
//! of fabricating a pass.
use std::env;
use std::path::Path;
use std::process::{Command, ExitCode};

pub fn bundle_aax() -> ExitCode {
    let aax_sdk = env::var("AAX_SDK").ok();
    let aax_validator = env::var("AAX_VALIDATOR").ok();

    match &aax_sdk {
        Some(path) => println!("AAX_SDK={path}"),
        None => println!(
            "note: AAX_SDK not set; gui-aax will build as a no-op stub (see build.rs warning)."
        ),
    }

    println!("Running: cargo build -p gui-aax --example gain --features aax");
    let status = Command::new("cargo")
        .args([
            "build",
            "-p",
            "gui-aax",
            "--example",
            "gain",
            "--features",
            "aax",
        ])
        .status()
        .expect("failed to run cargo build");

    if !status.success() {
        println!("FAIL: cargo build -p gui-aax --example gain --features aax");
        return ExitCode::from(1);
    }
    println!("PASS: gui-aax builds against AAX_SDK={:?}", aax_sdk);

    match aax_validator {
        Some(validator_path) => {
            if Path::new(&validator_path).exists() {
                println!("AAX_VALIDATOR={validator_path} (found)");
            } else {
                println!("AAX_VALIDATOR={validator_path} (not found at this path)");
            }
            println!(
                "SKIP: AAX Plug-In Validator run skipped. gui-aax does not yet produce a \
                 .aaxplugin bundle with a real AAX_CMain / effect description, so there is \
                 nothing for the validator to check yet."
            );
        }
        None => {
            println!(
                "SKIP: AAX_VALIDATOR not set. Set it to the AAXValidator binary path (e.g. \
                 .../AAXValidator.framework/AAXValidator) to have this reported explicitly."
            );
        }
    }

    ExitCode::from(0)
}
