//! `cargo xtask publish`: publishes every publishable crate in this
//! workspace to crates.io in dependency order, waiting for each to be
//! indexed before publishing whatever depends on it. Every internal
//! `mkapk-*` dependency is a `path` + `version` pair, and crates.io needs
//! that `version` to already resolve to a real published version before
//! it'll accept anything depending on it -- publishing out of order fails
//! with "no matching package named `mkapk-*` found" (see the workspace
//! README's Publishing section, which this mirrors).
use std::process::{Command, ExitCode};
use std::thread;
use std::time::{Duration, Instant};

/// Bottom-up: each crate only depends (via a real, non-dev `[dependencies]`
/// entry) on crates earlier in this list. Update this list if a crate is
/// added, removed, or gains/loses an internal dependency -- `xtask` has no
/// TOML/graph dependency to derive this automatically, so it's hand-kept in
/// sync with the workspace README's Publishing section instead.
/// `plugins/gain` and `xtask` itself are `publish = false` and excluded.
const PUBLISH_ORDER: &[&str] = &[
    "mkapk-core",
    "mkapk-host",
    "mkapk-res",
    "mkapk-accessibility",
    "mkapk-win32",
    "mkapk-mac",
    "mkapk-widgets",
    "mkapk-vst3",
    "mkapk-au",
    "mkapk-aax",
    "mkapk-test-host",
    "mkapk-standalone",
];

pub fn publish(args: &[String]) -> ExitCode {
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let version = workspace_version();

    if dry_run {
        return publish_dry_run(&version);
    }

    for &crate_name in PUBLISH_ORDER {
        if is_version_published(crate_name, &version) {
            println!("SKIP {crate_name} {version}: already on crates.io");
            continue;
        }

        println!("Running: cargo publish -p {crate_name}");
        let status = Command::new("cargo")
            .args(["publish", "-p", crate_name])
            .status()
            .expect("failed to run cargo publish");
        if !status.success() {
            eprintln!("FAIL: cargo publish -p {crate_name} did not succeed");
            return ExitCode::from(1);
        }

        if !wait_until_indexed(crate_name, &version, Duration::from_secs(180)) {
            eprintln!(
                "FAIL: {crate_name} {version} did not appear in the crates.io index within \
                 3 minutes of publishing -- it may just be slow; check \
                 https://crates.io/crates/{crate_name} and re-run `cargo xtask publish` to \
                 resume (already-published crates are skipped)."
            );
            return ExitCode::from(1);
        }
    }

    println!("All crates published at {version}.");
    ExitCode::from(0)
}

/// `--dry-run` can't fully validate a fresh, never-published workspace:
/// crates.io needs each dependency's exact version to already be live, so
/// anything past the first tier will legitimately fail dry-run until the
/// tiers before it are *really* published. This runs every crate anyway
/// (without stopping early) and prints a PASS/FAIL summary, since that's
/// still useful for catching real manifest problems (like the missing
/// `thirdparty/` exclude that caused a 413 earlier) independent of ordering.
fn publish_dry_run(version: &str) -> ExitCode {
    let mut any_failed = false;
    println!(
        "=== dry-run: expect failures past the first tier until earlier crates are really published ==="
    );
    for &crate_name in PUBLISH_ORDER {
        println!("Running: cargo publish -p {crate_name} --dry-run");
        let status = Command::new("cargo")
            .args(["publish", "-p", crate_name, "--dry-run", "--allow-dirty"])
            .status()
            .expect("failed to run cargo publish --dry-run");
        let label = if status.success() { "PASS" } else { "FAIL" };
        if !status.success() {
            any_failed = true;
        }
        println!("{label}  {crate_name} {version}");
    }
    if any_failed {
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}

fn workspace_version() -> String {
    let manifest = std::fs::read_to_string(workspace_root().join("Cargo.toml"))
        .expect("failed to read workspace Cargo.toml");
    let mut in_package_section = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed == "[workspace.package]" {
            in_package_section = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_package_section = trimmed == "[workspace.package]";
        }
        if in_package_section {
            if let Some(rest) = trimmed.strip_prefix("version") {
                if let Some(value) = rest.trim_start().strip_prefix('=') {
                    return value.trim().trim_matches('"').to_string();
                }
            }
        }
    }
    panic!("could not find `version` under [workspace.package] in the workspace Cargo.toml");
}

/// Checks crates.io's real API (not `cargo info`, which caches its own
/// index locally and can report stale results right after a fresh publish)
/// for whether `version` of `crate_name` is live yet.
fn is_version_published(crate_name: &str, version: &str) -> bool {
    let Ok(output) = Command::new("curl")
        .args([
            "-sf",
            &format!("https://crates.io/api/v1/crates/{crate_name}"),
        ])
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let body = String::from_utf8_lossy(&output.stdout);
    body.contains(&format!("\"num\":\"{version}\""))
}

fn wait_until_indexed(crate_name: &str, version: &str, timeout: Duration) -> bool {
    let start = Instant::now();
    loop {
        if is_version_published(crate_name, version) {
            return true;
        }
        if start.elapsed() >= timeout {
            return false;
        }
        println!("  waiting for {crate_name} {version} to appear in the crates.io index...");
        thread::sleep(Duration::from_secs(5));
    }
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask should live under the workspace root")
        .to_path_buf()
}
