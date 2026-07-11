use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let manifest_path = PathBuf::from(&manifest_dir);
    let resources_dir = manifest_path.join("resources");
    let generated_path = manifest_path.join("src").join("generated.rs");

    println!("cargo:rerun-if-changed=resources");
    println!("cargo:rerun-if-changed=build.rs");

    let mut entries = Vec::new();

    if resources_dir.is_dir() {
        let mut paths: Vec<_> = fs::read_dir(&resources_dir)
            .expect("failed to read resources directory")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect();
        paths.sort();

        for path in &paths {
            let file_name = path.file_name().unwrap().to_string_lossy();
            let static_name = format!("ASSET_{}", sanitize(&file_name));
            let id = format!("ResourceId::from_bytes_le(b\"{}\")", escape(&file_name));
            let include_path = format!(
                "include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/resources/{file_name}\"))"
            );
            entries.push((static_name, id, include_path));
        }
    }

    let mut output = String::new();
    output.push_str("use crate::{EmbeddedBundle, ResourceId};\n\n");

    if entries.is_empty() {
        output.push_str("pub static EMBEDDED: EmbeddedBundle = EmbeddedBundle::new(&[]);\n");
    } else {
        for (static_name, _, include_path) in &entries {
            output.push_str(&format!(
                "pub static {static_name}: &[u8] = {include_path};\n\n"
            ));
        }

        output.push_str("pub static EMBEDDED: EmbeddedBundle = EmbeddedBundle::new(&[\n");
        for (static_name, id, _) in &entries {
            output.push_str(&format!("    ({id}, {static_name}),\n"));
        }
        output.push_str("]);\n");
    }

    if let Some(parent) = generated_path.parent() {
        fs::create_dir_all(parent).expect("failed to create src directory");
    }

    let existing = fs::read_to_string(&generated_path).unwrap_or_default();
    if existing != output {
        fs::write(&generated_path, output).expect("failed to write generated.rs");
    }

    let _ = std::process::Command::new("rustfmt")
        .args(["--edition", "2024"])
        .arg(&generated_path)
        .status();
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn escape(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '\\' => "\\\\".to_string(),
            '"' => "\\\"".to_string(),
            _ => c.to_string(),
        })
        .collect()
}
