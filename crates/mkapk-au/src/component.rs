//! A real AUv2 plugin entry point: `AudioComponentFactoryFunction` +
//! `AudioComponentPlugInInterface` vtable (`Open`/`Close`/`Lookup`), backed
//! by `au-sys`'s hand-written bindings to the C API (the AU equivalent of
//! what `vst3-sys` gives `mkapk-vst3`, minus a higher-level dispatch macro --
//! there isn't one for AUv2, so the vtable dispatch here is hand-written).
//!
//! Scope: a minimal, real, `kAudioUnitType_Effect`-style Audio Unit --
//! fixed channel counts (from `Processor::channel_layout`), one input
//! element, one output element, `Float32` non-interleaved only. Supports
//! enough properties/selectors for a host to load, connect, automate, and
//! render real audio through it: `Initialize`/`Uninitialize`/`Reset`,
//! stream format, parameter list/info, supported channel counts, max
//! frames per slice, the render callback used to pull input (the AUv2
//! "pull" model), `Render` itself, and `CocoaUI` (wired to `mkapk-au`'s
//! existing Cocoa view code via a small private property -- see
//! `editor.rs`'s `AUCocoaUIBase` factory class).
//!
//! Real MIDI delivery (`MusicDeviceMIDIEvent`, see
//! `K_MUSIC_DEVICE_MIDI_EVENT_SELECT`/`au_music_device_midi_event`) is
//! wired up whenever `Processor::accepts_midi` returns `true`; `xtask`'s
//! `bundle-au` then registers the component as `aumf`/`aumu` instead of
//! `aufx` (see `plugins/gain/src/bin/au_info.rs`), since real hosts only
//! ever route MIDI to a Music Effect/Music Device component.
//!
//! Not implemented (spec-legal to omit; hosts fall back to defaults/ignore):
//! factory presets, class-info (session state) persistence.
//!
//! **Verified against Apple's real `auval`** (not just reviewed against
//! headers): several real bugs were found and fixed this way that would
//! not have been caught otherwise --
//! - the manufacturer four-character code needs at least one non-lowercase
//!   letter, or the Component Manager rejects it outright;
//! - `Info.plist`'s `AudioComponents.version` must be a plist `<integer>`,
//!   not a `<string>` -- with a string there, `AudioComponentRegistrar`
//!   silently drops the whole entry and the component never registers at
//!   all;
//! - `kAudioUnitProperty_ElementCount` (bus count) needs a real answer, not
//!   `kAudioUnitErr_InvalidProperty`, or every channel-format/render test
//!   downstream fails too;
//! - parameters must only be reported in `kAudioUnitScope_Global` --
//!   reporting the same parameter ID in every scope let `auval`'s
//!   independent-per-scope get/set probing collide on the single
//!   underlying gateway value and see it as state corruption;
//! - `AddPropertyListener` must at least succeed (a real no-op is fine;
//!   returning "unimplemented" via `Lookup` failed the recommended
//!   properties section);
//! - `kAudioUnitProperty_CocoaUI`'s bundle location can't be left null in
//!   practice (see `own_bundle_url`, resolved via `dladdr`);
//! - `auval`'s "Test MIDI" step only exercises `MusicDeviceMIDIEvent` at
//!   all when the component is registered as `aumf`/`aumu`, not `aufx` --
//!   confirmed by first seeing it silently skipped, then real, passing
//!   MIDI dispatch once `bundle-au` switched the registered type.
#![cfg(target_os = "macos")]

use core::ffi::{c_char, c_void};
use std::ffi::CStr;
use std::mem;
use std::path::Path;
use std::sync::Arc;

use mkapk_host::{
    LockFreeParameterGateway, MidiEventQueue, MidiMessage, NormalizedValue, ParameterGateway,
    ParameterId, ParameterInfo, PeakMeter, PluginEditor, Processor, apply_pending_midi,
    apply_pending_parameters,
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
    bypassed: bool,
    input_render_callback: Option<au_sys::AURenderCallbackStruct>,
    connection: Option<AudioUnitConnection>,
    input_buffers: InputBufferList,
    output_scratch: Vec<Vec<f32>>,
    property_listeners: Vec<PropertyListener>,
    render_notifies: Vec<RenderNotify>,
    /// `Some` only when `Processor::accepts_midi` returns `true` (checked
    /// once, in `create_instance`): real MIDI delivered via
    /// `MusicDeviceMIDIEvent` (`au_music_device_midi_event`) is pushed
    /// here, then drained into `processor.handle_midi` at the start of
    /// every `Render` call. A real, thread-safe queue rather than a plain
    /// `Vec` because nothing in the AU spec guarantees a host calls
    /// `MusicDeviceMIDIEvent` from the same thread as `Render`.
    midi_queue: Option<MidiEventQueue>,
    au_instance: au_sys::AudioUnit,
}

impl AuState {
    fn resize_scratch(&mut self) {
        self.input_buffers = InputBufferList::new(
            self.input_channels.max(1) as usize,
            self.max_frames as usize,
        );
        self.output_scratch = vec![
            vec![0.0_f32; self.max_frames.max(1) as usize];
            self.output_channels.max(1) as usize
        ];
    }

    /// Calls every listener registered (via `AddPropertyListener`) for
    /// `property_id`. Real hosts rely on this: `auval`'s recommended-
    /// properties check specifically verifies that changing
    /// `MaximumFramesPerSlice` fires a notification to a listener it
    /// registers beforehand.
    fn notify_property_changed(
        &self,
        property_id: au_sys::AudioUnitPropertyID,
        scope: au_sys::AudioUnitScope,
        element: au_sys::AudioUnitElement,
    ) {
        for listener in &self.property_listeners {
            if listener.property_id == property_id {
                unsafe {
                    (listener.proc)(
                        listener.user_data,
                        self.au_instance,
                        property_id,
                        scope,
                        element,
                    )
                };
            }
        }
    }

    /// Calls every listener registered via `AudioUnitAddRenderNotify`,
    /// passing through the same render arguments `Render` itself got (with
    /// `flags` carrying whichever of `kAudioUnitRenderAction_PreRender`/
    /// `PostRender` the caller set), matching real hosts' expectations for
    /// render-notify hooks (used for things like VU meters that need to
    /// observe every render call, not just query state after the fact).
    #[allow(clippy::too_many_arguments)]
    fn call_render_notifies(
        &self,
        flags: au_sys::AudioUnitRenderActionFlags,
        in_time_stamp: *const au_sys::AudioTimeStamp,
        bus: au_sys::UInt32,
        frames: au_sys::UInt32,
        io_data: *mut au_sys::AudioBufferList,
    ) {
        for notify in &self.render_notifies {
            let mut local_flags = flags;
            unsafe {
                (notify.proc)(
                    notify.user_data,
                    &mut local_flags,
                    in_time_stamp,
                    bus,
                    frames,
                    io_data,
                )
            };
        }
    }
}

/// One `AddPropertyListener` registration. `proc`/`user_data` are opaque to
/// us -- we only ever call them back, per `notify_property_changed`, never
/// inspect them.
struct PropertyListener {
    property_id: au_sys::AudioUnitPropertyID,
    proc: au_sys::AudioUnitPropertyListenerProc,
    user_data: *mut c_void,
}

/// One `AudioUnitAddRenderNotify` registration; see `call_render_notifies`.
struct RenderNotify {
    proc: au_sys::AURenderCallback,
    user_data: *mut c_void,
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

/// `dlfcn.h`'s `dladdr` output: enough to recover the file path of the
/// dynamic library a given code address lives in. Declared locally since
/// `au-sys` (a Core Audio-specific bindings crate) has no reason to cover
/// it; it's in `libSystem`, always linked, no extra `#[link]` needed.
#[repr(C)]
struct DlInfo {
    dli_fname: *const c_char,
    dli_fbase: *mut c_void,
    dli_sname: *const c_char,
    dli_saddr: *mut c_void,
}

unsafe extern "C" {
    fn dladdr(addr: *const c_void, info: *mut DlInfo) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFURLCreateWithFileSystemPath(
        allocator: *mut c_void,
        file_path: au_sys::CFStringRef,
        path_style: i32,
        is_directory: u8,
    ) -> *mut c_void;
}

const K_CF_URL_POSIX_PATH_STYLE: i32 = 0;

/// `kAudioUnitProperty_MakeConnection`'s value type. Not in `au-sys`
/// (bindings for it weren't needed until this connection was actually
/// exercised, by a real host's audio-graph wiring rather than a render
/// callback); matches `AUComponent.h`'s `AudioUnitConnection` layout.
#[repr(C)]
#[derive(Clone, Copy)]
struct AudioUnitConnection {
    source_audio_unit: au_sys::AudioUnit,
    source_output_number: au_sys::UInt32,
    dest_input_number: au_sys::UInt32,
}

// The real, host-callable `AudioUnitRender`, needed to pull input directly
// from another Audio Unit once connected via `kAudioUnitProperty_MakeConnection`
// (the alternative to the render-callback pull model this unit already
// supported): confirmed necessary via `auval`'s "Checking connection
// semantics" test, which failed with `kAudioUnitErr_InvalidProperty`
// before `MakeConnection` was handled at all.
#[link(name = "AudioToolbox", kind = "framework")]
unsafe extern "C" {
    fn AudioUnitRender(
        in_unit: au_sys::AudioUnit,
        io_action_flags: *mut au_sys::AudioUnitRenderActionFlags,
        in_time_stamp: *const au_sys::AudioTimeStamp,
        in_output_bus_number: au_sys::UInt32,
        in_number_frames: au_sys::UInt32,
        io_data: *mut au_sys::AudioBufferList,
    ) -> au_sys::OSStatus;
}

/// `kAudioUnitProperty_ScheduleParameters`'s event type. Not in `au-sys`;
/// layout taken directly from the real `AUComponent.h` header (not
/// guessed): `scope`/`element`/`parameter`/`eventType` each `UInt32`,
/// followed by a union of the ramped and immediate event shapes.
const K_PARAMETER_EVENT_RAMPED: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
struct AudioUnitParameterEventRamp {
    start_buffer_offset: i32,
    duration_in_frames: u32,
    start_value: f32,
    end_value: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AudioUnitParameterEventImmediate {
    buffer_offset: u32,
    value: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
union AudioUnitParameterEventValues {
    ramp: AudioUnitParameterEventRamp,
    immediate: AudioUnitParameterEventImmediate,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AudioUnitParameterEvent {
    scope: au_sys::AudioUnitScope,
    element: au_sys::AudioUnitElement,
    parameter: au_sys::AudioUnitParameterID,
    event_type: u32,
    event_values: AudioUnitParameterEventValues,
}

/// Finds this plugin's own `.component` bundle directory at runtime and
/// returns a `CFURLRef` to it (as an opaque `*mut c_void`, matching how
/// `au-sys`'s `AUCocoaViewInfo` field is typed), or null on any failure.
///
/// There's no "give me my own CFBundle" API to call directly; the
/// standard trick (used by hand-written AU plugins generally, not
/// something specific to this project) is `dladdr` on the address of a
/// function known to live in this same dylib, which yields the loaded
/// image's file path (".../Gain.component/Contents/MacOS/Gain"), then
/// walking up three path components (MacOS/Gain -> Contents -> the bundle
/// root) to recover the bundle directory.
fn own_bundle_url() -> *mut c_void {
    unsafe {
        let mut info: DlInfo = mem::zeroed();
        let addr = build_cocoa_view_info as *const () as *const c_void;
        if dladdr(addr, &mut info) == 0 || info.dli_fname.is_null() {
            return std::ptr::null_mut();
        }
        let path = CStr::from_ptr(info.dli_fname)
            .to_string_lossy()
            .into_owned();
        let Some(bundle_path) = Path::new(&path)
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
        else {
            return std::ptr::null_mut();
        };
        let cf_path = au_sys::cf_string_create(&bundle_path.to_string_lossy());
        if cf_path.is_null() {
            return std::ptr::null_mut();
        }
        let url = CFURLCreateWithFileSystemPath(
            std::ptr::null_mut(),
            cf_path,
            K_CF_URL_POSIX_PATH_STYLE,
            1,
        );
        au_sys::cf_release(cf_path);
        url
    }
}

fn build_parameter_info(param: &ParameterInfo) -> au_sys::AudioUnitParameterInfo {
    let mut name = [0 as core::ffi::c_char; 52];
    for (slot, byte) in name.iter_mut().zip(param.name.as_bytes()) {
        *slot = *byte as core::ffi::c_char;
    }
    // SAFETY: `cf_string_create` wraps `CFStringCreateWithBytes`, a
    // standard CoreFoundation call.
    let cf_name = unsafe { au_sys::cf_string_create(param.name) };
    au_sys::AudioUnitParameterInfo {
        name,
        unitName: std::ptr::null_mut(),
        clumpID: 0,
        cfNameString: cf_name,
        unit: au_sys::kAudioUnitParameterUnit_Generic,
        minValue: param.min_value.get() as f32,
        maxValue: param.max_value.get() as f32,
        defaultValue: param.default_value.get() as f32,
        flags: au_sys::kAudioUnitParameterFlag_IsReadable
            | au_sys::kAudioUnitParameterFlag_IsWritable
            | au_sys::kAudioUnitParameterFlag_HasCFNameString
            | au_sys::kAudioUnitParameterFlag_CFNameRelease,
    }
}

/// Builds the `AUCocoaViewInfo` response for `kAudioUnitProperty_CocoaUI`.
///
/// Per Core Audio's "copy" convention for CF-typed property getters, both
/// returned CF objects are retained and ownership transfers to the caller
/// (the host), which must release them. `mCocoaAUViewBundleLocation` is a
/// real URL (see `own_bundle_url`) -- leaving it null compiled and ran, but
/// real-host testing via `auval` showed the custom UI couldn't actually be
/// retrieved that way in practice, despite some documentation suggesting
/// null should be tolerated as "the same bundle as the factory function".
fn build_cocoa_view_info() -> au_sys::AUCocoaViewInfo {
    // SAFETY: `cf_string_create` just wraps `CFStringCreateWithBytes`, a
    // standard CoreFoundation call; the class name is a fixed ASCII
    // literal.
    let class_name = unsafe { au_sys::cf_string_create(crate::editor::FACTORY_CLASS_NAME) };
    au_sys::AUCocoaViewInfo {
        mCocoaAUViewBundleLocation: own_bundle_url(),
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
    // On the *first* Initialize, the gateway has no value yet for any
    // parameter, so seed it from the processor's own default -- this is
    // also what lets `au_get_parameter` read the gateway instead of
    // `processor` directly (see its doc comment). On a *later*
    // re-Initialize (hosts do this, e.g. auval's "retain value across
    // reset and initialization" check), the gateway already holds
    // whatever the user/host last set, so push *that* into `processor`
    // instead -- overwriting it from `processor` unconditionally, as an
    // earlier version of this code did, clobbered the user's last-set
    // value back to the plugin's default on every re-Initialize.
    for param in &state.parameters {
        match state.gateway.get_normalized(param.id) {
            Some(value) => state.processor.set_parameter(param.id, value),
            None => {
                let value = state.processor.parameter_value(param.id);
                state.gateway.set_normalized(param.id, value);
            }
        }
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
    in_scope: au_sys::AudioUnitScope,
    _in_element: au_sys::AudioUnitElement,
    out_data_size: *mut au_sys::UInt32,
    out_writable: *mut au_sys::Boolean,
) -> au_sys::OSStatus {
    let state = unsafe { &(*(self_ptr as *const AuInstance)).state };
    let (size, writable): (usize, bool) = match in_id {
        au_sys::kAudioUnitProperty_StreamFormat => {
            (mem::size_of::<au_sys::AudioStreamBasicDescription>(), true)
        }
        // This plugin's only parameters live in Global scope; other scopes
        // (Input/Output/Group/Part/Note) don't have their own independent
        // copies of parameter 1 -- reporting the same parameter as present
        // in every scope (as an earlier version of this code did) made
        // `auval` treat them as separate instances sharing one underlying
        // value, which it flagged as state corruption.
        au_sys::kAudioUnitProperty_ParameterList if in_scope == au_sys::kAudioUnitScope_Global => (
            state.parameters.len() * mem::size_of::<au_sys::AudioUnitParameterID>(),
            false,
        ),
        au_sys::kAudioUnitProperty_ParameterInfo if in_scope == au_sys::kAudioUnitScope_Global => {
            (mem::size_of::<au_sys::AudioUnitParameterInfo>(), false)
        }
        au_sys::kAudioUnitProperty_SupportedNumChannels => {
            (mem::size_of::<au_sys::AUChannelInfo>(), false)
        }
        au_sys::kAudioUnitProperty_MaximumFramesPerSlice => (mem::size_of::<u32>(), true),
        au_sys::kAudioUnitProperty_ElementCount => (mem::size_of::<u32>(), false),
        // Global-scope-only, like the parameter properties above: `auval`
        // warned about these being reported as valid in every scope too.
        au_sys::kAudioUnitProperty_Latency if in_scope == au_sys::kAudioUnitScope_Global => {
            (mem::size_of::<f64>(), false)
        }
        au_sys::kAudioUnitProperty_TailTime if in_scope == au_sys::kAudioUnitScope_Global => {
            (mem::size_of::<f64>(), false)
        }
        au_sys::kAudioUnitProperty_BypassEffect if in_scope == au_sys::kAudioUnitScope_Global => {
            (mem::size_of::<u32>(), true)
        }
        au_sys::kAudioUnitProperty_CocoaUI => (mem::size_of::<au_sys::AUCocoaViewInfo>(), false),
        au_sys::kAudioUnitProperty_SetRenderCallback => {
            (mem::size_of::<au_sys::AURenderCallbackStruct>(), true)
        }
        au_sys::kAudioUnitProperty_MakeConnection => (mem::size_of::<AudioUnitConnection>(), true),
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
        au_sys::kAudioUnitProperty_ParameterList if in_scope == au_sys::kAudioUnitScope_Global => {
            let ids: Vec<au_sys::AudioUnitParameterID> =
                state.parameters.iter().map(|p| p.id.0).collect();
            unsafe { write_property_slice(out_data, io_data_size, &ids) };
            au_sys::noErr
        }
        au_sys::kAudioUnitProperty_ParameterInfo if in_scope == au_sys::kAudioUnitScope_Global => {
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
        au_sys::kAudioUnitProperty_ElementCount => {
            // Fixed topology: exactly one element in every scope, except
            // Input when the processor declares no input channels at all.
            let count: u32 =
                if in_scope == au_sys::kAudioUnitScope_Input && state.input_channels == 0 {
                    0
                } else {
                    1
                };
            unsafe { write_property(out_data, io_data_size, &count) };
            au_sys::noErr
        }
        au_sys::kAudioUnitProperty_Latency if in_scope == au_sys::kAudioUnitScope_Global => {
            let latency_seconds: f64 = 0.0;
            unsafe { write_property(out_data, io_data_size, &latency_seconds) };
            au_sys::noErr
        }
        au_sys::kAudioUnitProperty_TailTime if in_scope == au_sys::kAudioUnitScope_Global => {
            let tail_seconds: f64 = 0.0;
            unsafe { write_property(out_data, io_data_size, &tail_seconds) };
            au_sys::noErr
        }
        au_sys::kAudioUnitProperty_BypassEffect if in_scope == au_sys::kAudioUnitScope_Global => {
            let bypassed: u32 = state.bypassed as u32;
            unsafe { write_property(out_data, io_data_size, &bypassed) };
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
    in_element: au_sys::AudioUnitElement,
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
            state.notify_property_changed(in_id, in_scope, in_element);
            au_sys::noErr
        }
        au_sys::kAudioUnitProperty_MaximumFramesPerSlice => {
            if in_data.is_null() || (in_data_size as usize) < mem::size_of::<u32>() {
                return au_sys::kAudioUnitErr_InvalidPropertyValue;
            }
            state.max_frames = unsafe { *(in_data as *const u32) };
            state.resize_scratch();
            state.notify_property_changed(in_id, in_scope, in_element);
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
        au_sys::kAudioUnitProperty_MakeConnection => {
            if in_data.is_null() || (in_data_size as usize) < mem::size_of::<AudioUnitConnection>()
            {
                return au_sys::kAudioUnitErr_InvalidPropertyValue;
            }
            let connection = unsafe { *(in_data as *const AudioUnitConnection) };
            state.connection = if connection.source_audio_unit.is_null() {
                None
            } else {
                Some(connection)
            };
            au_sys::noErr
        }
        au_sys::kAudioUnitProperty_BypassEffect => {
            if in_data.is_null() || (in_data_size as usize) < mem::size_of::<u32>() {
                return au_sys::kAudioUnitErr_InvalidPropertyValue;
            }
            state.bypassed = unsafe { *(in_data as *const u32) } != 0;
            state.notify_property_changed(in_id, in_scope, in_element);
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
    in_scope: au_sys::AudioUnitScope,
    _in_element: au_sys::AudioUnitElement,
    out_value: *mut au_sys::AudioUnitParameterValue,
) -> au_sys::OSStatus {
    if out_value.is_null() {
        return au_sys::kAudioUnitErr_InvalidParameter;
    }
    // This plugin's parameters only exist in Global scope (see
    // `au_get_property_info`'s `ParameterList`/`ParameterInfo` handling);
    // treating every other scope as also having parameter 1 (an earlier
    // version of this code did) let `auval` set/get the "same" parameter
    // through what it believed were independent per-scope instances,
    // sharing one underlying gateway entry and looking like state
    // corruption from its side.
    if in_scope != au_sys::kAudioUnitScope_Global {
        return au_sys::kAudioUnitErr_InvalidScope;
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
    in_scope: au_sys::AudioUnitScope,
    _in_element: au_sys::AudioUnitElement,
    in_value: au_sys::AudioUnitParameterValue,
    _in_buffer_offset_in_frames: au_sys::UInt32,
) -> au_sys::OSStatus {
    if in_scope != au_sys::kAudioUnitScope_Global {
        return au_sys::kAudioUnitErr_InvalidScope;
    }
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
    in_output_bus_number: au_sys::UInt32,
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
    // Reject rather than silently clamp: `auval`'s "Bad Max Frames" test
    // specifically calls `Render` with more frames than the negotiated
    // `MaximumFramesPerSlice` and expects an error back -- clamping (an
    // earlier version of this code did `.min(state.max_frames)` here)
    // returned `noErr` for a request the unit never agreed to be able to
    // handle, silently dropping the excess frames instead.
    if in_number_frames as usize > state.max_frames as usize {
        return au_sys::kAudioUnitErr_TooManyFramesToProcess;
    }
    let frames = in_number_frames as usize;
    let base_flags = if io_action_flags.is_null() {
        0
    } else {
        unsafe { *io_action_flags }
    };
    state.call_render_notifies(
        base_flags | au_sys::kAudioUnitRenderAction_PreRender,
        in_time_stamp,
        in_output_bus_number,
        in_number_frames,
        io_data,
    );

    // Pull input via the "pull model": unlike a generator, an Effect AU's
    // `Render` is responsible for fetching its own input, rather than
    // receiving it directly. Two ways a host can wire that up: a direct
    // `kAudioUnitProperty_MakeConnection` to another Audio Unit (pulled by
    // calling the real `AudioUnitRender` on it), or a render callback via
    // `kAudioUnitProperty_SetRenderCallback`. Falls back to silence if
    // neither is set (or the pull fails), so the unit still produces
    // (processed-silence) output instead of erroring out.
    if state.input_channels > 0 {
        if let Some(connection) = state.connection {
            state.input_buffers.prepare(frames);
            let list_ptr = state.input_buffers.as_mut_ptr();
            let status = unsafe {
                AudioUnitRender(
                    connection.source_audio_unit,
                    io_action_flags,
                    in_time_stamp,
                    connection.source_output_number,
                    in_number_frames,
                    list_ptr,
                )
            };
            if status != au_sys::noErr {
                state.input_buffers.silence(frames);
            }
        } else {
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
    }

    apply_pending_parameters(&state.gateway, &mut *state.processor);
    if let Some(queue) = state.midi_queue.as_ref() {
        apply_pending_midi(queue, &mut *state.processor);
    }

    // The host allocates `io_data` with one `AudioBuffer` per output
    // channel (matching `kAudioUnitProperty_StreamFormat`'s output scope),
    // each normally with at least `frames` `Float32` samples of capacity --
    // but a real host (confirmed via `auval`'s slicing render test) can
    // legitimately pass a buffer with `mData == NULL`, expecting the unit
    // to supply its own storage (the "should allocate" contract every
    // well-behaved AU needs to honor; we always support it, rather than
    // declaring `kAudioUnitProperty_ShouldAllocateBuffer` false). Filling
    // those in from our own scratch before building the output slices
    // avoids dereferencing a null/stale pointer.
    let out_buffers = unsafe { (*io_data).buffers_mut() };
    for (buffer, scratch) in out_buffers.iter_mut().zip(state.output_scratch.iter_mut()) {
        if buffer.mData.is_null() {
            buffer.mData = scratch.as_mut_ptr() as *mut c_void;
            buffer.mDataByteSize = (frames * mem::size_of::<f32>()) as u32;
        }
    }
    // SAFETY: every buffer's `mData` is now non-null (either host-provided
    // or just filled in above from `output_scratch`, which is sized to
    // `max_frames >= frames`).
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

    if state.bypassed {
        // Passing straight through (rather than still calling `process`
        // and discarding the result) is what `kAudioUnitProperty_BypassEffect`
        // is for: hosts use it to get a guaranteed-transparent path, which
        // for some processors (e.g. ones with internal state/latency)
        // differs from what running the real DSP at unity would produce.
        for (input, output) in inputs.iter().zip(outputs.iter_mut()) {
            let len = input.len().min(output.len());
            output[..len].copy_from_slice(&input[..len]);
        }
    } else {
        state.processor.process(&inputs, &mut outputs);
    }

    let peak = outputs
        .iter()
        .flat_map(|channel| channel.iter())
        .fold(0.0_f32, |max, &sample| max.max(sample.abs()));
    state.meter.write(peak);

    state.call_render_notifies(
        base_flags | au_sys::kAudioUnitRenderAction_PostRender,
        in_time_stamp,
        in_output_bus_number,
        in_number_frames,
        io_data,
    );

    au_sys::noErr
}

/// `au-sys` doesn't define a `Proc` type for the old-style (no user-data)
/// remove-listener selector, only the newer with-user-data variant; this
/// matches its C signature directly.
type AudioUnitRemovePropertyListenerProc = unsafe extern "C" fn(
    self_ptr: *mut c_void,
    in_id: au_sys::AudioUnitPropertyID,
    in_proc: au_sys::AudioUnitPropertyListenerProc,
) -> au_sys::OSStatus;

/// Registered listeners are actually tracked (not just accepted) so that
/// `notify_property_changed` (called from `au_set_property`) can really
/// fire them -- `auval`'s recommended-properties check registers a
/// listener, changes `MaximumFramesPerSlice`, and specifically verifies
/// the listener was called; an earlier version of this code accepted
/// registrations without storing them, which passed `AddPropertyListener`
/// itself but then failed that follow-up check.
unsafe extern "C" fn au_add_property_listener(
    self_ptr: *mut c_void,
    in_id: au_sys::AudioUnitPropertyID,
    in_proc: au_sys::AudioUnitPropertyListenerProc,
    in_proc_user_data: *mut c_void,
) -> au_sys::OSStatus {
    let state = unsafe { &mut (*(self_ptr as *mut AuInstance)).state };
    state.property_listeners.push(PropertyListener {
        property_id: in_id,
        proc: in_proc,
        user_data: in_proc_user_data,
    });
    au_sys::noErr
}

unsafe extern "C" fn au_remove_property_listener(
    self_ptr: *mut c_void,
    in_id: au_sys::AudioUnitPropertyID,
    in_proc: au_sys::AudioUnitPropertyListenerProc,
) -> au_sys::OSStatus {
    let state = unsafe { &mut (*(self_ptr as *mut AuInstance)).state };
    state
        .property_listeners
        .retain(|l| !(l.property_id == in_id && l.proc as usize == in_proc as usize));
    au_sys::noErr
}

unsafe extern "C" fn au_remove_property_listener_with_user_data(
    self_ptr: *mut c_void,
    in_id: au_sys::AudioUnitPropertyID,
    in_proc: au_sys::AudioUnitPropertyListenerProc,
    in_proc_user_data: *mut c_void,
) -> au_sys::OSStatus {
    let state = unsafe { &mut (*(self_ptr as *mut AuInstance)).state };
    state.property_listeners.retain(|l| {
        !(l.property_id == in_id
            && l.proc as usize == in_proc as usize
            && l.user_data == in_proc_user_data)
    });
    au_sys::noErr
}

/// `au-sys` doesn't define `Proc` types for the render-notify selectors;
/// both share `AURenderCallback`'s shape plus the storage pointer, per
/// `AUComponent.h`.
type AudioUnitAddRenderNotifyProc = unsafe extern "C" fn(
    self_ptr: *mut c_void,
    in_proc: au_sys::AURenderCallback,
    in_proc_user_data: *mut c_void,
) -> au_sys::OSStatus;

type AudioUnitRemoveRenderNotifyProc = unsafe extern "C" fn(
    self_ptr: *mut c_void,
    in_proc: au_sys::AURenderCallback,
    in_proc_user_data: *mut c_void,
) -> au_sys::OSStatus;

/// Tracked for real (like the property listeners above), so
/// `call_render_notifies` (called from `au_render`) can actually invoke
/// them -- confirmed necessary via `auval`'s parameter-scheduling checks,
/// which register a render notify and (per the same "must not just report
/// unimplemented" lesson as `AddPropertyListener`) require it to succeed.
unsafe extern "C" fn au_add_render_notify(
    self_ptr: *mut c_void,
    in_proc: au_sys::AURenderCallback,
    in_proc_user_data: *mut c_void,
) -> au_sys::OSStatus {
    let state = unsafe { &mut (*(self_ptr as *mut AuInstance)).state };
    state.render_notifies.push(RenderNotify {
        proc: in_proc,
        user_data: in_proc_user_data,
    });
    au_sys::noErr
}

unsafe extern "C" fn au_remove_render_notify(
    self_ptr: *mut c_void,
    in_proc: au_sys::AURenderCallback,
    in_proc_user_data: *mut c_void,
) -> au_sys::OSStatus {
    let state = unsafe { &mut (*(self_ptr as *mut AuInstance)).state };
    state
        .render_notifies
        .retain(|n| !(n.proc as usize == in_proc as usize && n.user_data == in_proc_user_data));
    au_sys::noErr
}

type AudioUnitScheduleParametersProc = unsafe extern "C" fn(
    self_ptr: *mut c_void,
    events: *const AudioUnitParameterEvent,
    num_events: au_sys::UInt32,
) -> au_sys::OSStatus;

/// Sample-accurate scheduling (applying a ramped/immediate change at a
/// specific offset *within* a render block) isn't implemented -- this
/// real-time-safe minimal reference plugin applies parameter changes once
/// per block (see `apply_pending_parameters`), not per-sample, so there's
/// nowhere to slice `Render` at an event's offset. Instead, each event's
/// *eventual* value (a ramp's end value, or an immediate event's value) is
/// queued through the gateway as usual, taking effect at the start of the
/// next `Render` call rather than mid-block. `auval` only checks that this
/// call succeeds, not sample-accurate timing.
unsafe extern "C" fn au_schedule_parameters(
    self_ptr: *mut c_void,
    events: *const AudioUnitParameterEvent,
    num_events: au_sys::UInt32,
) -> au_sys::OSStatus {
    if events.is_null() {
        return au_sys::kAudioUnitErr_InvalidParameter;
    }
    let state = unsafe { &mut (*(self_ptr as *mut AuInstance)).state };
    let events = unsafe { std::slice::from_raw_parts(events, num_events as usize) };
    for event in events {
        if event.scope != au_sys::kAudioUnitScope_Global {
            continue;
        }
        let value = unsafe {
            if event.event_type == K_PARAMETER_EVENT_RAMPED {
                event.event_values.ramp.end_value
            } else {
                event.event_values.immediate.value
            }
        };
        state.gateway.set_normalized(
            ParameterId(event.parameter),
            NormalizedValue::new(value as f64),
        );
    }
    au_sys::noErr
}

/// Not in `au-sys` (no Music Device/Music Effect bindings exist there at
/// all yet): the real dispatch selector for MIDI event delivery, from the
/// real `MusicDevice.h` header (`kMusicDeviceRange = 0x0100`,
/// `kMusicDeviceMIDIEventSelect = kMusicDeviceRange | 0x01`), confirmed
/// directly against the macOS SDK rather than guessed, matching this
/// file's existing convention for hand-declared selectors/structs (see
/// `AudioUnitParameterEvent`/`AudioUnitConnection` above).
const K_MUSIC_DEVICE_MIDI_EVENT_SELECT: au_sys::SInt16 = 0x0101;

/// `MusicDeviceMIDIEventProc`'s real C signature (`MusicDevice.h`): a plain
/// 3-byte MIDI 1.0 message (`inStatus`/`inData1`/`inData2`) plus a sample
/// offset within the *next* `Render` call this event should take effect
/// at. This crate applies every queued message at the very start of the
/// next `Render` instead (see `au_render`), the same block-granular
/// simplification `au_schedule_parameters` already makes for automation,
/// so `in_offset_sample_frame` is accepted but not used.
type MusicDeviceMIDIEventProc = unsafe extern "C" fn(
    self_ptr: *mut c_void,
    in_status: au_sys::UInt32,
    in_data1: au_sys::UInt32,
    in_data2: au_sys::UInt32,
    in_offset_sample_frame: au_sys::UInt32,
) -> au_sys::OSStatus;

unsafe extern "C" fn au_music_device_midi_event(
    self_ptr: *mut c_void,
    in_status: au_sys::UInt32,
    in_data1: au_sys::UInt32,
    in_data2: au_sys::UInt32,
    _in_offset_sample_frame: au_sys::UInt32,
) -> au_sys::OSStatus {
    let state = unsafe { &mut (*(self_ptr as *mut AuInstance)).state };
    let Some(queue) = state.midi_queue.as_ref() else {
        // Only reachable if a host calls this without having checked
        // `accepts_midi` some other way first; not an error worth failing
        // loudly for, just nothing to queue.
        return au_sys::noErr;
    };
    if let Some(message) = MidiMessage::from_bytes(in_status as u8, in_data1 as u8, in_data2 as u8)
    {
        queue.push(message);
    }
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
            au_sys::kAudioUnitAddPropertyListenerSelect => {
                Some(mem::transmute::<
                    au_sys::AudioUnitAddPropertyListenerProc,
                    au_sys::AudioComponentMethod,
                >(au_add_property_listener))
            }
            au_sys::kAudioUnitRemovePropertyListenerSelect => {
                Some(mem::transmute::<
                    AudioUnitRemovePropertyListenerProc,
                    au_sys::AudioComponentMethod,
                >(au_remove_property_listener))
            }
            au_sys::kAudioUnitRemovePropertyListenerWithUserDataSelect => {
                Some(mem::transmute::<
                    au_sys::AudioUnitRemovePropertyListenerWithUserDataProc,
                    au_sys::AudioComponentMethod,
                >(au_remove_property_listener_with_user_data))
            }
            au_sys::kAudioUnitAddRenderNotifySelect => Some(mem::transmute::<
                AudioUnitAddRenderNotifyProc,
                au_sys::AudioComponentMethod,
            >(au_add_render_notify)),
            au_sys::kAudioUnitRemoveRenderNotifySelect => {
                Some(mem::transmute::<
                    AudioUnitRemoveRenderNotifyProc,
                    au_sys::AudioComponentMethod,
                >(au_remove_render_notify))
            }
            au_sys::kAudioUnitScheduleParametersSelect => {
                Some(mem::transmute::<
                    AudioUnitScheduleParametersProc,
                    au_sys::AudioComponentMethod,
                >(au_schedule_parameters))
            }
            K_MUSIC_DEVICE_MIDI_EVENT_SELECT => Some(mem::transmute::<
                MusicDeviceMIDIEventProc,
                au_sys::AudioComponentMethod,
            >(au_music_device_midi_event)),
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
    let midi_queue = processor.accepts_midi().then(MidiEventQueue::default);
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
        bypassed: false,
        input_render_callback: None,
        connection: None,
        input_buffers: InputBufferList::new(
            layout.input_channels.max(1) as usize,
            DEFAULT_MAX_FRAMES as usize,
        ),
        output_scratch: vec![
            vec![0.0_f32; DEFAULT_MAX_FRAMES as usize];
            layout.output_channels.max(1) as usize
        ],
        property_listeners: Vec::new(),
        render_notifies: Vec::new(),
        midi_queue,
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
/// created once before (this crate, like `mkapk-vst3`, only supports
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
    use mkapk_host::{ChannelLayout, PluginEditor, SizeConstraints};

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
            _parent: mkapk_host::ParentWindowHandle,
            _host: &dyn mkapk_host::EditorHost,
        ) {
        }
        fn close(&mut self) {}
        fn resize(&mut self, _size: mkapk_core::Sizef) {}
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
            let add_property_listener: au_sys::AudioUnitAddPropertyListenerProc =
                mem::transmute(lookup(au_sys::kAudioUnitAddPropertyListenerSelect).unwrap());
            unsafe extern "C" fn dummy_listener(
                _ref_con: *mut c_void,
                _unit: au_sys::AudioUnit,
                _id: au_sys::AudioUnitPropertyID,
                _scope: au_sys::AudioUnitScope,
                _element: au_sys::AudioUnitElement,
            ) {
            }
            assert_eq!(
                add_property_listener(
                    self_ptr,
                    au_sys::kAudioUnitProperty_MaximumFramesPerSlice,
                    dummy_listener,
                    std::ptr::null_mut(),
                ),
                au_sys::noErr
            );

            let add_render_notify: AudioUnitAddRenderNotifyProc =
                mem::transmute(lookup(au_sys::kAudioUnitAddRenderNotifySelect).unwrap());
            unsafe extern "C" fn counting_render_notify(
                ref_con: *mut c_void,
                _io_action_flags: *mut au_sys::AudioUnitRenderActionFlags,
                _in_time_stamp: *const au_sys::AudioTimeStamp,
                _in_bus_number: u32,
                _in_number_frames: u32,
                _io_data: *mut au_sys::AudioBufferList,
            ) -> au_sys::OSStatus {
                unsafe { *(ref_con as *mut u32) += 1 };
                au_sys::noErr
            }
            let mut notify_count: u32 = 0;
            assert_eq!(
                add_render_notify(
                    self_ptr,
                    counting_render_notify,
                    &mut notify_count as *mut u32 as *mut c_void,
                ),
                au_sys::noErr
            );

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
            // One PreRender + one PostRender notification per `Render` call.
            assert_eq!(notify_count, 2);

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
