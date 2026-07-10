//! AAX build helper for `cargo xtask bundle-aax`.
//!
//! `AAX_SDK_PATH` (already read by `gui-aax`'s build.rs) and `AAX_VALIDATOR`
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

use crate::bundle::BundleStatus;

pub fn bundle_aax() -> ExitCode {
    bundle_aax_status().into_exit_code()
}

/// AAX stays build-only (no real `AAX_CMain`/effect description/`.aaxplugin`
/// bundle -- see `gui-aax`'s own docs for why this is scoped out for now),
/// so unlike VST3/AU this doesn't assemble a bundle at all; it just
/// verifies `gui-aax` builds against a real SDK when one is configured, and
/// reports the AAX Plug-In Validator's status honestly instead of
/// fabricating a pass.
pub fn bundle_aax_status() -> BundleStatus {
    let aax_sdk = env::var("AAX_SDK_PATH").ok();
    let aax_validator = env::var("AAX_VALIDATOR").ok();

    match &aax_sdk {
        Some(path) => println!("AAX_SDK_PATH={path}"),
        None => println!(
            "note: AAX_SDK_PATH not set; gui-aax will build as a no-op stub (see build.rs warning)."
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
        return BundleStatus::Fail(
            "cargo build -p gui-aax --example gain --features aax did not succeed".to_string(),
        );
    }

    match aax_validator {
        Some(validator_path) => {
            if Path::new(&validator_path).exists() {
                println!("AAX_VALIDATOR={validator_path} (found)");
            } else {
                println!("AAX_VALIDATOR={validator_path} (not found at this path)");
            }
        }
        None => {
            println!(
                "note: AAX_VALIDATOR not set. Set it to the AAXValidator binary path (e.g. \
                 .../AAXValidator.framework/AAXValidator) to have this reported explicitly."
            );
        }
    }

    BundleStatus::Skip(format!(
        "gui-aax builds against AAX_SDK_PATH={aax_sdk:?}, but does not yet produce a real \
         .aaxplugin bundle (no AAX_CMain/effect description), so there is nothing for the AAX \
         Plug-In Validator to check yet -- build-only status quo."
    ))
}
