//! Build, bundle, validate, and CI helper tasks for the workspace.
use std::env;
use std::process::{Command, ExitCode};

mod aax;
mod bundle;
mod new_plugin;
mod plugin_target;
mod publish;
mod setup;
mod vst3;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo xtask <command>");
        return ExitCode::from(1);
    }

    match args[1].as_str() {
        "test" => run_test(),
        "check" => run_check(),
        "bundle-standalone" => bundle::bundle_standalone(),
        "bundle-vst3" => vst3::bundle_vst3(),
        "bundle-au" => bundle::bundle_au(),
        "bundle-aax" => aax::bundle_aax(),
        "bundle-all" => run_bundle_all(),
        "validate" => run_validate(),
        "new-plugin" => new_plugin::new_plugin(&args[2..]),
        "publish" => publish::publish(&args[2..]),
        other => {
            eprintln!("Unknown command: {}", other);
            ExitCode::from(1)
        }
    }
}

/// Runs every `bundle-*` command in turn and prints a PASS/SKIP/FAIL
/// summary table, so the state of all four plugin formats is visible at a
/// glance rather than requiring four separate invocations.
fn run_bundle_all() -> ExitCode {
    let results = [
        ("standalone", bundle::bundle_standalone_status()),
        ("vst3", vst3::bundle_vst3_status()),
        ("au", bundle::bundle_au_status()),
        ("aax", aax::bundle_aax_status()),
    ];

    println!("\n=== bundle-all summary ===");
    let mut any_failed = false;
    for (name, status) in &results {
        if matches!(status, bundle::BundleStatus::Fail(_)) {
            any_failed = true;
        }
        println!("{:<5}  {:<11}  {}", status.label(), name, status.detail());
    }

    if any_failed {
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}

fn run_test() -> ExitCode {
    let mut failed = false;

    let mut test_cmd = Command::new("cargo");
    test_cmd.args(["test", "--workspace"]);
    println!("Running: cargo test --workspace");
    let status = test_cmd.status().expect("failed to run cargo test");
    if !status.success() {
        failed = true;
    }

    let mut clippy_cmd = Command::new("cargo");
    clippy_cmd.args(["clippy", "--workspace", "--", "-D", "warnings"]);
    println!("Running: cargo clippy --workspace -- -D warnings");
    let status = clippy_cmd.status().expect("failed to run cargo clippy");
    if !status.success() {
        failed = true;
    }

    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}

fn run_validate() -> ExitCode {
    let mut failed = false;

    let mut test_cmd = Command::new("cargo");
    test_cmd.args(["test", "--workspace"]);
    println!("Running: cargo test --workspace");
    let status = test_cmd.status().expect("failed to run cargo test");
    if status.success() {
        println!("PASS: cargo test --workspace");
    } else {
        println!("FAIL: cargo test --workspace");
        failed = true;
    }

    let mut clippy_cmd = Command::new("cargo");
    clippy_cmd.args(["clippy", "--workspace", "--", "-D", "warnings"]);
    println!("Running: cargo clippy --workspace -- -D warnings");
    let status = clippy_cmd.status().expect("failed to run cargo clippy");
    if status.success() {
        println!("PASS: cargo clippy --workspace -- -D warnings");
    } else {
        println!("FAIL: cargo clippy --workspace -- -D warnings");
        failed = true;
    }

    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}

fn run_check() -> ExitCode {
    let mut failed = false;

    let mut fmt_cmd = Command::new("cargo");
    fmt_cmd.args(["fmt", "--check"]);
    println!("Running: cargo fmt --check");
    let status = fmt_cmd.status().expect("failed to run cargo fmt");
    if !status.success() {
        failed = true;
    }

    let mut clippy_cmd = Command::new("cargo");
    clippy_cmd.args(["clippy", "--workspace", "--", "-D", "warnings"]);
    println!("Running: cargo clippy --workspace -- -D warnings");
    let status = clippy_cmd.status().expect("failed to run cargo clippy");
    if !status.success() {
        failed = true;
    }

    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}
