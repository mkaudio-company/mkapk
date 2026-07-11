//! VST3 plugin format wrapper.
//!
//! Always available (behind the `vst3` feature): an `IPlugView`
//! implementation around `gui-host::PluginEditor` (`view`/`PluginView`).
//!
//! Gated behind `VST3_SDK_PATH` being set at build time (see `build.rs`,
//! which sets `cfg(vst3_sdk)`): a real, loadable VST3 plugin entry point
//! (`IPluginFactory`/`IComponent`/`IAudioProcessor`/`IEditController`) that
//! bridges any `gui_host::Processor` + `gui_host::PluginEditor` pair into a
//! host-discoverable plugin, via the `vst3_entry!` macro. Without
//! `VST3_SDK_PATH` set, this crate is a view-only stub (no factory, so
//! nothing a VST3 host could actually load).
#[cfg(feature = "vst3")]
pub mod view;
#[cfg(feature = "vst3")]
pub use view::PluginView;

#[cfg(all(feature = "vst3", vst3_sdk))]
pub mod controller;
#[cfg(all(feature = "vst3", vst3_sdk))]
pub mod factory;
#[cfg(all(feature = "vst3", vst3_sdk))]
pub mod processor;
#[cfg(all(feature = "vst3", vst3_sdk))]
mod util;

#[cfg(all(feature = "vst3", vst3_sdk))]
pub use factory::{PluginInfo, VstFactory, guid};
#[cfg(all(feature = "vst3", vst3_sdk))]
pub use vst3_sys;

/// Generates a real VST3 plugin entry point (`GetPluginFactory` and the
/// module-load exports every VST3 host module convention expects) wiring a
/// concrete `gui_host::Processor` + `gui_host::PluginEditor` pair into
/// this crate's reusable `VstAudioProcessor`/`VstEditController`/`VstFactory`.
///
/// `parameters` should be `Vec<gui_host::ParameterInfo>` describing every
/// automatable parameter (typically `my_processor.parameters().to_vec()`);
/// `processor_cid`/`controller_cid` should be built via [`guid`] from two
/// distinct, randomly generated 16-byte values (never reuse another
/// plugin's CIDs). `midi_cc_map` pairs MIDI CC numbers with the parameter
/// each should automate (VST3's `IMidiMapping` mechanism -- see
/// `controller::VstEditController`); pass `&[]` for a plugin with no
/// MIDI-CC automation.
///
/// # Example
/// ```ignore
/// gui_vst3::vst3_entry! {
///     processor: Box::new(processor::GainProcessor::new()),
///     editor: |gateway| Box::new(ui::GainEditor::new(gateway)),
///     parameters: processor::GainProcessor::new().parameters().to_vec(),
///     processor_cid: gui_vst3::guid([0x84, 0xE8, /* ... */]),
///     controller_cid: gui_vst3::guid([0xD3, 0x9D, /* ... */]),
///     midi_cc_map: &[(7, processor::GAIN_PARAM)],
///     name: "Gain",
///     vendor: "mkaudio",
///     url: "https://mkaudio.company",
///     email: "support@mkaudio.company",
///     version: "0.1.0",
/// }
/// ```
#[cfg(all(feature = "vst3", vst3_sdk))]
#[macro_export]
macro_rules! vst3_entry {
    (
        processor: $make_processor:expr,
        editor: $make_editor:expr,
        parameters: $parameters:expr,
        processor_cid: $processor_cid:expr,
        controller_cid: $controller_cid:expr,
        midi_cc_map: $midi_cc_map:expr,
        name: $name:expr,
        vendor: $vendor:expr,
        url: $url:expr,
        email: $email:expr,
        version: $version:expr $(,)?
    ) => {
        fn __gui_vst3_create_processor() -> *mut ::std::os::raw::c_void {
            $crate::processor::VstAudioProcessor::create_instance($controller_cid, $make_processor)
        }

        fn __gui_vst3_create_controller() -> *mut ::std::os::raw::c_void {
            $crate::controller::VstEditController::create_instance(
                $parameters,
                $make_editor,
                $midi_cc_map,
            )
        }

        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub extern "system" fn InitDll() -> bool {
            true
        }

        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub extern "system" fn ExitDll() -> bool {
            true
        }

        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub extern "system" fn bundleEntry(_: *mut ::std::os::raw::c_void) -> bool {
            true
        }

        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub extern "system" fn bundleExit() -> bool {
            true
        }

        #[cfg(target_os = "linux")]
        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub extern "system" fn ModuleEntry(_: *mut ::std::os::raw::c_void) -> bool {
            true
        }

        #[cfg(target_os = "linux")]
        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub extern "system" fn ModuleExit() -> bool {
            true
        }

        /// # Safety
        /// Called by the VST3 host loader per the module ABI; takes no
        /// arguments and only ever returns a freshly heap-allocated factory.
        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub unsafe extern "system" fn GetPluginFactory() -> *mut ::std::os::raw::c_void {
            $crate::factory::VstFactory::create_instance(
                $crate::factory::PluginInfo {
                    processor_cid: $processor_cid,
                    controller_cid: $controller_cid,
                    name: $name,
                    vendor: $vendor,
                    url: $url,
                    email: $email,
                    version: $version,
                },
                __gui_vst3_create_processor,
                __gui_vst3_create_controller,
            )
        }
    };
}
