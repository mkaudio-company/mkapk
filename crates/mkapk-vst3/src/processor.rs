//! Bridge functions the C++ shim's `VstProcessor` (`cpp/VstProcessor.cpp`)
//! calls into via `cpp/VstBridge.h`. `vst3_entry!` wires each of these into
//! a concrete `#[unsafe(no_mangle)] extern "C" fn` for whichever
//! `mkapk_host::Processor` a plugin project supplies; nothing here is
//! specific to one plugin.
use std::os::raw::c_void;

use mkapk_host::{MidiMessage, NormalizedValue, ParameterId, ParameterInfo, Processor};

/// Constructs a transient processor just to answer a bus-layout query --
/// never touches the real, persistent per-instance processor created by
/// [`create`]. Parameter metadata (below) comes from `vst3_entry!`'s own
/// `parameters:` field instead, matching this crate's public macro API.
pub fn input_channels(make_processor: &mut dyn FnMut() -> Box<dyn Processor>) -> u32 {
    make_processor().channel_layout().input_channels
}

pub fn output_channels(make_processor: &mut dyn FnMut() -> Box<dyn Processor>) -> u32 {
    make_processor().channel_layout().output_channels
}

pub fn accepts_midi(make_processor: &mut dyn FnMut() -> Box<dyn Processor>) -> bool {
    make_processor().accepts_midi()
}

pub fn parameter_count(parameters: &[ParameterInfo]) -> i32 {
    parameters.len() as i32
}

pub fn parameter_id(parameters: &[ParameterInfo], index: i32) -> u32 {
    parameters[index as usize].id.0
}

/// Writes up to `out_capacity` UTF-16 code units (including a trailing
/// NUL) of the parameter's display name into `out`; returns the unit count
/// written.
///
/// # Safety
/// `out` must be valid for `out_capacity` writable `u16`s -- upheld by this
/// crate's C++ shim, the only caller.
pub unsafe fn parameter_name(
    parameters: &[ParameterInfo],
    index: i32,
    out: *mut u16,
    out_capacity: i32,
) -> i32 {
    let mut units: Vec<u16> = parameters[index as usize].name.encode_utf16().collect();
    units.truncate((out_capacity as usize).saturating_sub(1));
    units.push(0);
    unsafe {
        std::ptr::copy_nonoverlapping(units.as_ptr(), out, units.len());
    }
    units.len() as i32
}

pub fn parameter_default(parameters: &[ParameterInfo], index: i32) -> f64 {
    parameters[index as usize].default_value.get()
}

pub fn parameter_step_count(parameters: &[ParameterInfo], index: i32) -> i32 {
    parameters[index as usize].step_count.unwrap_or(0) as i32
}

pub fn midi_cc_to_param(midi_cc_map: &[(u8, ParameterId)], midi_cc_number: i32) -> Option<u32> {
    let cc = u8::try_from(midi_cc_number).ok()?;
    midi_cc_map
        .iter()
        .find(|(mapped_cc, _)| *mapped_cc == cc)
        .map(|(_, id)| id.0)
}

/// One boxed `dyn Processor` per real plugin instance, exactly matching a
/// host's one-`IComponent`-per-plugin-instance lifetime -- created and
/// destroyed by the C++ shim's `VstProcessor` constructor/destructor.
pub fn create(make_processor: &mut dyn FnMut() -> Box<dyn Processor>) -> *mut c_void {
    let boxed: Box<Box<dyn Processor>> = Box::new(make_processor());
    Box::into_raw(boxed) as *mut c_void
}

/// # Safety
/// `handle` must be a still-live pointer previously returned by [`create`],
/// not yet destroyed.
pub unsafe fn destroy(handle: *mut c_void) {
    unsafe {
        drop(Box::from_raw(handle as *mut Box<dyn Processor>));
    }
}

/// # Safety
/// `handle` must be a still-live pointer previously returned by [`create`].
unsafe fn with_processor<'a>(handle: *mut c_void) -> &'a mut Box<dyn Processor> {
    unsafe { &mut *(handle as *mut Box<dyn Processor>) }
}

/// # Safety
/// `handle` must be a still-live pointer previously returned by [`create`].
pub unsafe fn prepare(handle: *mut c_void, sample_rate: f64, max_samples_per_block: i32) {
    unsafe {
        with_processor(handle).prepare(sample_rate, max_samples_per_block as usize);
    }
}

/// # Safety
/// `handle` must be a still-live pointer previously returned by [`create`].
pub unsafe fn set_parameter(handle: *mut c_void, param_id: u32, normalized_value: f64) {
    unsafe {
        with_processor(handle).set_parameter(
            ParameterId(param_id),
            NormalizedValue::new(normalized_value),
        );
    }
}

/// # Safety
/// `handle` must be a still-live pointer previously returned by [`create`].
pub unsafe fn get_parameter(handle: *mut c_void, param_id: u32) -> f64 {
    unsafe {
        with_processor(handle)
            .parameter_value(ParameterId(param_id))
            .get()
    }
}

/// # Safety
/// `handle` must be a still-live pointer previously returned by [`create`].
pub unsafe fn handle_midi(handle: *mut c_void, status: u8, data1: u8, data2: u8) {
    let channel = status & 0x0F;
    let message = match status & 0xF0 {
        0x90 if data2 > 0 => Some(MidiMessage::NoteOn {
            channel,
            note: data1,
            velocity: data2,
        }),
        0x90 => Some(MidiMessage::NoteOff {
            channel,
            note: data1,
            velocity: data2,
        }),
        0x80 => Some(MidiMessage::NoteOff {
            channel,
            note: data1,
            velocity: data2,
        }),
        0xA0 => Some(MidiMessage::PolyphonicKeyPressure {
            channel,
            note: data1,
            pressure: data2,
        }),
        _ => None,
    };
    if let Some(message) = message {
        unsafe {
            with_processor(handle).handle_midi(message);
        }
    }
}

/// # Safety
/// `handle` must be a still-live pointer previously returned by [`create`];
/// `inputs`/`outputs` must each be valid for `num_inputs`/`num_outputs`
/// `float*` entries, each of those valid for `num_frames` samples -- upheld
/// by this crate's C++ shim, the only caller.
pub unsafe fn process(
    handle: *mut c_void,
    inputs: *const *const f32,
    num_inputs: i32,
    outputs: *const *mut f32,
    num_outputs: i32,
    num_frames: i32,
) {
    let num_frames = num_frames as usize;
    let input_slices: Vec<&[f32]> = unsafe {
        if num_inputs > 0 {
            std::slice::from_raw_parts(inputs, num_inputs as usize)
                .iter()
                .map(|&p| std::slice::from_raw_parts(p, num_frames))
                .collect()
        } else {
            Vec::new()
        }
    };
    let mut output_slices: Vec<&mut [f32]> = unsafe {
        if num_outputs > 0 {
            std::slice::from_raw_parts(outputs, num_outputs as usize)
                .iter()
                .map(|&p| std::slice::from_raw_parts_mut(p, num_frames))
                .collect()
        } else {
            Vec::new()
        }
    };
    unsafe {
        with_processor(handle).process(&input_slices, &mut output_slices);
    }
}
