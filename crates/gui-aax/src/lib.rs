//! AAX plugin format wrapper.
//!
//! Without the Avid AAX SDK (`AAX_SDK_PATH` unset at build time) this crate
//! is build-only: just the editor-view wrapper below, no real entry point.
//! With the SDK present (`cfg(aax_sdk)`, set by `build.rs`) and the `aax`
//! feature enabled, [`component`] additionally provides the real-time
//! process bridge (`process_block`/[`aax_entry`]) that this crate's C++
//! shim (`cpp/`) calls into -- see `component`'s doc comment for the full
//! architecture.
#![cfg_attr(not(aax_sdk), deny(unsafe_code))]

use gui_core::Sizef;
use gui_host::{EditorHost, NormalizedValue, ParameterId, ParentWindowHandle, PluginEditor};

#[cfg(all(feature = "aax", aax_sdk))]
pub mod component;

pub mod page_table;

/// Packs a 4-character code (e.g. AAX manufacturer/product/plugin IDs) into
/// the `u32` AAX's C++ API expects, so plugin projects don't need to do the
/// byte-shifting themselves.
pub const fn fourcc(bytes: [u8; 4]) -> u32 {
    ((bytes[0] as u32) << 24)
        | ((bytes[1] as u32) << 16)
        | ((bytes[2] as u32) << 8)
        | (bytes[3] as u32)
}

/// Generates every `extern "C"` function this crate's *generic* AAX C++
/// shim (`cpp/AaxPlugin_Describe.cpp`, `cpp/AaxPlugin_Parameters.cpp`,
/// `cpp/AaxPlugin_AlgProc.cpp`) calls into: plugin-identity getters, the
/// parameter-metadata getters those files loop over at Describe/EffectInit
/// time, and the real-time `gui_aax_process_block` bridge. None of the C++
/// on the other side of this boundary is specific to `$make_processor`'s
/// concrete type -- see `component`'s doc comment for why this works for
/// any `gui_host::Processor` with at most [`component::MAX_PARAMS`]
/// parameters, not just this one plugin.
///
/// `make_processor` should build a fresh `gui_host::Processor` (e.g.
/// `processor::GainProcessor::new`). The FourCC fields are most easily
/// built with [`fourcc`], e.g. `gui_aax::fourcc(*b"Mkau")`.
///
/// # Example
/// ```ignore
/// gui_aax::aax_entry! {
///     processor: processor::GainProcessor::new,
///     manufacturer_id: gui_aax::fourcc(*b"Mkau"),
///     product_id: gui_aax::fourcc(*b"Gain"),
///     plugin_id_native: gui_aax::fourcc(*b"GnNa"),
///     effect_id: "com.mkaudio.aax.gain",
///     name: "Gain",
///     manufacturer_name: "mkaudio",
/// }
/// ```
#[cfg(all(feature = "aax", aax_sdk))]
#[macro_export]
macro_rules! aax_entry {
    (
        processor: $make_processor:expr,
        manufacturer_id: $manufacturer_id:expr,
        product_id: $product_id:expr,
        plugin_id_native: $plugin_id_native:expr,
        effect_id: $effect_id:expr,
        name: $name:expr,
        manufacturer_name: $manufacturer_name:expr $(,)?
    ) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn gui_aax_effect_id() -> *const u8 {
            concat!($effect_id, "\0").as_ptr()
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn gui_aax_plugin_name() -> *const u8 {
            concat!($name, "\0").as_ptr()
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn gui_aax_manufacturer_name() -> *const u8 {
            concat!($manufacturer_name, "\0").as_ptr()
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn gui_aax_manufacturer_id() -> u32 {
            $manufacturer_id
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn gui_aax_product_id() -> u32 {
            $product_id
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn gui_aax_plugin_id_native() -> u32 {
            $plugin_id_native
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn gui_aax_parameter_count() -> i32 {
            $crate::component::parameter_count(&mut || $make_processor())
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn gui_aax_parameter_id(index: i32) -> u32 {
            $crate::component::parameter_id(&mut || $make_processor(), index)
        }
        /// # Safety
        /// `out` must be valid for `out_capacity` writable bytes -- upheld
        /// by this crate's C++ shim, the only caller.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn gui_aax_parameter_name(
            index: i32,
            out: *mut u8,
            out_capacity: i32,
        ) -> i32 {
            // SAFETY: caller (this function's own doc comment) guarantees
            // `out` is valid for `out_capacity` bytes.
            unsafe {
                $crate::component::parameter_name(
                    &mut || $make_processor(),
                    index,
                    out,
                    out_capacity,
                )
            }
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn gui_aax_parameter_default(index: i32) -> f32 {
            $crate::component::parameter_default(&mut || $make_processor(), index)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn gui_aax_parameter_step_count(index: i32) -> i32 {
            $crate::component::parameter_step_count(&mut || $make_processor(), index)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn gui_aax_accepts_midi() -> i32 {
            $crate::component::accepts_midi(&mut || $make_processor()) as i32
        }

        /// # Safety
        /// `values` must be valid for `num_values` reads (or null);
        /// `midi_bytes` must be valid for `num_midi_messages * 3` reads (or
        /// null/zero); `input`/`output` must each be valid for `num_frames`
        /// samples -- upheld by this crate's C++ shim, the only caller, per
        /// AAX's own `SAaxGeneric_Alg_Context` contract.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn gui_aax_process_block(
            values: *const f32,
            num_values: i32,
            midi_bytes: *const u8,
            num_midi_messages: i32,
            input: *const f32,
            output: *mut f32,
            num_frames: i32,
        ) {
            // SAFETY: caller (this function's own doc comment) guarantees
            // `values`/`midi_bytes`/`input`/`output` validity.
            unsafe {
                $crate::component::process_block(
                    || $make_processor(),
                    values,
                    num_values,
                    midi_bytes,
                    num_midi_messages,
                    input,
                    output,
                    num_frames,
                );
            }
        }
    };
}

struct NoopHost;

impl EditorHost for NoopHost {
    fn request_resize(&self, _size: Sizef) {}
    fn start_parameter_gesture(&self, _id: ParameterId) {}
    fn end_parameter_gesture(&self, _id: ParameterId) {}
    fn set_parameter_normalized(&self, _id: ParameterId, _value: NormalizedValue) {}
}

pub struct AaxEditor {
    editor: Box<dyn PluginEditor>,
    host: Option<Box<dyn EditorHost>>,
}

impl AaxEditor {
    pub fn new(editor: Box<dyn PluginEditor>) -> Self {
        Self { editor, host: None }
    }

    pub fn set_host(&mut self, host: Box<dyn EditorHost>) {
        self.host = Some(host);
    }

    #[allow(clippy::result_unit_err)]
    pub fn create_view(&mut self, parent: ParentWindowHandle) -> Result<(), ()> {
        let host: &dyn EditorHost = match &self.host {
            Some(host) => &**host,
            None => &NoopHost,
        };
        self.editor.open(parent, host);
        Ok(())
    }

    pub fn view_size(&self) -> Option<Sizef> {
        Some(
            self.editor
                .size_constraints()
                .min_size
                .unwrap_or_else(|| Sizef::new(400.0, 300.0)),
        )
    }

    pub fn draw(&mut self) {}

    pub fn timer_wakeup(&mut self) {
        self.editor.idle();
    }

    pub fn set_parameter(&mut self, id: u32, normalized: f32) {
        self.editor
            .on_parameter_changed(ParameterId(id), NormalizedValue::new(f64::from(normalized)));
    }

    pub fn destroy_view(&mut self) {
        self.editor.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gui_host::{EditorHost, NormalizedValue, ParameterId, PluginEditor, SizeConstraints};

    struct MockEditor;

    impl PluginEditor for MockEditor {
        fn open(&mut self, _parent: ParentWindowHandle, _host: &dyn EditorHost) {}
        fn close(&mut self) {}
        fn resize(&mut self, _size: Sizef) {}
        fn idle(&mut self) {}
        fn on_parameter_changed(&mut self, _id: ParameterId, _value: NormalizedValue) {}
        fn size_constraints(&self) -> SizeConstraints {
            SizeConstraints::default()
        }
    }

    #[test]
    fn aax_editor_lifecycle() {
        let mut editor = AaxEditor::new(Box::new(MockEditor));
        editor
            .create_view(ParentWindowHandle::Mac(core::ptr::null_mut()))
            .unwrap();
        let _ = editor.view_size();
        editor.timer_wakeup();
        editor.set_parameter(1, 0.5);
        editor.destroy_view();
    }
}
