//! `cargo xtask new-plugin <slug>` -- scaffolds a new `plugins/<slug>` crate
//! from `plugins/gain` as a template: same processor/UI starting point
//! (`GainProcessor`/`GainEditor` renamed to `<Slug>Processor`/`<Slug>Editor`),
//! fresh VST3 GUIDs, fresh AAX/AU FourCC codes derived from the plugin name
//! and company, and a `Cargo.toml`/`lib.rs` trimmed to only the formats
//! requested. Registers the new crate in the root `Cargo.toml`'s workspace
//! `members` list.
//!
//! `build.rs`/`src/processor.rs`/`src/ui.rs`/`src/bin/*.rs` are copied
//! *from the live `plugins/gain` source on disk* and text-substituted, not
//! from a baked-in copy inside this binary -- so the template can never
//! drift from the actual reference implementation. `Cargo.toml`/`src/lib.rs`
//! are constructed fresh instead, since their content genuinely varies by
//! which formats were requested (conditional features/deps/bin entries).
use std::collections::hash_map::RandomState;
use std::fs;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::plugin_target::fourcc_code_from;
use crate::setup::slugify;

const ALL_FORMATS: [&str; 4] = ["standalone", "vst3", "au", "aax"];

pub fn new_plugin(args: &[String]) -> ExitCode {
    let mut positional = Vec::new();
    let mut display_name: Option<String> = None;
    let mut company: Option<String> = None;
    let mut formats: Vec<String> = ALL_FORMATS.iter().map(|s| s.to_string()).collect();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--display-name" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("--display-name requires a value");
                    return ExitCode::from(1);
                };
                display_name = Some(value.clone());
                i += 2;
            }
            "--company" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("--company requires a value");
                    return ExitCode::from(1);
                };
                company = Some(value.clone());
                i += 2;
            }
            "--formats" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("--formats requires a value");
                    return ExitCode::from(1);
                };
                formats = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                i += 2;
            }
            other => {
                positional.push(other.to_string());
                i += 1;
            }
        }
    }

    let Some(slug) = positional.first() else {
        eprintln!(
            "Usage: cargo xtask new-plugin <crate-slug> [--display-name NAME] [--company NAME] \
             [--formats standalone,vst3,au,aax]"
        );
        return ExitCode::from(1);
    };

    if !is_valid_slug(slug) {
        eprintln!(
            "Invalid plugin slug '{slug}': use lowercase ASCII letters, digits, and hyphens only \
             (e.g. \"delay\" or \"multi-band\")."
        );
        return ExitCode::from(1);
    }

    for format in &formats {
        if !ALL_FORMATS.contains(&format.as_str()) {
            eprintln!(
                "Unknown format '{format}'; expected one of: {}",
                ALL_FORMATS.join(", ")
            );
            return ExitCode::from(1);
        }
    }

    let display_name = display_name.unwrap_or_else(|| titlecase(slug));
    let company = company.unwrap_or_else(|| "mkaudio".to_string());

    let workspace_root = workspace_root();
    let plugin_dir = workspace_root.join("plugins").join(slug);
    if plugin_dir.exists() {
        eprintln!("plugins/{slug} already exists; pick a different slug.");
        return ExitCode::from(1);
    }

    let identity = Identity::new(slug, &display_name, &company);

    if let Err(error) = generate(&workspace_root, &plugin_dir, &identity, &formats) {
        eprintln!("FAIL: {error}");
        return ExitCode::from(1);
    }

    if let Err(error) = register_workspace_member(&workspace_root, slug) {
        eprintln!("FAIL: {error}");
        return ExitCode::from(1);
    }

    println!("Created plugins/{slug} ({display_name}, by {company})");
    println!("Formats: {}", formats.join(", "));
    println!();
    println!("Next steps:");
    println!("  cd plugins/{slug}");
    println!("  # edit src/processor.rs (DSP) and src/ui.rs (editor UI)");
    println!("  cargo build -p {}-plugin", slug);
    if formats.iter().any(|f| f == "standalone") {
        println!(
            "  cargo run -p {}-plugin --bin {}-standalone --features standalone",
            slug, slug
        );
    }
    println!("  PLUGIN_CRATE={} cargo xtask bundle-all", slug);
    ExitCode::from(0)
}

fn is_valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !slug.starts_with('-')
        && !slug.ends_with('-')
}

/// Shared with `bundle.rs` as the default `PLUGIN_NAME` for whichever
/// `plugins/<slug>` crate `PLUGIN_CRATE` selects, so a scaffolded plugin's
/// display name defaults sensibly even before anyone bundles it.
pub(crate) fn titlecase(slug: &str) -> String {
    slug.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn pascal_case(slug: &str) -> String {
    slug.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn upper_snake(slug: &str) -> String {
    slug.to_ascii_uppercase().replace('-', "_")
}

struct Identity {
    slug: String,
    display_name: String,
    company: String,
    lib_name: String,
    au_factory_symbol: String,
    manufacturer_fourcc: String,
    product_fourcc: String,
    native_fourcc: String,
    processor_cid: [u8; 16],
    controller_cid: [u8; 16],
    pascal: String,
    upper: String,
    company_slug: String,
    slug_slug: String,
}

impl Identity {
    fn new(slug: &str, display_name: &str, company: &str) -> Self {
        let target = crate::plugin_target::PluginTarget::from_slug(slug);
        Self {
            slug: slug.to_string(),
            display_name: display_name.to_string(),
            company: company.to_string(),
            lib_name: target.lib_name,
            au_factory_symbol: target.au_factory_symbol,
            manufacturer_fourcc: fourcc_code_from(company),
            product_fourcc: fourcc_code_from(slug),
            native_fourcc: fourcc_code_from(&format!("n{slug}")),
            processor_cid: random_bytes_16(),
            controller_cid: random_bytes_16(),
            pascal: pascal_case(slug),
            upper: upper_snake(slug),
            company_slug: slugify(company),
            slug_slug: slugify(slug),
        }
    }

    /// Ordered find/replace pairs applied to every file copied verbatim
    /// from `plugins/gain` (`build.rs`/`processor.rs`/`ui.rs`/`bin/*.rs`).
    /// Order matters: more specific patterns (byte-string literals, whole
    /// URLs) must run before the generic substrings they'd otherwise be
    /// clobbered by (e.g. `*b"Gain"` before the bare `"Gain"` display
    /// string, and the full "mkaudio" URLs before the bare `mkaudio` token).
    fn substitution_pairs(&self) -> Vec<(String, String)> {
        vec![
            (
                "plugins/gain/".to_string(),
                format!("plugins/{}/", self.slug),
            ),
            (
                "gain-standalone".to_string(),
                format!("{}-standalone", self.slug),
            ),
            (
                "gain-aax-page-table".to_string(),
                format!("{}-aax-page-table", self.slug),
            ),
            ("gain-plugin".to_string(), format!("{}-plugin", self.slug)),
            ("gain_plugin".to_string(), self.lib_name.clone()),
            (
                "gain_au_factory".to_string(),
                self.au_factory_symbol.clone(),
            ),
            (
                "GainProcessor".to_string(),
                format!("{}Processor", self.pascal),
            ),
            ("GainEditor".to_string(), format!("{}Editor", self.pascal)),
            (
                "GAIN_MIDI_CC".to_string(),
                format!("{}_MIDI_CC", self.upper),
            ),
            ("GAIN_PARAM".to_string(), format!("{}_PARAM", self.upper)),
            (
                "com.mkaudio.aax.gain".to_string(),
                format!("com.{}.aax.{}", self.company_slug, self.slug_slug),
            ),
            (
                "*b\"Mkau\"".to_string(),
                format!("*b\"{}\"", self.manufacturer_fourcc),
            ),
            (
                "*b\"Gain\"".to_string(),
                format!("*b\"{}\"", self.product_fourcc),
            ),
            (
                "*b\"GnNa\"".to_string(),
                format!("*b\"{}\"", self.native_fourcc),
            ),
            (
                "https://mkaudio.company".to_string(),
                format!("https://{}.example.com", self.company_slug),
            ),
            (
                "support@mkaudio.company".to_string(),
                format!("support@{}.example.com", self.company_slug),
            ),
            ("mkaudio".to_string(), self.company.clone()),
            ("gain plugin".to_string(), format!("{} plugin", self.slug)),
            ("\"Gain\"".to_string(), format!("\"{}\"", self.display_name)),
        ]
    }

    fn apply_substitutions(&self, mut content: String) -> String {
        for (from, to) in self.substitution_pairs() {
            content = content.replace(&from, &to);
        }
        content
    }
}

/// Non-cryptographic 16-byte generator for VST3 CIDs: seeded from
/// `RandomState`'s process-level OS-entropy seed plus the current time, so
/// two plugins generated in the same process still get different bytes.
/// Only needs to avoid accidental collisions between plugins, not resist an
/// adversary, so this avoids adding a `rand` dependency just for this.
fn random_bytes_16() -> [u8; 16] {
    let mut bytes = [0_u8; 16];
    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for (i, chunk) in bytes.chunks_mut(8).enumerate() {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u128(now_nanos);
        hasher.write_usize(i);
        let value = hasher.finish().to_le_bytes();
        chunk.copy_from_slice(&value[..chunk.len()]);
    }
    bytes
}

fn generate(
    workspace_root: &Path,
    plugin_dir: &Path,
    identity: &Identity,
    formats: &[String],
) -> Result<(), String> {
    let template_dir = workspace_root.join("plugins/gain");
    let has = |format: &str| formats.iter().any(|f| f == format);

    fs::create_dir_all(plugin_dir.join("src/bin"))
        .map_err(|e| format!("failed to create {}: {e}", plugin_dir.display()))?;

    copy_and_substitute(
        &template_dir.join("build.rs"),
        &plugin_dir.join("build.rs"),
        identity,
    )?;
    copy_and_substitute(
        &template_dir.join("src/processor.rs"),
        &plugin_dir.join("src/processor.rs"),
        identity,
    )?;
    copy_and_substitute(
        &template_dir.join("src/ui.rs"),
        &plugin_dir.join("src/ui.rs"),
        identity,
    )?;

    if has("standalone") {
        copy_and_substitute(
            &template_dir.join("src/bin/standalone.rs"),
            &plugin_dir.join("src/bin/standalone.rs"),
            identity,
        )?;
    }
    if has("aax") {
        copy_and_substitute(
            &template_dir.join("src/bin/aax_page_table.rs"),
            &plugin_dir.join("src/bin/aax_page_table.rs"),
            identity,
        )?;
    }
    if has("au") {
        copy_and_substitute(
            &template_dir.join("src/bin/au_info.rs"),
            &plugin_dir.join("src/bin/au_info.rs"),
            identity,
        )?;
    }

    fs::write(
        plugin_dir.join("Cargo.toml"),
        render_cargo_toml(identity, formats),
    )
    .map_err(|e| format!("failed to write Cargo.toml: {e}"))?;
    fs::write(
        plugin_dir.join("src/lib.rs"),
        render_lib_rs(identity, formats),
    )
    .map_err(|e| format!("failed to write src/lib.rs: {e}"))?;

    Ok(())
}

fn copy_and_substitute(from: &Path, to: &Path, identity: &Identity) -> Result<(), String> {
    let content =
        fs::read_to_string(from).map_err(|e| format!("failed to read {}: {e}", from.display()))?;
    let content = identity.apply_substitutions(content);
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    fs::write(to, content).map_err(|e| format!("failed to write {}: {e}", to.display()))
}

fn render_cargo_toml(identity: &Identity, formats: &[String]) -> String {
    let has = |format: &str| formats.iter().any(|f| f == format);
    let package_name = format!("{}-plugin", identity.slug);

    let mut bins = String::new();
    if has("standalone") {
        bins.push_str(&format!(
            "[[bin]]\nname = \"{slug}-standalone\"\npath = \"src/bin/standalone.rs\"\nrequired-features = [\"standalone\"]\n\n",
            slug = identity.slug
        ));
    }
    if has("aax") {
        bins.push_str(&format!(
            "[[bin]]\nname = \"{slug}-aax-page-table\"\npath = \"src/bin/aax_page_table.rs\"\nrequired-features = [\"aax\"]\n\n",
            slug = identity.slug
        ));
    }
    if has("au") {
        bins.push_str(&format!(
            "[[bin]]\nname = \"{slug}-au-info\"\npath = \"src/bin/au_info.rs\"\nrequired-features = [\"au\"]\n\n",
            slug = identity.slug
        ));
    }

    let mut feature_lines = vec!["default = []".to_string()];
    let mut deps = vec![
        "gui-core = { workspace = true }".to_string(),
        "gui-host = { workspace = true }".to_string(),
        "gui-res = { workspace = true }".to_string(),
        "gui-widgets = { workspace = true }".to_string(),
    ];
    if has("standalone") {
        feature_lines.push("standalone = [\"dep:gui-standalone\"]".to_string());
        deps.push("gui-standalone = { workspace = true, optional = true }".to_string());
    }
    if has("vst3") {
        feature_lines.push("vst3 = [\"dep:gui-vst3\"]".to_string());
        deps.push("gui-vst3 = { workspace = true, optional = true }".to_string());
    }
    if has("au") {
        feature_lines.push("au = [\"dep:gui-au\"]".to_string());
        deps.push("gui-au = { workspace = true, optional = true }".to_string());
    }
    if has("aax") {
        feature_lines.push("aax = [\"dep:gui-aax\"]".to_string());
        deps.push(
            "gui-aax = { workspace = true, optional = true, features = [\"aax\"] }".to_string(),
        );
    }

    format!(
        "[package]\n\
         name = \"{package_name}\"\n\
         version.workspace = true\n\
         edition.workspace = true\n\
         rust-version.workspace = true\n\
         authors.workspace = true\n\
         license.workspace = true\n\
         \n\
         [lib]\n\
         name = \"{lib_name}\"\n\
         crate-type = [\"cdylib\", \"staticlib\", \"rlib\"]\n\
         \n\
         {bins}\
         [features]\n\
         {features}\n\
         \n\
         [dependencies]\n\
         {deps}\n\
         \n\
         [target.'cfg(target_os = \"macos\")'.dependencies]\n\
         gui-mac = {{ workspace = true }}\n\
         \n\
         [target.'cfg(target_os = \"windows\")'.dependencies]\n\
         gui-win32 = {{ workspace = true }}\n",
        lib_name = identity.lib_name,
        features = feature_lines.join("\n"),
        deps = deps.join("\n"),
    )
}

fn render_lib_rs(identity: &Identity, formats: &[String]) -> String {
    let has = |format: &str| formats.iter().any(|f| f == format);
    let pascal = &identity.pascal;

    let mut out = format!(
        "//! A single plugin project -- one processor file, one UI file -- built and\n\
         //! bundled into Standalone/VST3/AUv2/AAX for macOS and Windows. See\n\
         //! `src/processor.rs` for the DSP and `src/ui.rs` for the editor; each\n\
         //! format's entry point (added behind its own feature) wires the two\n\
         //! together via `gui_host::LockFreeParameterGateway` without either file\n\
         //! knowing about the other.\n\
         //!\n\
         //! Scaffolded by `cargo xtask new-plugin` from `plugins/gain`.\n\
         \n\
         pub mod processor;\n\
         pub mod ui;\n\
         \n\
         pub use processor::{pascal}Processor;\n\
         pub use ui::{pascal}Editor;\n"
    );

    if has("vst3") {
        out.push_str(&format!(
            "\n\
             #[cfg(all(feature = \"vst3\", vst3_sdk))]\n\
             gui_vst3::vst3_entry! {{\n\
             \x20\x20\x20\x20processor: Box::new(processor::{pascal}Processor::new()),\n\
             \x20\x20\x20\x20editor: |gateway| Box::new(ui::{pascal}Editor::new(gateway, gui_host::PeakMeter::new())),\n\
             \x20\x20\x20\x20parameters: {{\n\
             \x20\x20\x20\x20\x20\x20\x20\x20use gui_host::Processor as _;\n\
             \x20\x20\x20\x20\x20\x20\x20\x20processor::{pascal}Processor::new().parameters().to_vec()\n\
             \x20\x20\x20\x20}},\n\
             \x20\x20\x20\x20processor_cid: gui_vst3::guid([{processor_cid}]),\n\
             \x20\x20\x20\x20controller_cid: gui_vst3::guid([{controller_cid}]),\n\
             \x20\x20\x20\x20// No MIDI-CC automation by default -- e.g. &[(7,\n\
             \x20\x20\x20\x20// processor::SOME_PARAM)] maps CC 7 to a parameter.\n\
             \x20\x20\x20\x20midi_cc_map: &[],\n\
             \x20\x20\x20\x20name: \"{display_name}\",\n\
             \x20\x20\x20\x20vendor: \"{company}\",\n\
             \x20\x20\x20\x20url: \"https://{company_slug}.example.com\",\n\
             \x20\x20\x20\x20email: \"support@{company_slug}.example.com\",\n\
             \x20\x20\x20\x20version: \"0.1.0\",\n\
             }}\n",
            processor_cid = format_byte_array(&identity.processor_cid),
            controller_cid = format_byte_array(&identity.controller_cid),
            display_name = identity.display_name,
            company = identity.company,
            company_slug = identity.company_slug,
        ));
    }

    if has("au") {
        out.push_str(&format!(
            "\n\
             #[cfg(all(feature = \"au\", target_os = \"macos\"))]\n\
             gui_au::au_entry! {{\n\
             \x20\x20\x20\x20processor: Box::new(processor::{pascal}Processor::new()),\n\
             \x20\x20\x20\x20editor: |gateway, meter| Box::new(ui::{pascal}Editor::new(gateway, meter)),\n\
             \x20\x20\x20\x20parameters: {{\n\
             \x20\x20\x20\x20\x20\x20\x20\x20use gui_host::Processor as _;\n\
             \x20\x20\x20\x20\x20\x20\x20\x20processor::{pascal}Processor::new().parameters().to_vec()\n\
             \x20\x20\x20\x20}},\n\
             \x20\x20\x20\x20factory_function: {au_factory_symbol},\n\
             }}\n",
            au_factory_symbol = identity.au_factory_symbol,
        ));
    }

    if has("aax") {
        out.push_str(&format!(
            "\n\
             #[cfg(all(feature = \"aax\", aax_sdk))]\n\
             gui_aax::aax_entry! {{\n\
             \x20\x20\x20\x20processor: processor::{pascal}Processor::new,\n\
             \x20\x20\x20\x20manufacturer_id: gui_aax::fourcc(*b\"{manufacturer_fourcc}\"),\n\
             \x20\x20\x20\x20product_id: gui_aax::fourcc(*b\"{product_fourcc}\"),\n\
             \x20\x20\x20\x20plugin_id_native: gui_aax::fourcc(*b\"{native_fourcc}\"),\n\
             \x20\x20\x20\x20effect_id: \"com.{company_slug}.aax.{slug_slug}\",\n\
             \x20\x20\x20\x20name: \"{display_name}\",\n\
             \x20\x20\x20\x20manufacturer_name: \"{company}\",\n\
             }}\n",
            manufacturer_fourcc = identity.manufacturer_fourcc,
            product_fourcc = identity.product_fourcc,
            native_fourcc = identity.native_fourcc,
            company_slug = identity.company_slug,
            slug_slug = identity.slug_slug,
            display_name = identity.display_name,
            company = identity.company,
        ));
    }

    out
}

fn format_byte_array(bytes: &[u8; 16]) -> String {
    bytes
        .iter()
        .map(|b| format!("0x{b:02X}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Appends `"plugins/<slug>"` to the root `Cargo.toml`'s `members` array,
/// right after the `"plugins/gain"` entry, preserving the rest of the file
/// untouched.
fn register_workspace_member(workspace_root: &Path, slug: &str) -> Result<(), String> {
    let cargo_toml_path = workspace_root.join("Cargo.toml");
    let content = fs::read_to_string(&cargo_toml_path)
        .map_err(|e| format!("failed to read {}: {e}", cargo_toml_path.display()))?;

    let marker = "\"plugins/gain\",";
    let Some(marker_pos) = content.find(marker) else {
        return Err(format!(
            "could not find {marker:?} in {} to insert the new member after",
            cargo_toml_path.display()
        ));
    };
    let insert_at = marker_pos + marker.len();
    let new_entry = format!("\n    \"plugins/{slug}\",");
    let mut updated = content;
    updated.insert_str(insert_at, &new_entry);

    fs::write(&cargo_toml_path, updated)
        .map_err(|e| format!("failed to write {}: {e}", cargo_toml_path.display()))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask should live under the workspace root")
        .to_path_buf()
}
