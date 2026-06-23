//! Build, bundle, validate, and CI helper tasks for the workspace.
use std::env;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo xtask <command>");
        return ExitCode::from(1);
    }

    match args[1].as_str() {
        "test" => run_test(),
        "check" => run_check(),
        "bundle-vst3" => {
            println!("bundle-vst3 not yet implemented");
            ExitCode::from(0)
        }
        "bundle-au" => {
            println!("bundle-au not yet implemented");
            ExitCode::from(0)
        }
        "bundle-aax" => {
            println!("bundle-aax not yet implemented");
            ExitCode::from(0)
        }
        "validate" => run_validate(),
        other => {
            eprintln!("Unknown command: {}", other);
            ExitCode::from(1)
        }
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
