//! A real AUv2 plugin entry point: `AudioComponentFactoryFunction` +
//! `AudioComponentPlugInInterface` vtable (`Open`/`Close`/`Lookup`), backed
//! by `au-sys`'s hand-written bindings to the C API (the AU equivalent of
//! what `vst3-sys` gives `gui-vst3`, minus a higher-level dispatch macro --
//! there isn't one for AUv2, so the vtable dispatch here is hand-written).
//!
//! Scope: a minimal, real, `kAudioUnitType_Effect`-style Audio Unit --
//! fixed channel counts (from `Processor::channel_layout`), one input
//! element, one output element, `Float32` non-interleaved only, no MIDI.
//! Supports enough properties/selectors for a host to load, connect,
//! automate, and render real audio through it: `Initialize`/
//! `Uninitialize`/`Reset`, stream format, parameter list/info, supported
//! channel counts, max frames per slice, the render callback used to pull
//! input (the AUv2 "pull" model), `Render` itself, and `CocoaUI` (wired to
//! `gui-au`'s existing Cocoa view code via a small private property -- see
//! `editor.rs`'s `AUCocoaUIBase` factory class).
//!
//! Not implemented (spec-legal to omit; hosts fall back to defaults/ignore):
//! property-change listeners, factory presets, class-info (session state)
//! persistence, MIDI.
//!
//! **Verification ceiling**: this compiles, type-checks, and its logic has
//! been reviewed carefully against the AudioToolbox headers `au-sys` is
//! transcribed from, but there is no AU host or `auval`-equivalent
//! validator available in this environment, so it has not been run against
//! a real host. Treat it the same as `gui-vst3`'s real entry point was
//! before first real-host testing.
#![cfg(target_os = "macos")]

use core::ffi::c_void;
use std::mem;
use std::sync::Arc;

use gui_host::{
    LockFreeParameterGateway, NormalizedValue, ParameterGateway, ParameterId, ParameterInfo,
    PeakMeter, PluginEditor, Processor, apply_pending_parameters,
};

/// A custom, non-Apple-reserved property ID (Apple's own IDs top out in the
/// low hundreds; developer-private IDs conventionally start at 64000) used
/// only as a back-channel between `Render`/dispatch (this module) and the
/// `AUCocoaUIBase` factory class (`editor.rs`): given the host's opaque
/// `AudioUnit` handle, `AudioUnitGetProperty` with this ID returns the raw
/// pointer to this instance's `AuInstance`, letting the Cocoa factory reach
/// back into the same `Processor`/`PluginEditor` pair `Render` drives.
pub(crate) const K_GUI_HOST_STATE_PROPERTY: au_sys::AudioUnitPropertyID = 64_000;

/// The vtable-plus-state block a factory function allocates and returns a
/// pointer to. `#[repr(C)]` with `interface` first is load-bearing: the
/// Component Manager passes the *same pointer the factory returned* back
/// into `Open`/`Close` as `self_ptr`, and because `interface` is the first
/// field, that pointer is simultaneously valid as `*mut AuInstance` and
/// `*mut au_sys::AudioComponentPlugInInterface`.
#[repr(C)]
struct AuInstance {
    interface: au_sys::AudioComponentPlugInInterface,
    state: AuState,
}

type EditorFactory =
    Box<dyn FnOnce(Arc<LockFreeParameterGateway>, PeakMeter) -> Box<dyn PluginEditor>>;

struct AuState {
    processor: Box<dyn Processor>,
    gateway: Arc<LockFreeParameterGateway>,
    meter: PeakMeter,
    make_editor: Option<EditorFactory>,
    parameters: Vec<ParameterInfo>,
    sample_rate: f64,
    max_frames: u32,
    input_channels: u32,
    output_channels: u32,
    initialized: bool,
    input_render_callback: Option<au_sys::AURenderCallbackStruct>,
    input_buffers: InputBufferList,
    #[allow(dead_code)]
    au_instance: au_sys::AudioUnit,
}

impl AuState {
    fn resize_scratch(&mut self) {
        self.input_buffers = InputBufferList::new(
            self.input_channels.max(1) as usize,
            self.max_frames as usize,
        );
    }
}

/// A reusable, non-allocating (after construction) `AudioBufferList` used
/// to pull input from a registered `AURenderCallback`. `AudioBufferList`'s
/// C layout is a fixed one-buffer header followed by as many additional
/// `AudioBuffer` slots as `mNumberBuffers` needs, so this owns a raw byte
/// buffer sized for the real channel count and casts into it, rather than
/// using `au_sys::AudioBufferList` directly (which only has room for one).
struct InputBufferList {
    storage: Vec<u8>,
    channel_data: Vec<Vec<f32>>,
}

impl InputBufferList {
    fn new(channels: usize, max_frames: usize) -> Self {
        let channels = channels.max(1);
        let size = mem::size_of::<au_sys::AudioBufferList>()
            + channels.saturating_sub(1) * mem::size_of::<au_sys::AudioBuffer>();
        Self {
            storage: vec![0u8; size],
            channel_data: vec![vec![0.0_f32; max_frames.max(1)]; channels],
        }
    }

    fn as_mut_ptr(&mut self) -> *mut au_sys::AudioBufferList {
        self.storage.as_mut_ptr() as *mut au_sys::AudioBufferList
    }

    /// Points the buffer list's entries at this call's scratch channels and
    /// sizes them to `frames`, ready to be filled by a render callback.
    fn prepare(&mut self, frames: usize) {
        let channel_count = self.channel_data.len();
        // SAFETY: `storage` is sized for exactly `channel_count` buffers
        // (see `new`), so writing `mNumberBuffers` then indexing that many
        // entries via `buffers_mut` stays in bounds.
        unsafe {
            let list = &mut *self.as_mut_ptr();
            list.mNumberBuffers = channel_count as u32;
            let buffers = list.buffers_mut();
            for (buffer, data) in buffers.iter_mut().zip(self.channel_data.iter_mut()) {
                buffer.mNumberChannels = 1;
                buffer.mDataByteSize = (frames * mem::size_of::<f32>()) as u32;
                buffer.mData = data.as_mut_ptr() as *mut c_void;
            }
        }
    }

    fn silence(&mut self, frames: usize) {
        for channel in &mut self.channel_data {
            let frames = frames.min(channel.len());
            channel[..frames].fill(0.0);
        }
    }
}

/// Writes as much of `*value` as fits in `*io_data_size` bytes (the
/// destination buffer's capacity, per `AudioUnitGetProperty`'s in/out
/// convention), then reports the real size back through `*io_data_size`.
///
/// # Safety
/// `out_data` must point to at least `*io_data_size` writable bytes, and
/// `io_data_size` must point to a valid `u32`.
unsafe fn write_property<T>(out_data: *mut c_void, io_data_size: *mut u32, value: &T) {
    unsafe {
        let needed = mem::size_of::<T>();
        let capacity = *io_data_size as usize;
        let to_copy = needed.min(capacity);
        if to_copy > 0 {
            std::ptr::copy_nonoverlapping(
                value as *const T as *const u8,
                out_data as *mut u8,
                to_copy,
            );
        }
        *io_data_size = needed as u32;
    }
}

/// Slice-valued counterpart to [`write_property`] (e.g. the parameter ID
/// list), same in/out size convention.
///
/// # Safety
/// Same as [`write_property`].
unsafe fn write_property_slice<T: Copy>(
    out_data: *mut c_void,
    io_data_size: *mut u32,
    values: &[T],
) {
    unsafe {
        let needed = std::mem::size_of_val(values);
        let capacity = *io_data_size as usize;
        let to_copy = needed.min(capacity);
        if to_copy > 0 {
            std::ptr::copy_nonoverlapping(
                values.as_ptr() as *const u8,
                out_data as *mut u8,
                to_copy,
            );
        }
        *io_data_size = needed as u32;
    }
}

fn build_parameter_info(param: &ParameterInfo) -> au_sys::AudioUnitParameterInfo {
    let mut name = [0 as core::ffi::c_char; 52];
    for (slot, byte) in name.iter_mut().zip(param.name.as_bytes()) {
        *slot = *byte as core::ffi::c_char;
    }
    au_sys::AudioUnitParameterInfo {
        name,
        unitName: std::ptr::null_mut(),
        clumpID: 0,
        cfNameString: std::ptr::null_mut(),
        unit: au_sys::kAudioUnitParameterUnit_Generic,
        minValue: param.min_value.get() as f32,
        maxValue: param.max_value.get() as f32,
        defaultValue: param.default_value.get() as f32,
        flags: au_sys::kAudioUnitParameterFlag_IsReadable
            | au_sys::kAudioUnitParameterFlag_IsWritable,
    }
}

/// Builds the `AUCocoaViewInfo` response for `kAudioUnitProperty_CocoaUI`.
///
/// Per Core Audio's "copy" convention for CF-typed property getters, the
/// returned `CFStringRef` is retained and ownership transfers to the
/// caller (the host), which must `CFRelease` it. `mCocoaAUViewBundleLocation`
/// is left null, which hosts treat as "the same bundle the factory function
/// loaded from".
fn build_cocoa_view_info() -> au_sys::AUCocoaViewInfo {
    // SAFETY: `cf_string_create` just wraps `CFStringCreateWithBytes`, a
    // standard CoreFoundation call; the class name is a fixed ASCII
    // literal.
    let class_name = unsafe { au_sys::cf_string_create(crate::editor::FACTORY_CLASS_NAME) };
    au_sys::AUCocoaViewInfo {
        mCocoaAUViewBundleLocation: std::ptr::null_mut(),
        mCocoaAUViewClass: [class_name],
    }
}

unsafe extern "C" fn au_open(
    self_ptr: *mut c_void,
    instance: au_sys::AudioUnit,
) -> au_sys::OSStatus {
    let inst = unsafe { &mut *(self_ptr as *mut AuInstance) };
    inst.state.au_instance = instance;
    au_sys::noErr
}

unsafe extern "C" fn au_close(self_ptr: *mut c_void) -> au_sys::OSStatus {
    // SAFETY: `self_ptr` is the exact pointer `create_instance` returned
    // via `Box::into_raw`; the Component Manager guarantees `Close` is
    // called at most once and no other method after it.
    drop(unsafe { Box::from_raw(self_ptr as *mut AuInstance) });
    au_sys::noErr
}

unsafe extern "C" fn au_initialize(self_ptr: *mut c_void) -> au_sys::OSStatus {
    let state = unsafe { &mut (*(self_ptr as *mut AuInstance)).state };
    state
        .processor
        .prepare(state.sample_rate, state.max_frames as usize);
    // Seed the gateway with every parameter's current value *before* the
    // host can start calling `Render`/`GetParameter` concurrently, so
    // `au_get_parameter` never needs to read `processor` directly off the
    // audio thread (see its doc comment).
    for param in &state.parameters {
        let value = state.processor.parameter_value(param.id);
        state.gateway.set_normalized(param.id, value);
    }
    state.resize_scratch();
    state.initialized = true;
    au_sys::noErr
}

unsafe extern "C" fn au_uninitialize(self_ptr: *mut c_void) -> au_sys::OSStatus {
    let state = unsafe { &mut (*(self_ptr as *mut AuInstance)).state };
    state.initialized = false;
    au_sys::noErr
}

unsafe extern "C" fn au_reset(
    self_ptr: *mut c_void,
    _in_scope: au_sys::AudioUnitScope,
    _in_element: au_sys::AudioUnitElement,
) -> au_sys::OSStatus {
    let state = unsafe { &mut (*(self_ptr as *mut AuInstance)).state };
    state.processor.reset();
    au_sys::noErr
}

unsafe extern "C" fn au_get_property_info(
    self_ptr: *mut c_void,
    in_id: au_sys::AudioUnitPropertyID,
    _in_scope: au_sys::AudioUnitScope,
    _in_element: au_sys::AudioUnitElement,
    out_data_size: *mut au_sys::UInt32,
    out_writable: *mut au_sys::Boolean,
) -> au_sys::OSStatus {
    let state = unsafe { &(*(self_ptr as *const AuInstance)).state };
    let (size, writable): (usize, bool) = match in_id {
        au_sys::kAudioUnitProperty_StreamFormat => {
            (mem::size_of::<au_sys::AudioStreamBasicDescription>(), true)
        }
        au_sys::kAudioUnitProperty_ParameterList => (
            state.parameters.len() * mem::size_of::<au_sys::AudioUnitParameterID>(),
            false,
        ),
        au_sys::kAudioUnitProperty_ParameterInfo => {
            (mem::size_of::<au_sys::AudioUnitParameterInfo>(), false)
        }
        au_sys::kAudioUnitProperty_SupportedNumChannels => {
            (mem::size_of::<au_sys::AUChannelInfo>(), false)
        }
        au_sys::kAudioUnitProperty_MaximumFramesPerSlice => (mem::size_of::<u32>(), true),
        au_sys::kAudioUnitProperty_CocoaUI => (mem::size_of::<au_sys::AUCocoaViewInfo>(), false),
        au_sys::kAudioUnitProperty_SetRenderCallback => {
            (mem::size_of::<au_sys::AURenderCallbackStruct>(), true)
        }
        K_GUI_HOST_STATE_PROPERTY => (mem::size_of::<*mut c_void>(), false),
        _ => return au_sys::kAudioUnitErr_InvalidProperty,
    };
    unsafe {
        if !out_data_size.is_null() {
            *out_data_size = size as u32;
        }
        if !out_writable.is_null() {
            *out_writable = writable as au_sys::Boolean;
        }
    }
    au_sys::noErr
}

unsafe extern "C" fn au_get_property(
    self_ptr: *mut c_void,
    in_id: au_sys::AudioUnitPropertyID,
    in_scope: au_sys::AudioUnitScope,
    in_element: au_sys::AudioUnitElement,
    out_data: *mut c_void,
    io_data_size: *mut au_sys::UInt32,
) -> au_sys::OSStatus {
    if out_data.is_null() || io_data_size.is_null() {
        return au_sys::kAudioUnitErr_InvalidParameter;
    }
    let state = unsafe { &(*(self_ptr as *const AuInstance)).state };
    match in_id {
        au_sys::kAudioUnitProperty_StreamFormat => {
            let channels = if in_scope == au_sys::kAudioUnitScope_Input {
                state.input_channels
            } else {
                state.output_channels
            };
            let asbd = au_sys::AudioStreamBasicDescription::noninterleaved_float32(
                state.sample_rate,
                channels,
            );
            unsafe { write_property(out_data, io_data_size, &asbd) };
            au_sys::noErr
        }
        au_sys::kAudioUnitProperty_ParameterList => {
            let ids: Vec<au_sys::AudioUnitParameterID> =
                state.parameters.iter().map(|p| p.id.0).collect();
            unsafe { write_property_slice(out_data, io_data_size, &ids) };
            au_sys::noErr
        }
        au_sys::kAudioUnitProperty_ParameterInfo => {
            let Some(param) = state.parameters.iter().find(|p| p.id.0 == in_element) else {
                return au_sys::kAudioUnitErr_InvalidElement;
            };
            let info = build_parameter_info(param);
            unsafe { write_property(out_data, io_data_size, &info) };
            au_sys::noErr
        }
        au_sys::kAudioUnitProperty_SupportedNumChannels => {
            let info = au_sys::AUChannelInfo {
                inChannels: state.input_channels as i16,
                outChannels: state.output_channels as i16,
            };
            unsafe { write_property(out_data, io_data_size, &info) };
            au_sys::noErr
        }
        au_sys::kAudioUnitProperty_MaximumFramesPerSlice => {
            unsafe { write_property(out_data, io_data_size, &state.max_frames) };
            au_sys::noErr
        }
        au_sys::kAudioUnitProperty_CocoaUI => {
            let info = build_cocoa_view_info();
            unsafe { write_property(out_data, io_data_size, &info) };
            au_sys::noErr
        }
        K_GUI_HOST_STATE_PROPERTY => {
            unsafe { write_property(out_data, io_data_size, &self_ptr) };
            au_sys::noErr
        }
        _ => au_sys::kAudioUnitErr_InvalidProperty,
    }
}

unsafe extern "C" fn au_set_property(
    self_ptr: *mut c_void,
    in_id: au_sys::AudioUnitPropertyID,
    in_scope: au_sys::AudioUnitScope,
    _in_element: au_sys::AudioUnitElement,
    in_data: *const c_void,
    in_data_size: au_sys::UInt32,
) -> au_sys::OSStatus {
    let state = unsafe { &mut (*(self_ptr as *mut AuInstance)).state };
    match in_id {
        au_sys::kAudioUnitProperty_StreamFormat => {
            if in_data.is_null()
                || (in_data_size as usize) < mem::size_of::<au_sys::AudioStreamBasicDescription>()
            {
                return au_sys::kAudioUnitErr_InvalidPropertyValue;
            }
            let asbd = unsafe { &*(in_data as *const au_sys::AudioStreamBasicDescription) };
            let expected = if in_scope == au_sys::kAudioUnitScope_Input {
                state.input_channels
            } else {
                state.output_channels
            };
            if asbd.mChannelsPerFrame != expected {
                // This plugin's channel layout is fixed (from
                // `Processor::channel_layout`); it doesn't renegotiate.
                return au_sys::kAudioUnitErr_FormatNotSupported;
            }
            state.sample_rate = asbd.mSampleRate;
            au_sys::noErr
        }
        au_sys::kAudioUnitProperty_MaximumFramesPerSlice => {
            if in_data.is_null() || (in_data_size as usize) < mem::size_of::<u32>() {
                return au_sys::kAudioUnitErr_InvalidPropertyValue;
            }
            state.max_frames = unsafe { *(in_data as *const u32) };
            state.resize_scratch();
            au_sys::noErr
        }
        au_sys::kAudioUnitProperty_SetRenderCallback => {
            if in_data.is_null()
                || (in_data_size as usize) < mem::size_of::<au_sys::AURenderCallbackStruct>()
            {
                return au_sys::kAudioUnitErr_InvalidPropertyValue;
            }
            state.input_render_callback =
                Some(unsafe { *(in_data as *const au_sys::AURenderCallbackStruct) });
            au_sys::noErr
        }
        _ => au_sys::kAudioUnitErr_InvalidProperty,
    }
}

/// Reads a parameter's current value. Deliberately goes through `gateway`
/// (seeded for every parameter in `au_initialize`) rather than
/// `state.processor.parameter_value(..)` directly: this can be called from
/// any thread the host likes, concurrently with `Render` (which mutates
/// `processor` on the audio thread via `apply_pending_parameters`), and the
/// gateway is the one piece of shared state actually designed for that.
unsafe extern "C" fn au_get_parameter(
    self_ptr: *mut c_void,
    in_id: au_sys::AudioUnitParameterID,
    _in_scope: au_sys::AudioUnitScope,
    _in_element: au_sys::AudioUnitElement,
    out_value: *mut au_sys::AudioUnitParameterValue,
) -> au_sys::OSStatus {
    if out_value.is_null() {
        return au_sys::kAudioUnitErr_InvalidParameter;
    }
    let state = unsafe { &(*(self_ptr as *const AuInstance)).state };
    let id = ParameterId(in_id);
    let Some(value) = state.gateway.get_normalized(id) else {
        return au_sys::kAudioUnitErr_InvalidParameter;
    };
    unsafe { *out_value = value.get() as f32 };
    au_sys::noErr
}

unsafe extern "C" fn au_set_parameter(
    self_ptr: *mut c_void,
    in_id: au_sys::AudioUnitParameterID,
    _in_scope: au_sys::AudioUnitScope,
    _in_element: au_sys::AudioUnitElement,
    in_value: au_sys::AudioUnitParameterValue,
    _in_buffer_offset_in_frames: au_sys::UInt32,
) -> au_sys::OSStatus {
    let state = unsafe { &(*(self_ptr as *const AuInstance)).state };
    state
        .gateway
        .set_normalized(ParameterId(in_id), NormalizedValue::new(in_value as f64));
    au_sys::noErr
}

unsafe extern "C" fn au_render(
    self_ptr: *mut c_void,
    io_action_flags: *mut au_sys::AudioUnitRenderActionFlags,
    in_time_stamp: *const au_sys::AudioTimeStamp,
    _in_output_bus_number: au_sys::UInt32,
    in_number_frames: au_sys::UInt32,
    io_data: *mut au_sys::AudioBufferList,
) -> au_sys::OSStatus {
    let state = unsafe { &mut (*(self_ptr as *mut AuInstance)).state };
    if !state.initialized {
        return au_sys::kAudioUnitErr_Uninitialized;
    }
    if io_data.is_null() {
        return au_sys::kAudioUnitErr_InvalidParameter;
    }
    let frames = (in_number_frames as usize).min(state.max_frames as usize);

    // Pull input via the "pull model": unlike a generator, an Effect AU's
    // `Render` is responsible for fetching its own input by calling back
    // through whatever the host registered via
    // `kAudioUnitProperty_SetRenderCallback`, rather than receiving it
    // directly. Falls back to silence if nothing is registered (or the
    // pull fails), so the unit still produces (processed-silence) output
    // instead of erroring out.
    if state.input_channels > 0 {
        match state.input_render_callback.and_then(|cb| cb.inputProc) {
            Some(input_proc) => {
                let callback = state
                    .input_render_callback
                    .expect("checked by match guard above");
                state.input_buffers.prepare(frames);
                let list_ptr = state.input_buffers.as_mut_ptr();
                let status = unsafe {
                    input_proc(
                        callback.inputProcRefCon,
                        io_action_flags,
                        in_time_stamp,
                        0,
                        in_number_frames,
                        list_ptr,
                    )
                };
                if status != au_sys::noErr {
                    state.input_buffers.silence(frames);
                }
            }
            None => state.input_buffers.silence(frames),
        }
    }

    apply_pending_parameters(&state.gateway, &mut *state.processor);

    // SAFETY: the host allocates `io_data` with one `AudioBuffer` per
    // output channel (matching `kAudioUnitProperty_StreamFormat`'s output
    // scope, which this unit reports as `state.output_channels`), each
    // with at least `frames` `Float32` samples of capacity, per the
    // negotiated `MaximumFramesPerSlice`.
    let out_buffers = unsafe { (*io_data).buffers_mut() };
    let mut outputs: Vec<&mut [f32]> = out_buffers
        .iter_mut()
        .map(|buffer| unsafe { std::slice::from_raw_parts_mut(buffer.mData as *mut f32, frames) })
        .collect();

    let inputs: Vec<&[f32]> = state
        .input_buffers
        .channel_data
        .iter()
        .map(|channel| &channel[..frames])
        .collect();
    state.processor.process(&inputs, &mut outputs);

    let peak = outputs
        .iter()
        .flat_map(|channel| channel.iter())
        .fold(0.0_f32, |max, &sample| max.max(sample.abs()));
    state.meter.write(peak);

    au_sys::noErr
}

unsafe extern "C" fn au_lookup(selector: au_sys::SInt16) -> Option<au_sys::AudioComponentMethod> {
    // SAFETY: every arm transmutes a concrete, selector-matched
    // `AudioUnit*Proc` function pointer into the opaque, variadic
    // `AudioComponentMethod` shape `Lookup` must return -- the same
    // "generic vtable slot" pattern the C API itself uses (callers already
    // know, from the selector they asked for, which concrete signature to
    // call through). Pointer-to-pointer, same size, no live data touched.
    unsafe {
        match selector {
            au_sys::kAudioUnitInitializeSelect => Some(mem::transmute::<
                au_sys::AudioUnitInitializeProc,
                au_sys::AudioComponentMethod,
            >(au_initialize)),
            au_sys::kAudioUnitUninitializeSelect => Some(mem::transmute::<
                au_sys::AudioUnitUninitializeProc,
                au_sys::AudioComponentMethod,
            >(au_uninitialize)),
            au_sys::kAudioUnitGetPropertyInfoSelect => Some(mem::transmute::<
                au_sys::AudioUnitGetPropertyInfoProc,
                au_sys::AudioComponentMethod,
            >(au_get_property_info)),
            au_sys::kAudioUnitGetPropertySelect => Some(mem::transmute::<
                au_sys::AudioUnitGetPropertyProc,
                au_sys::AudioComponentMethod,
            >(au_get_property)),
            au_sys::kAudioUnitSetPropertySelect => Some(mem::transmute::<
                au_sys::AudioUnitSetPropertyProc,
                au_sys::AudioComponentMethod,
            >(au_set_property)),
            au_sys::kAudioUnitGetParameterSelect => Some(mem::transmute::<
                au_sys::AudioUnitGetParameterProc,
                au_sys::AudioComponentMethod,
            >(au_get_parameter)),
            au_sys::kAudioUnitSetParameterSelect => Some(mem::transmute::<
                au_sys::AudioUnitSetParameterProc,
                au_sys::AudioComponentMethod,
            >(au_set_parameter)),
            au_sys::kAudioUnitResetSelect => Some(mem::transmute::<
                au_sys::AudioUnitResetProc,
                au_sys::AudioComponentMethod,
            >(au_reset)),
            au_sys::kAudioUnitRenderSelect => Some(mem::transmute::<
                au_sys::AudioUnitRenderProc,
                au_sys::AudioComponentMethod,
            >(au_render)),
            _ => None,
        }
    }
}

/// Allocates and returns a new AU instance. Called by the
/// `#[unsafe(no_mangle)] extern "C"` factory function [`crate::au_entry!`]
/// generates; not meant to be called directly.
pub fn create_instance<F>(
    parameters: Vec<ParameterInfo>,
    make_processor: impl FnOnce() -> Box<dyn Processor>,
    make_editor: F,
) -> *mut au_sys::AudioComponentPlugInInterface
where
    F: FnOnce(Arc<LockFreeParameterGateway>, PeakMeter) -> Box<dyn PluginEditor> + 'static,
{
    let processor = make_processor();
    let layout = processor.channel_layout();
    const DEFAULT_MAX_FRAMES: u32 = 512;

    let state = AuState {
        processor,
        gateway: Arc::new(LockFreeParameterGateway::default()),
        meter: PeakMeter::new(),
        make_editor: Some(Box::new(make_editor)),
        parameters,
        sample_rate: 44_100.0,
        max_frames: DEFAULT_MAX_FRAMES,
        input_channels: layout.input_channels,
        output_channels: layout.output_channels,
        initialized: false,
        input_render_callback: None,
        input_buffers: InputBufferList::new(
            layout.input_channels.max(1) as usize,
            DEFAULT_MAX_FRAMES as usize,
        ),
        au_instance: std::ptr::null_mut(),
    };

    let instance = Box::new(AuInstance {
        interface: au_sys::AudioComponentPlugInInterface {
            Open: au_open,
            Close: au_close,
            Lookup: au_lookup,
            reserved: std::ptr::null_mut(),
        },
        state,
    });

    Box::into_raw(instance) as *mut au_sys::AudioComponentPlugInInterface
}

/// Consumes the stored editor factory (if it hasn't already been used) and
/// constructs the `PluginEditor`, sharing the same gateway/meter `Render`
/// uses. Called by `editor.rs`'s `AUCocoaUIBase` factory class once it has
/// recovered `instance_ptr` from the host's `AudioUnit` handle via
/// [`K_GUI_HOST_STATE_PROPERTY`].
///
/// Returns `None` if `instance_ptr` is null or the editor was already
/// created once before (this crate, like `gui-vst3`, only supports
/// constructing the editor a single time per instance).
///
/// # Safety
/// `instance_ptr` must be a currently-alive pointer this module's
/// `create_instance` returned (i.e. the same value `Open`/`Close`/`Lookup`
/// receive as `self_ptr`).
pub(crate) unsafe fn build_editor(instance_ptr: *mut c_void) -> Option<Box<dyn PluginEditor>> {
    if instance_ptr.is_null() {
        return None;
    }
    let state = unsafe { &mut (*(instance_ptr as *mut AuInstance)).state };
    let make_editor = state.make_editor.take()?;
    Some(make_editor(state.gateway.clone(), state.meter.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gui_host::{ChannelLayout, PluginEditor, SizeConstraints};

    const GAIN_PARAM: ParameterId = ParameterId(1);

    struct TestGainProcessor {
        gain: NormalizedValue,
    }

    impl Processor for TestGainProcessor {
        fn prepare(&mut self, _sample_rate: f64, _max_block_size: usize) {}

        fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]]) {
            let gain = self.gain.get() as f32;
            for (input, output) in inputs.iter().zip(outputs.iter_mut()) {
                for (sample_in, sample_out) in input.iter().zip(output.iter_mut()) {
                    *sample_out = sample_in * gain;
                }
            }
        }

        fn reset(&mut self) {}

        fn channel_layout(&self) -> ChannelLayout {
            ChannelLayout {
                input_channels: 1,
                output_channels: 1,
            }
        }

        fn parameters(&self) -> &[ParameterInfo] {
            &[]
        }

        fn set_parameter(&mut self, id: ParameterId, value: NormalizedValue) {
            if id == GAIN_PARAM {
                self.gain = value;
            }
        }

        fn parameter_value(&self, id: ParameterId) -> NormalizedValue {
            if id == GAIN_PARAM {
                self.gain
            } else {
                NormalizedValue::new(0.0)
            }
        }
    }

    struct NoOpEditor;

    impl PluginEditor for NoOpEditor {
        fn open(
            &mut self,
            _parent: gui_host::ParentWindowHandle,
            _host: &dyn gui_host::EditorHost,
        ) {
        }
        fn close(&mut self) {}
        fn resize(&mut self, _size: gui_core::Sizef) {}
        fn idle(&mut self) {}
        fn on_parameter_changed(&mut self, _id: ParameterId, _value: NormalizedValue) {}
        fn size_constraints(&self) -> SizeConstraints {
            SizeConstraints::default()
        }
    }

    fn parameters() -> Vec<ParameterInfo> {
        vec![ParameterInfo {
            id: GAIN_PARAM,
            name: "Gain",
            default_value: NormalizedValue::new(1.0),
            min_value: NormalizedValue::new(0.0),
            max_value: NormalizedValue::new(1.0),
            step_count: None,
        }]
    }

    /// Builds one instance and runs it through the exact call sequence a
    /// real host would: `Open` -> `Lookup` each selector it needs ->
    /// `Initialize` -> property queries -> parameter get/set -> `Render`
    /// (twice, to also exercise the `apply_pending_parameters` drain) ->
    /// `Close`. Nothing here touches Cocoa/AudioToolbox -- it validates the
    /// pure-Rust half of the dispatch table (everything except the
    /// `AUCocoaUIBase` factory method, which needs a live host/ObjC
    /// runtime to invoke).
    #[test]
    fn full_instance_lifecycle_processes_real_audio() {
        unsafe {
            let raw = create_instance(
                parameters(),
                || {
                    Box::new(TestGainProcessor {
                        gain: NormalizedValue::new(1.0),
                    }) as Box<dyn Processor>
                },
                |_gateway, _meter| Box::new(NoOpEditor) as Box<dyn PluginEditor>,
            );
            assert!(!raw.is_null());

            let interface = &*raw;
            let self_ptr = raw as *mut c_void;

            assert_eq!(
                (interface.Open)(self_ptr, std::ptr::null_mut()),
                au_sys::noErr
            );

            let lookup = interface.Lookup;
            let initialize: au_sys::AudioUnitInitializeProc =
                mem::transmute(lookup(au_sys::kAudioUnitInitializeSelect).unwrap());
            let get_property_info: au_sys::AudioUnitGetPropertyInfoProc =
                mem::transmute(lookup(au_sys::kAudioUnitGetPropertyInfoSelect).unwrap());
            let get_property: au_sys::AudioUnitGetPropertyProc =
                mem::transmute(lookup(au_sys::kAudioUnitGetPropertySelect).unwrap());
            let set_parameter: au_sys::AudioUnitSetParameterProc =
                mem::transmute(lookup(au_sys::kAudioUnitSetParameterSelect).unwrap());
            let get_parameter: au_sys::AudioUnitGetParameterProc =
                mem::transmute(lookup(au_sys::kAudioUnitGetParameterSelect).unwrap());
            let render: au_sys::AudioUnitRenderProc =
                mem::transmute(lookup(au_sys::kAudioUnitRenderSelect).unwrap());
            let reset: au_sys::AudioUnitResetProc =
                mem::transmute(lookup(au_sys::kAudioUnitResetSelect).unwrap());
            assert!(lookup(au_sys::kAudioUnitAddPropertyListenerSelect).is_none());

            assert_eq!(initialize(self_ptr), au_sys::noErr);

            // ParameterList should report exactly our one parameter ID.
            let mut size: u32 = 0;
            let mut writable: u8 = 0;
            assert_eq!(
                get_property_info(
                    self_ptr,
                    au_sys::kAudioUnitProperty_ParameterList,
                    au_sys::kAudioUnitScope_Global,
                    0,
                    &mut size,
                    &mut writable,
                ),
                au_sys::noErr
            );
            assert_eq!(
                size as usize,
                mem::size_of::<au_sys::AudioUnitParameterID>()
            );
            let mut id_out: au_sys::AudioUnitParameterID = 0;
            let mut io_size = size;
            assert_eq!(
                get_property(
                    self_ptr,
                    au_sys::kAudioUnitProperty_ParameterList,
                    au_sys::kAudioUnitScope_Global,
                    0,
                    &mut id_out as *mut _ as *mut c_void,
                    &mut io_size,
                ),
                au_sys::noErr
            );
            assert_eq!(id_out, GAIN_PARAM.0);

            // StreamFormat should reflect the processor's declared layout.
            let mut asbd = au_sys::AudioStreamBasicDescription::default();
            let mut asbd_size = mem::size_of::<au_sys::AudioStreamBasicDescription>() as u32;
            assert_eq!(
                get_property(
                    self_ptr,
                    au_sys::kAudioUnitProperty_StreamFormat,
                    au_sys::kAudioUnitScope_Output,
                    0,
                    &mut asbd as *mut _ as *mut c_void,
                    &mut asbd_size,
                ),
                au_sys::noErr
            );
            assert_eq!(asbd.mChannelsPerFrame, 1);

            // SetParameter -> GetParameter round-trips through the gateway.
            assert_eq!(
                set_parameter(
                    self_ptr,
                    GAIN_PARAM.0,
                    au_sys::kAudioUnitScope_Global,
                    0,
                    0.5,
                    0
                ),
                au_sys::noErr
            );
            let mut value: f32 = -1.0;
            assert_eq!(
                get_parameter(
                    self_ptr,
                    GAIN_PARAM.0,
                    au_sys::kAudioUnitScope_Global,
                    0,
                    &mut value
                ),
                au_sys::noErr
            );
            assert_eq!(value, 0.5);

            // Render with no input callback registered: input is silence,
            // so output (silence * gain) should also be silence.
            const FRAMES: usize = 8;
            let mut output_samples = [1.0_f32; FRAMES]; // pre-fill with nonzero to prove Render overwrites it
            let mut output_buffer = au_sys::AudioBuffer {
                mNumberChannels: 1,
                mDataByteSize: (FRAMES * mem::size_of::<f32>()) as u32,
                mData: output_samples.as_mut_ptr() as *mut c_void,
            };
            let mut output_list = au_sys::AudioBufferList {
                mNumberBuffers: 1,
                mBuffers: [output_buffer],
            };
            let mut flags: au_sys::AudioUnitRenderActionFlags = 0;
            let timestamp = au_sys::AudioTimeStamp::default();
            assert_eq!(
                render(
                    self_ptr,
                    &mut flags,
                    &timestamp,
                    0,
                    FRAMES as u32,
                    &mut output_list,
                ),
                au_sys::noErr
            );
            assert_eq!(output_samples, [0.0_f32; FRAMES]);

            // A second Render call (after another SetParameter) exercises
            // `apply_pending_parameters` draining a queued change.
            assert_eq!(
                set_parameter(
                    self_ptr,
                    GAIN_PARAM.0,
                    au_sys::kAudioUnitScope_Global,
                    0,
                    1.0,
                    0
                ),
                au_sys::noErr
            );
            output_buffer.mData = output_samples.as_mut_ptr() as *mut c_void;
            output_list.mBuffers = [output_buffer];
            assert_eq!(
                render(
                    self_ptr,
                    &mut flags,
                    &timestamp,
                    0,
                    FRAMES as u32,
                    &mut output_list,
                ),
                au_sys::noErr
            );
            assert_eq!(output_samples, [0.0_f32; FRAMES]); // still silence in, silence out

            assert_eq!(
                reset(self_ptr, au_sys::kAudioUnitScope_Global, 0),
                au_sys::noErr
            );
            assert_eq!((interface.Close)(self_ptr), au_sys::noErr);
        }
    }

    #[test]
    fn render_pulls_input_via_registered_callback() {
        unsafe extern "C" fn constant_input_callback(
            ref_con: *mut c_void,
            _io_action_flags: *mut au_sys::AudioUnitRenderActionFlags,
            _in_time_stamp: *const au_sys::AudioTimeStamp,
            _in_bus_number: u32,
            in_number_frames: u32,
            io_data: *mut au_sys::AudioBufferList,
        ) -> au_sys::OSStatus {
            unsafe {
                let value = *(ref_con as *const f32);
                let buffers = (*io_data).buffers_mut();
                for buffer in buffers {
                    let samples = std::slice::from_raw_parts_mut(
                        buffer.mData as *mut f32,
                        in_number_frames as usize,
                    );
                    samples.fill(value);
                }
            }
            au_sys::noErr
        }

        unsafe {
            let raw = create_instance(
                parameters(),
                || {
                    Box::new(TestGainProcessor {
                        gain: NormalizedValue::new(1.0),
                    }) as Box<dyn Processor>
                },
                |_gateway, _meter| Box::new(NoOpEditor) as Box<dyn PluginEditor>,
            );
            let interface = &*raw;
            let self_ptr = raw as *mut c_void;
            assert_eq!(
                (interface.Open)(self_ptr, std::ptr::null_mut()),
                au_sys::noErr
            );

            let lookup = interface.Lookup;
            let initialize: au_sys::AudioUnitInitializeProc =
                mem::transmute(lookup(au_sys::kAudioUnitInitializeSelect).unwrap());
            let set_property: au_sys::AudioUnitSetPropertyProc =
                mem::transmute(lookup(au_sys::kAudioUnitSetPropertySelect).unwrap());
            let render: au_sys::AudioUnitRenderProc =
                mem::transmute(lookup(au_sys::kAudioUnitRenderSelect).unwrap());

            assert_eq!(initialize(self_ptr), au_sys::noErr);

            let mut input_value = 0.25_f32;
            let callback = au_sys::AURenderCallbackStruct {
                inputProc: Some(constant_input_callback),
                inputProcRefCon: &mut input_value as *mut f32 as *mut c_void,
            };
            assert_eq!(
                set_property(
                    self_ptr,
                    au_sys::kAudioUnitProperty_SetRenderCallback,
                    au_sys::kAudioUnitScope_Input,
                    0,
                    &callback as *const _ as *const c_void,
                    mem::size_of::<au_sys::AURenderCallbackStruct>() as u32,
                ),
                au_sys::noErr
            );

            const FRAMES: usize = 4;
            let mut output_samples = [0.0_f32; FRAMES];
            let mut output_list = au_sys::AudioBufferList {
                mNumberBuffers: 1,
                mBuffers: [au_sys::AudioBuffer {
                    mNumberChannels: 1,
                    mDataByteSize: (FRAMES * mem::size_of::<f32>()) as u32,
                    mData: output_samples.as_mut_ptr() as *mut c_void,
                }],
            };
            let mut flags: au_sys::AudioUnitRenderActionFlags = 0;
            let timestamp = au_sys::AudioTimeStamp::default();
            assert_eq!(
                render(
                    self_ptr,
                    &mut flags,
                    &timestamp,
                    0,
                    FRAMES as u32,
                    &mut output_list,
                ),
                au_sys::noErr
            );
            // Unity gain: output should equal the pulled input exactly.
            assert_eq!(output_samples, [0.25_f32; FRAMES]);

            assert_eq!((interface.Close)(self_ptr), au_sys::noErr);
        }
    }
}
