//! A single plugin project — one processor file, one UI file — built and
//! bundled into Standalone/VST3/AUv2/AAX for macOS and Windows. See
//! `src/processor.rs` for the DSP and `src/ui.rs` for the editor; each
//! format's entry point (added behind its own feature) wires the two
//! together via `gui_host::LockFreeParameterGateway` without either file
//! knowing about the other.
//!
//! No unsafe code is written in this crate's own `processor`/`ui` modules;
//! the `vst3_entry!` invocation below expands to VST3's required
//! `unsafe extern "system"` module-load exports (owned/verified by
//! `gui-vst3`, not authored here), which is why this crate does not set
//! `#![deny(unsafe_code)]` at the root the way `gui-standalone` does.

pub mod processor;
pub mod ui;

pub use processor::GainProcessor;
pub use ui::GainEditor;

// Two randomly generated, plugin-specific CIDs (never reuse these for a
// different plugin). `gui_vst3::vst3_entry!` only exists (and only expands
// to anything, including the `unsafe extern "system"` VST3 module-load
// exports it generates) when gui-vst3 is built with `VST3_SDK_PATH` set.
// `cargo:rustc-cfg` from another crate's build script doesn't propagate
// here, so this crate's own `build.rs` re-checks the same env var to gate
// this invocation consistently -- without it, `--features vst3` alone
// (no `VST3_SDK_PATH`) would fail to compile instead of building as a
// harmless stub.
// VST3 doesn't yet give the editor a real per-block audio peak (see
// `gui-vst3`'s `IAudioProcessor::process`, which isn't wired to a shared
// `PeakMeter` the way `gui-standalone`'s audio callback is), so the meter
// stays at a flat zero here rather than showing a fake/decorative value.
#[cfg(all(feature = "vst3", vst3_sdk))]
gui_vst3::vst3_entry! {
    processor: Box::new(processor::GainProcessor::new()),
    editor: |gateway| Box::new(ui::GainEditor::new(gateway, gui_host::PeakMeter::new())),
    parameters: {
        use gui_host::Processor as _;
        processor::GainProcessor::new().parameters().to_vec()
    },
    processor_cid: gui_vst3::guid([
        0x54, 0xA9, 0x72, 0x9E, 0xDE, 0xF6, 0x4F, 0xBF, 0xA4, 0x48, 0x65, 0x3F, 0x34, 0xF2, 0x69,
        0x32,
    ]),
    controller_cid: gui_vst3::guid([
        0x92, 0x1E, 0x0D, 0x83, 0xE1, 0x93, 0x48, 0x69, 0xA9, 0xF0, 0x87, 0xF6, 0x14, 0xED, 0xE0,
        0xF5,
    ]),
    // CC 7 (Channel Volume) automates the same gain parameter this crate's
    // UI and `GainProcessor::handle_midi` respond to -- see
    // `processor::GAIN_MIDI_CC`.
    midi_cc_map: &[(processor::GAIN_MIDI_CC, processor::GAIN_PARAM)],
    name: "Gain",
    vendor: "mkaudio",
    url: "https://mkaudio.company",
    email: "support@mkaudio.company",
    version: "0.1.0",
}

// Unlike VST3/AAX, AUv2 needs no separate SDK (AudioToolbox ships with
// macOS), so `gui_au::au_entry!` only needs `target_os = "macos"` to have
// gated the real entry point in, not an `_SDK_PATH` env var; see
// `gui-au`'s `component` module for the real audio-thread wiring this
// expands into (including real per-block peak metering, unlike VST3's).
#[cfg(all(feature = "au", target_os = "macos"))]
gui_au::au_entry! {
    processor: Box::new(processor::GainProcessor::new()),
    editor: |gateway, meter| Box::new(ui::GainEditor::new(gateway, meter)),
    parameters: {
        use gui_host::Processor as _;
        processor::GainProcessor::new().parameters().to_vec()
    },
    factory_function: gain_au_factory,
}

// AAX's own "Describe"/parameter-registration code (`gui-aax/cpp/`) is a
// generic C++ layer built by the Avid AAX SDK's own CMake tooling -- it
// reads this plugin's identity and parameter list entirely through the
// extern "C" functions this macro generates, so nothing Gain-specific lives
// in C++ at all. See `gui-aax::component`'s doc comment for the full
// picture.
#[cfg(all(feature = "aax", aax_sdk))]
gui_aax::aax_entry! {
    processor: processor::GainProcessor::new,
    manufacturer_id: gui_aax::fourcc(*b"Mkau"),
    product_id: gui_aax::fourcc(*b"Gain"),
    plugin_id_native: gui_aax::fourcc(*b"GnNa"),
    effect_id: "com.mkaudio.aax.gain",
    name: "Gain",
    manufacturer_name: "mkaudio",
}
