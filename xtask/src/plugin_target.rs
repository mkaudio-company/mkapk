//! Resolves which `plugins/<slug>` crate the `bundle-*` commands operate on,
//! and derives that crate's package/lib/bin/symbol names from the one fixed
//! naming convention `new_plugin` writes when it scaffolds a plugin -- so
//! adding a new plugin (via `cargo xtask new-plugin`) never requires
//! touching any other part of xtask's own source.
use std::env;

pub struct PluginTarget {
    /// The `plugins/<slug>` directory name, e.g. `"gain"`.
    pub slug: String,
    /// Cargo package name, e.g. `"gain-plugin"`.
    pub package_name: String,
    /// `[lib] name`, e.g. `"gain_plugin"` -- what the built cdylib/staticlib
    /// is actually called on disk (`lib{lib_name}.dylib`/`.a`).
    pub lib_name: String,
    /// `[[bin]] name` for the standalone build, e.g. `"gain-standalone"`.
    pub standalone_bin: String,
    /// `[[bin]] name` for the AAX page-table generator, e.g.
    /// `"gain-aax-page-table"`.
    pub aax_page_table_bin: String,
    /// The symbol name passed to `au_entry! { factory_function: .. }`, e.g.
    /// `"gain_au_factory"`.
    pub au_factory_symbol: String,
}

impl PluginTarget {
    pub fn from_slug(slug: &str) -> Self {
        let underscored = slug.replace('-', "_");
        Self {
            package_name: format!("{slug}-plugin"),
            lib_name: format!("{underscored}_plugin"),
            standalone_bin: format!("{slug}-standalone"),
            aax_page_table_bin: format!("{slug}-aax-page-table"),
            au_factory_symbol: format!("{underscored}_au_factory"),
            slug: slug.to_string(),
        }
    }

    /// Reads `PLUGIN_CRATE` (default `"gain"`, this workspace's reference
    /// plugin) to decide which `plugins/<slug>` crate `bundle-*` operates on.
    pub fn resolve() -> Self {
        let slug = env::var("PLUGIN_CRATE").unwrap_or_else(|_| "gain".to_string());
        Self::from_slug(&slug)
    }
}

/// Derives a 4-character FourCC-style code from free text (a company or
/// plugin name): first character uppercased, the rest lowercased, padded
/// with `'x'` or truncated to exactly 4 ASCII characters. Guarantees at
/// least one non-lowercase character -- `auval` rejects manufacturer codes
/// that are all-lowercase (confirmed against a real host earlier), and this
/// keeps every generated plugin safe from that by construction rather than
/// by convention someone has to remember.
pub fn fourcc_code_from(text: &str) -> String {
    let alnum: String = text.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let base = if alnum.is_empty() {
        "plug".to_string()
    } else {
        alnum
    };
    let mut chars: Vec<char> = base.chars().take(4).collect();
    while chars.len() < 4 {
        chars.push('x');
    }
    chars[0] = chars[0].to_ascii_uppercase();
    for c in chars.iter_mut().skip(1) {
        *c = c.to_ascii_lowercase();
    }
    chars.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_slug_derives_every_name_from_the_slug_alone() {
        let target = PluginTarget::from_slug("multi-band");
        assert_eq!(target.package_name, "multi-band-plugin");
        assert_eq!(target.lib_name, "multi_band_plugin");
        assert_eq!(target.standalone_bin, "multi-band-standalone");
        assert_eq!(target.aax_page_table_bin, "multi-band-aax-page-table");
        assert_eq!(target.au_factory_symbol, "multi_band_au_factory");
    }

    #[test]
    fn fourcc_code_from_pads_short_text_and_fixes_casing() {
        assert_eq!(fourcc_code_from("mkaudio"), "Mkau");
        assert_eq!(fourcc_code_from("gain"), "Gain");
        assert_eq!(fourcc_code_from("hi"), "Hixx");
        assert_eq!(fourcc_code_from(""), "Plug");
    }
}
