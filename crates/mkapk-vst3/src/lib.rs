//! VST3 plugin format wrapper.
//!
//! Always available (behind the `vst3` feature): an `IPlugView`-facing
//! editor bridge (`view`) around `mkapk-host::PluginEditor`.
//!
//! Gated behind the vendored VST3 SDK submodules being populated (see
//! `build.rs`, which sets `cfg(vst3_sdk)`): a real, loadable VST3 plugin
//! entry point. Unlike the previous `vst3-sys`-based implementation, the
//! real `IPluginFactory`/`IComponent`/`IAudioProcessor`/`IEditController`/
//! `IPlugView` interfaces are implemented in C++ (`cpp/`, built by `xtask`
//! against the vendored, MIT-licensed Steinberg VST3 SDK), not in Rust --
//! this module only provides the `extern "C"` bridge functions that C++
//! shim calls into (see `cpp/VstBridge.h`), generated per-plugin by the
//! `vst3_entry!` macro. Without the SDK submodules populated, this crate is
//! a view-only stub (no bridge functions, so nothing a VST3 host could
//! actually load) -- run `git submodule update --init
//! crates/mkapk-vst3/thirdparty/*` to populate them.
#[cfg(feature = "vst3")]
pub mod view;

#[cfg(all(feature = "vst3", vst3_sdk))]
pub mod processor;

/// Identity function kept for source compatibility with existing
/// `mkapk_vst3::guid([...])` call sites -- CIDs are plain 16-byte arrays now
/// (no `vst3-sys`/`vst3-com` `GUID` type to convert into).
pub const fn guid(data: [u8; 16]) -> [u8; 16] {
    data
}

/// Generates every `extern "C"` function this crate's *generic* VST3 C++
/// shim (`cpp/VstFactory.cpp`, `cpp/VstProcessor.cpp`, `cpp/VstController.cpp`,
/// `cpp/VstView.cpp`) calls into (see `cpp/VstBridge.h`): plugin-identity
/// getters, both class IDs, bus/parameter metadata, MIDI-CC mapping, the
/// real-time processor bridge, and the editor bridge. None of the C++ on
/// the other side of this boundary is specific to `$make_processor`'s
/// concrete type -- it derives everything through these functions instead.
///
/// `parameters` should be `Vec<mkapk_host::ParameterInfo>` describing every
/// automatable parameter (typically `my_processor.parameters().to_vec()`);
/// `processor_cid`/`controller_cid` should be built via [`guid`] from two
/// distinct, randomly generated 16-byte values (never reuse another
/// plugin's CIDs). `midi_cc_map` pairs MIDI CC numbers with the parameter
/// each should automate; pass `&[]` for a plugin with no MIDI-CC
/// automation.
///
/// # Example
/// ```ignore
/// mkapk_vst3::vst3_entry! {
///     processor: Box::new(processor::GainProcessor::new()),
///     editor: |gateway| Box::new(ui::GainEditor::new(gateway)),
///     parameters: processor::GainProcessor::new().parameters().to_vec(),
///     processor_cid: mkapk_vst3::guid([0x84, 0xE8, /* ... */]),
///     controller_cid: mkapk_vst3::guid([0xD3, 0x9D, /* ... */]),
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
        fn __mkapk_vst3_parameters() -> &'static [::mkapk_host::ParameterInfo] {
            static PARAMETERS: ::std::sync::OnceLock<::std::vec::Vec<::mkapk_host::ParameterInfo>> =
                ::std::sync::OnceLock::new();
            PARAMETERS.get_or_init(|| $parameters)
        }

        // ---- Plugin identity ----------------------------------------------------
        #[unsafe(no_mangle)]
        pub extern "C" fn mkapk_vst3_plugin_name() -> *const ::std::os::raw::c_char {
            concat!($name, "\0").as_ptr() as *const ::std::os::raw::c_char
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn mkapk_vst3_vendor_name() -> *const ::std::os::raw::c_char {
            concat!($vendor, "\0").as_ptr() as *const ::std::os::raw::c_char
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn mkapk_vst3_url() -> *const ::std::os::raw::c_char {
            concat!($url, "\0").as_ptr() as *const ::std::os::raw::c_char
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn mkapk_vst3_email() -> *const ::std::os::raw::c_char {
            concat!($email, "\0").as_ptr() as *const ::std::os::raw::c_char
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn mkapk_vst3_version() -> *const ::std::os::raw::c_char {
            concat!($version, "\0").as_ptr() as *const ::std::os::raw::c_char
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn mkapk_vst3_processor_cid() -> *const u8 {
            static CID: [u8; 16] = $processor_cid;
            CID.as_ptr()
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn mkapk_vst3_controller_cid() -> *const u8 {
            static CID: [u8; 16] = $controller_cid;
            CID.as_ptr()
        }

        // ---- Bus layout -----------------------------------------------------------
        #[unsafe(no_mangle)]
        pub extern "C" fn mkapk_vst3_input_channels() -> u32 {
            $crate::processor::input_channels(&mut || $make_processor)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn mkapk_vst3_output_channels() -> u32 {
            $crate::processor::output_channels(&mut || $make_processor)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn mkapk_vst3_accepts_midi() -> i32 {
            $crate::processor::accepts_midi(&mut || $make_processor) as i32
        }

        // ---- Parameter metadata -----------------------------------------------------
        #[unsafe(no_mangle)]
        pub extern "C" fn mkapk_vst3_parameter_count() -> i32 {
            $crate::processor::parameter_count(__mkapk_vst3_parameters())
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn mkapk_vst3_parameter_id(index: i32) -> u32 {
            $crate::processor::parameter_id(__mkapk_vst3_parameters(), index)
        }
        /// # Safety
        /// `out` must be valid for `out_capacity` writable `u16`s -- upheld
        /// by this crate's C++ shim, the only caller.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn mkapk_vst3_parameter_name(
            index: i32,
            out: *mut u16,
            out_capacity: i32,
        ) -> i32 {
            // SAFETY: caller (this function's own doc comment) guarantees
            // `out` validity.
            unsafe {
                $crate::processor::parameter_name(
                    __mkapk_vst3_parameters(),
                    index,
                    out,
                    out_capacity,
                )
            }
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn mkapk_vst3_parameter_default(index: i32) -> f64 {
            $crate::processor::parameter_default(__mkapk_vst3_parameters(), index)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn mkapk_vst3_parameter_step_count(index: i32) -> i32 {
            $crate::processor::parameter_step_count(__mkapk_vst3_parameters(), index)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn mkapk_vst3_midi_cc_to_param(
            midi_cc_number: i32,
            out_param_id: *mut u32,
        ) -> i32 {
            match $crate::processor::midi_cc_to_param($midi_cc_map, midi_cc_number) {
                // SAFETY: `out_param_id` validity is this function's own
                // C++-shim-only-caller contract, matching every other
                // out-pointer in this bridge.
                Some(id) => {
                    unsafe {
                        *out_param_id = id;
                    }
                    1
                }
                None => 0,
            }
        }

        // ---- Processor lifecycle -----------------------------------------------------
        #[unsafe(no_mangle)]
        pub extern "C" fn mkapk_vst3_processor_create() -> *mut ::std::os::raw::c_void {
            $crate::processor::create(&mut || $make_processor)
        }
        /// # Safety
        /// `handle` must be a still-live pointer previously returned by
        /// `mkapk_vst3_processor_create`, not yet destroyed.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn mkapk_vst3_processor_destroy(handle: *mut ::std::os::raw::c_void) {
            unsafe { $crate::processor::destroy(handle) }
        }
        /// # Safety
        /// `handle` must be a still-live pointer previously returned by
        /// `mkapk_vst3_processor_create`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn mkapk_vst3_processor_prepare(
            handle: *mut ::std::os::raw::c_void,
            sample_rate: f64,
            max_samples_per_block: i32,
        ) {
            unsafe { $crate::processor::prepare(handle, sample_rate, max_samples_per_block) }
        }
        /// # Safety
        /// `handle` must be a still-live pointer previously returned by
        /// `mkapk_vst3_processor_create`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn mkapk_vst3_processor_set_parameter(
            handle: *mut ::std::os::raw::c_void,
            param_id: u32,
            normalized_value: f64,
        ) {
            unsafe { $crate::processor::set_parameter(handle, param_id, normalized_value) }
        }
        /// # Safety
        /// `handle` must be a still-live pointer previously returned by
        /// `mkapk_vst3_processor_create`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn mkapk_vst3_processor_get_parameter(
            handle: *mut ::std::os::raw::c_void,
            param_id: u32,
        ) -> f64 {
            unsafe { $crate::processor::get_parameter(handle, param_id) }
        }
        /// # Safety
        /// `handle` must be a still-live pointer previously returned by
        /// `mkapk_vst3_processor_create`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn mkapk_vst3_processor_handle_midi(
            handle: *mut ::std::os::raw::c_void,
            status: u8,
            data1: u8,
            data2: u8,
        ) {
            unsafe { $crate::processor::handle_midi(handle, status, data1, data2) }
        }
        /// # Safety
        /// `handle` must be a still-live pointer previously returned by
        /// `mkapk_vst3_processor_create`; `inputs`/`outputs` must each be
        /// valid for `num_inputs`/`num_outputs` `float*` entries, each of
        /// those valid for `num_frames` samples -- upheld by this crate's
        /// C++ shim, the only caller.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn mkapk_vst3_processor_process(
            handle: *mut ::std::os::raw::c_void,
            inputs: *const *const f32,
            num_inputs: i32,
            outputs: *const *mut f32,
            num_outputs: i32,
            num_frames: i32,
        ) {
            unsafe {
                $crate::processor::process(
                    handle,
                    inputs,
                    num_inputs,
                    outputs,
                    num_outputs,
                    num_frames,
                )
            }
        }

        // ---- Editor lifecycle -----------------------------------------------------
        #[unsafe(no_mangle)]
        pub extern "C" fn mkapk_vst3_editor_create() -> *mut ::std::os::raw::c_void {
            $crate::view::create(&mut $make_editor)
        }
        /// # Safety
        /// `handle` must be a still-live pointer previously returned by
        /// `mkapk_vst3_editor_create`, not yet destroyed.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn mkapk_vst3_editor_destroy(handle: *mut ::std::os::raw::c_void) {
            unsafe { $crate::view::destroy(handle) }
        }
        /// # Safety
        /// `handle` must be a still-live pointer previously returned by
        /// `mkapk_vst3_editor_create`; `parent` must be a valid native
        /// parent view/window handle for the current platform.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn mkapk_vst3_editor_open(
            handle: *mut ::std::os::raw::c_void,
            parent: *mut ::std::os::raw::c_void,
        ) {
            unsafe { $crate::view::open(handle, parent) }
        }
        /// # Safety
        /// `handle` must be a still-live pointer previously returned by
        /// `mkapk_vst3_editor_create`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn mkapk_vst3_editor_close(handle: *mut ::std::os::raw::c_void) {
            unsafe { $crate::view::close(handle) }
        }
        /// # Safety
        /// `handle` must be a still-live pointer previously returned by
        /// `mkapk_vst3_editor_create`; `out_width`/`out_height` must each be
        /// valid for one written `f32`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn mkapk_vst3_editor_get_size(
            handle: *mut ::std::os::raw::c_void,
            out_width: *mut f32,
            out_height: *mut f32,
        ) {
            unsafe { $crate::view::get_size(handle, out_width, out_height) }
        }
        /// # Safety
        /// `handle` must be a still-live pointer previously returned by
        /// `mkapk_vst3_editor_create`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn mkapk_vst3_editor_resize(
            handle: *mut ::std::os::raw::c_void,
            width: f32,
            height: f32,
        ) {
            unsafe { $crate::view::resize(handle, width, height) }
        }
        /// # Safety
        /// `handle` must be a still-live pointer previously returned by
        /// `mkapk_vst3_editor_create`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn mkapk_vst3_editor_idle(handle: *mut ::std::os::raw::c_void) {
            unsafe { $crate::view::idle(handle) }
        }
        /// # Safety
        /// `handle` must be a still-live pointer previously returned by
        /// `mkapk_vst3_editor_create`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn mkapk_vst3_editor_parameter_changed(
            handle: *mut ::std::os::raw::c_void,
            param_id: u32,
            normalized_value: f64,
        ) {
            unsafe { $crate::view::parameter_changed(handle, param_id, normalized_value) }
        }
        /// # Safety
        /// `handle` must be a still-live pointer previously returned by
        /// `mkapk_vst3_editor_create`. The function pointers, if non-null,
        /// must remain valid for the editor's lifetime -- upheld by this
        /// crate's C++ shim, the only caller.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn mkapk_vst3_editor_set_host_context(
            handle: *mut ::std::os::raw::c_void,
            context: *mut ::std::os::raw::c_void,
            begin_edit: ::std::option::Option<extern "C" fn(*mut ::std::os::raw::c_void, u32)>,
            perform_edit: ::std::option::Option<
                extern "C" fn(*mut ::std::os::raw::c_void, u32, f64),
            >,
            end_edit: ::std::option::Option<extern "C" fn(*mut ::std::os::raw::c_void, u32)>,
            resize_view: ::std::option::Option<
                extern "C" fn(*mut ::std::os::raw::c_void, f32, f32),
            >,
        ) {
            unsafe {
                $crate::view::set_host_context(
                    handle,
                    context,
                    begin_edit,
                    perform_edit,
                    end_edit,
                    resize_view,
                )
            }
        }
    };
}
