//! Prints this plugin's AAX page-table XML to stdout, for `xtask bundle-aax`
//! to capture into a file before invoking the AAX C++ shim's CMake build
//! (see `mkapk-aax`'s `page_table` module for why this resource is generated
//! rather than hand-maintained, and `xtask/src/aax.rs` for how it's used).
//!
//! The FourCC/name values below MUST match `plugins/gain/src/lib.rs`'s
//! `aax_entry!` invocation exactly -- both describe the same plugin
//! identity, but this binary doesn't require the AAX SDK to be present (it
//! only touches `mkapk_aax::page_table`, which has no SDK dependency), so it
//! can't simply reuse the `extern "C"` getters `aax_entry!` generates.
use mkapk_host::Processor as _;

fn main() {
    let identity = mkapk_aax::page_table::AaxPluginIdentity {
        manufacturer_id: mkapk_aax::fourcc(*b"Mkau"),
        product_id: mkapk_aax::fourcc(*b"Gain"),
        plugin_id_native: mkapk_aax::fourcc(*b"GnNa"),
        manufacturer_name: "mkaudio",
        plugin_name: "Gain",
    };
    let processor = gain_plugin::GainProcessor::new();
    let xml = mkapk_aax::page_table::generate_page_table_xml(&identity, processor.parameters());
    print!("{xml}");
}
