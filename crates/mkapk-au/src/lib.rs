//! Audio Unit plugin format wrapper.
//!
//! Always available (behind the `au` feature): an `AUCocoaUIBase`-ready
//! Cocoa view around `gui-host::PluginEditor` (`editor`/`AuEditor`).
//!
//! macOS-only (no separate SDK download is needed -- AudioToolbox ships
//! with the OS -- so unlike VST3/AAX there's no `_SDK_PATH` env var gate,
//! just `target_os = "macos"`): a real, loadable AUv2 plugin entry point
//! (`AudioComponentPlugInInterface`'s `Open`/`Close`/`Lookup` vtable,
//! `Initialize`/`Render`/etc.) that bridges any `gui_host::Processor` +
//! `gui_host::PluginEditor` pair into a host-discoverable Audio Unit, via
//! the [`au_entry!`] macro. On other platforms, or without the `au`
//! feature, this crate is a no-op.
#![allow(unexpected_cfgs)]

#[cfg(all(feature = "au", target_os = "macos"))]
#[macro_use]
extern crate objc;

#[cfg(feature = "au")]
pub mod editor;
#[cfg(feature = "au")]
pub use editor::AuEditor;

#[cfg(all(feature = "au", target_os = "macos"))]
pub mod component;

#[cfg(all(feature = "au", target_os = "macos"))]
pub use au_sys;

/// Generates a real AUv2 plugin entry point (an `AudioComponentFactoryFunction`
/// named `factory_function`) wiring a concrete `gui_host::Processor` +
/// `gui_host::PluginEditor` pair into this crate's reusable
/// `component::create_instance`.
///
/// `parameters` should be `Vec<gui_host::ParameterInfo>` describing every
/// automatable parameter (typically `my_processor.parameters().to_vec()`).
/// Whatever bundles this plugin (see `xtask`) must register
/// `factory_function`'s name as the Audio Component's `factoryFunction` key
/// in its `Info.plist`, alongside matching `type`/`subtype`/`manufacturer`
/// four-character codes -- those codes live in the bundle manifest, not in
/// this macro, since this factory function itself never inspects them.
///
/// # Example
/// ```ignore
/// gui_au::au_entry! {
///     processor: Box::new(processor::GainProcessor::new()),
///     editor: |gateway, meter| Box::new(ui::GainEditor::new(gateway, meter)),
///     parameters: processor::GainProcessor::new().parameters().to_vec(),
///     factory_function: gain_au_factory,
/// }
/// ```
#[cfg(all(feature = "au", target_os = "macos"))]
#[macro_export]
macro_rules! au_entry {
    (
        processor: $make_processor:expr,
        editor: $make_editor:expr,
        parameters: $parameters:expr,
        factory_function: $factory_fn_name:ident $(,)?
    ) => {
        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub extern "C" fn $factory_fn_name(
            _desc: *const $crate::au_sys::AudioComponentDescription,
        ) -> *mut $crate::au_sys::AudioComponentPlugInInterface {
            $crate::editor::ensure_factory_class_registered();
            $crate::component::create_instance($parameters, || $make_processor, $make_editor)
        }
    };
}
