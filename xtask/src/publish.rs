//! `cargo xtask publish`: publishes every publishable crate in this
//! workspace to crates.io in dependency order, retrying whatever depends on
//! a just-published crate until the index catches up. Every internal
//! `mkapk-*` dependency is a `path` + `version` pair, and crates.io needs
//! that `version` to already resolve to a real published version before
//! it'll accept anything depending on it -- publishing out of order fails
//! with "no matching package named `mkapk-*` found" (see the workspace
//! README's Publishing section, which this mirrors).
//!
//! Deliberately has no separate "is this version already published" check
//! of its own (an earlier version of this file queried crates.io's JSON API
//! directly via `curl` -- that API enforces a data-access/User-Agent policy
//! plain `curl` doesn't satisfy and returns a 403 instead of real data,
//! which made the check silently useless). Instead, `cargo publish` itself
//! is the only source of truth: its own stderr already distinguishes
//! "already exists" (skip) from "no matching package" (retry, the
//! dependency just isn't indexed yet) from a real failure.
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
    "mkapk-win32",
    "mkapk-mac",
    "mkapk-accessibility",
    "mkapk-widgets",
    "mkapk-vst3",
    "mkapk-au",
    "mkapk-aax",
    "mkapk-test-host",
    "mkapk-standalone",
];

const INDEX_RETRY_TIMEOUT: Duration = Duration::from_secs(180);
const INDEX_RETRY_INTERVAL: Duration = Duration::from_secs(5);

enum PublishOutcome {
    Published,
    AlreadyPublished,
    /// The registry hasn't indexed a just-published dependency yet --
    /// transient, worth retrying.
    DependencyNotYetIndexed,
    Failed(String),
}

pub fn publish(args: &[String]) -> ExitCode {
    let dry_run = args.iter().any(|a| a == "--dry-run");

    if dry_run {
        return publish_dry_run();
    }

    for &crate_name in PUBLISH_ORDER {
        println!("Running: cargo publish -p {crate_name}");
        let start = Instant::now();
        loop {
            match run_publish(crate_name, false) {
                PublishOutcome::Published => {
                    println!("PASS {crate_name}: published");
                    break;
                }
                PublishOutcome::AlreadyPublished => {
                    println!("SKIP {crate_name}: already on crates.io");
                    break;
                }
                PublishOutcome::DependencyNotYetIndexed => {
                    if start.elapsed() >= INDEX_RETRY_TIMEOUT {
                        eprintln!(
                            "FAIL: {crate_name}'s dependencies still aren't indexed on \
                             crates.io after 3 minutes of retrying. Check \
                             https://crates.io/crates/ for whatever it depends on and re-run \
                             `cargo xtask publish` to resume (already-published crates are \
                             skipped)."
                        );
                        return ExitCode::from(1);
                    }
                    println!(
                        "  a dependency isn't indexed yet, retrying {crate_name} in {}s...",
                        INDEX_RETRY_INTERVAL.as_secs()
                    );
                    thread::sleep(INDEX_RETRY_INTERVAL);
                }
                PublishOutcome::Failed(message) => {
                    eprintln!("FAIL: cargo publish -p {crate_name} did not succeed\n{message}");
                    return ExitCode::from(1);
                }
            }
        }
    }

    println!("All crates published.");
    ExitCode::from(0)
}

/// `--dry-run` can't fully validate a fresh, never-published workspace:
/// crates.io needs each dependency's exact version to already be live, so
/// anything past the first tier will legitimately fail dry-run until the
/// tiers before it are *really* published. This runs every crate anyway
/// (without stopping early or retrying) and prints a PASS/FAIL summary,
/// since that's still useful for catching real manifest problems (like the
/// missing `thirdparty/` exclude that caused a 413 earlier) independent of
/// ordering.
fn publish_dry_run() -> ExitCode {
    let mut any_failed = false;
    println!(
        "=== dry-run: expect failures past the first tier until earlier crates are really published ==="
    );
    for &crate_name in PUBLISH_ORDER {
        println!("Running: cargo publish -p {crate_name} --dry-run");
        match run_publish(crate_name, true) {
            PublishOutcome::Published => println!("PASS  {crate_name}"),
            PublishOutcome::AlreadyPublished => println!("SKIP  {crate_name} (already published)"),
            PublishOutcome::DependencyNotYetIndexed => {
                any_failed = true;
                println!("FAIL  {crate_name} (dependency not yet indexed)");
            }
            PublishOutcome::Failed(message) => {
                any_failed = true;
                println!("FAIL  {crate_name}\n{message}");
            }
        }
    }
    if any_failed {
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}

fn run_publish(crate_name: &str, dry_run: bool) -> PublishOutcome {
    let mut cmd = Command::new("cargo");
    cmd.args(["publish", "-p", crate_name]);
    if dry_run {
        cmd.args(["--dry-run", "--allow-dirty"]);
    }
    let output = cmd.output().expect("failed to run cargo publish");
    let stderr = String::from_utf8_lossy(&output.stderr);
    print!("{stderr}");

    if output.status.success() {
        return PublishOutcome::Published;
    }
    if stderr.contains("already exists") {
        return PublishOutcome::AlreadyPublished;
    }
    if stderr.contains("no matching package named") {
        return PublishOutcome::DependencyNotYetIndexed;
    }
    PublishOutcome::Failed(stderr.into_owned())
}
