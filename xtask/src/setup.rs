//! Interactive setup helpers for bundling: plugin identity and code-signing
//! identity, each overridable via env var for non-interactive (CI) use.
use std::io::{self, IsTerminal, Write};
use std::process::Command;

/// Reads `PLUGIN_NAME`, prompting interactively (default `default_name`,
/// e.g. the titlecased slug of whichever `plugins/<slug>` crate is being
/// bundled -- see `PluginTarget`) if unset and stdin is a terminal.
pub fn resolve_plugin_name(default_name: &str) -> String {
    if let Ok(name) = std::env::var("PLUGIN_NAME") {
        if !name.trim().is_empty() {
            return name.trim().to_string();
        }
    }
    prompt_with_default("Plugin name", default_name)
}

/// Reads `PLUGIN_COMPANY`, prompting interactively (default "mkaudio") if
/// unset and stdin is a terminal.
pub fn resolve_company_name() -> String {
    if let Ok(company) = std::env::var("PLUGIN_COMPANY") {
        if !company.trim().is_empty() {
            return company.trim().to_string();
        }
    }
    prompt_with_default("Company name", "mkaudio")
}

/// Reads `CODESIGN_IDENTITY`, else lists identities from the keychain via
/// `security find-identity -v -p codesigning` and prompts for a choice
/// (including an ad-hoc "-" option) if stdin is a terminal. Returns `None`
/// only if no identity could be resolved (e.g. non-interactive with no env
/// var set), in which case bundling should skip code signing.
pub fn resolve_codesign_identity() -> Option<String> {
    if let Ok(identity) = std::env::var("CODESIGN_IDENTITY") {
        if !identity.trim().is_empty() {
            return Some(identity.trim().to_string());
        }
    }

    if !io::stdin().is_terminal() {
        println!(
            "note: CODESIGN_IDENTITY not set and stdin is not a terminal; skipping code signing."
        );
        return None;
    }

    let identities = list_codesign_identities();
    println!("Available code-signing identities:");
    for (i, identity) in identities.iter().enumerate() {
        println!("  {}) {}", i + 1, identity);
    }
    println!("  a) Ad-hoc signing (no certificate)");
    println!("  s) Skip code signing");

    loop {
        print!("Select an identity [1-{}, a, s]: ", identities.len());
        let _ = io::stdout().flush();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            return None;
        }
        let choice = line.trim();
        match choice {
            "s" | "S" => return None,
            "a" | "A" => return Some("-".to_string()),
            other => {
                if let Ok(index) = other.parse::<usize>() {
                    if index >= 1 && index <= identities.len() {
                        return Some(identities[index - 1].clone());
                    }
                }
                println!("Invalid selection, try again.");
            }
        }
    }
}

fn list_codesign_identities() -> Vec<String> {
    let output = Command::new("security")
        .args(["find-identity", "-v", "-p", "codesigning"])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let mut identities = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        // Lines look like: `1) HASH "Identity Name"`
        if let Some(start) = line.find('"') {
            if let Some(end) = line.rfind('"') {
                if end > start {
                    identities.push(line[start + 1..end].to_string());
                }
            }
        }
    }
    identities
}

fn prompt_with_default(label: &str, default: &str) -> String {
    if !io::stdin().is_terminal() {
        return default.to_string();
    }

    print!("{label} [{default}]: ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err() {
        return default.to_string();
    }
    let trimmed = line.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Lowercases and strips everything but ASCII alphanumerics, for use in
/// bundle identifiers.
pub fn slugify(value: &str) -> String {
    let slug: String = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if slug.is_empty() {
        "plugin".to_string()
    } else {
        slug
    }
}
