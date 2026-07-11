//! Standalone desktop host: runs a `Processor` against real audio input and
//! output devices (via `cpal`) alongside a real window running a
//! `PluginEditor`, connected through the existing
//! `LockFreeParameterGateway`.
//!
//! Real capture and playback are two independently-scheduled `cpal`
//! streams; a lock-free ring buffer (`ringbuf`) bridges captured samples
//! from the input stream's callback to the output stream's callback
//! without blocking either audio thread. If no input device is selected
//! (or the selected one can't be opened), a fixed, quiet sine test tone is
//! fed in as "input" instead, so the processing pipeline stays audible
//! end-to-end even without a capture device.
#![deny(unsafe_code)]
#![allow(unexpected_cfgs, deprecated)]

use std::sync::Arc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use mkapk_core::Sizef;
use mkapk_host::{
    EditorHost, LockFreeParameterGateway, MidiEventQueue, MidiMessage, NormalizedValue,
    ParameterGateway, ParameterId, PeakMeter, PluginEditor, Processor, apply_pending_midi,
    apply_pending_parameters,
};
use ringbuf::HeapRb;
use ringbuf::traits::{Consumer, Producer, Split};

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

#[cfg(target_os = "macos")]
mod device_picker;
#[cfg(target_os = "macos")]
mod main_window;

/// Standalone window configuration.
pub struct StandaloneConfig {
    pub width: u32,
    pub height: u32,
}

impl Default for StandaloneConfig {
    fn default() -> Self {
        Self {
            width: 400,
            height: 300,
        }
    }
}

struct StandaloneEditorHost {
    gateway: Arc<LockFreeParameterGateway>,
}

impl EditorHost for StandaloneEditorHost {
    fn request_resize(&self, _size: Sizef) {}

    fn start_parameter_gesture(&self, id: ParameterId) {
        self.gateway.begin_gesture(id);
    }

    fn end_parameter_gesture(&self, id: ParameterId) {
        self.gateway.end_gesture(id);
    }

    fn set_parameter_normalized(&self, id: ParameterId, value: NormalizedValue) {
        self.gateway.set_normalized(id, value);
    }
}

/// Runs `processor` against real (user-selected, on macOS) audio input and
/// output devices and `editor` in a real window, blocking until the window
/// is closed.
///
/// `processor` and `editor` should share the same `Arc<LockFreeParameterGateway>`
/// (construct one, clone it into the processor-driving code and pass the
/// other clone into the editor) so UI parameter changes reach the audio
/// thread and vice versa; `meter` should likewise be the same `PeakMeter`
/// given to the editor, so the peak level this function measures from the
/// real output stream reaches whatever level meter the UI draws; see
/// `plugins/gain` for a full example.
pub fn run<P, E>(
    processor: P,
    gateway: Arc<LockFreeParameterGateway>,
    meter: PeakMeter,
    mut editor: E,
    config: StandaloneConfig,
) where
    P: Processor + Send + 'static,
    E: PluginEditor + 'static,
{
    #[cfg(target_os = "macos")]
    let (output_device, input_device) = match device_picker::pick_devices() {
        Some(devices) => (Some(devices.output), devices.input),
        None => (None, None),
    };
    // No picker UI on this platform yet, so fall back to the host's own
    // defaults directly (unlike macOS, there's no way to express "the user
    // deliberately chose no input" here).
    #[cfg(not(target_os = "macos"))]
    let (output_device, input_device): (Option<cpal::Device>, Option<cpal::Device>) = {
        let host = cpal::default_host();
        (None, host.default_input_device())
    };

    let _streams = build_audio_stream(
        processor,
        gateway.clone(),
        meter,
        output_device,
        input_device,
    );
    if _streams.is_none() {
        eprintln!(
            "mkapk-standalone: no usable audio output device found in this environment; running UI only"
        );
    }

    // The main window's chrome (the native NSWindow) comes from mkgraphic on
    // macOS; the plugin's own content view inside it is created and driven
    // by this project's rendering pipeline (see `main_window`). mkgraphic's
    // cross-platform Window/App facade doesn't cover Windows yet, so that
    // platform still uses mkapk-test-host's own window creation.
    #[cfg(target_os = "macos")]
    let (window, parent_handle) =
        main_window::MainWindow::create("Gain", config.width, config.height);
    #[cfg(not(target_os = "macos"))]
    let (window, parent_handle) = mkapk_test_host::create_host_window(config.width, config.height);

    let host = StandaloneEditorHost { gateway };
    editor.open(parent_handle, &host);
    editor.resize(Sizef::new(config.width as f32, config.height as f32));

    // On macOS, the editor wires its own view's mouse handling directly
    // (see `mkapk_mac::paint_view`) as part of `open()`. `mkapk-test-host`'s
    // Win32 window, by contrast, owns the only `wndproc` we get a chance to
    // hook on Windows, so it needs the sink installed from here instead;
    // `mkapk_test_host::attach_mouse_input` owns the unsafe raw-pointer
    // capture this requires (this crate forbids unsafe code itself).
    #[cfg(not(target_os = "macos"))]
    mkapk_test_host::attach_mouse_input(&window, &mut editor);

    while window.pump_events() {
        editor.idle();
        std::thread::sleep(Duration::from_millis(16));
    }

    editor.close();
    window.destroy();
}

/// Maximum frames processed per `Processor::process` call. cpal callback
/// buffer sizes can vary by host/device; the reusable scratch buffers below
/// are sized to this and larger callback buffers are simply capped (the
/// remainder is zero-filled) rather than growing allocations at run time.
const MAX_BLOCK_SIZE: usize = 4096;

/// Both `cpal` streams plus the real MIDI input connection (if any) behind
/// [`build_audio_stream`]. Kept alive together by whoever calls
/// `build_audio_stream`: dropping any of them stops it.
#[allow(dead_code)]
struct AudioStreams {
    output: cpal::Stream,
    input: Option<cpal::Stream>,
    midi_input: Option<midir::MidiInputConnection<()>>,
}

/// Opens the first available real MIDI input port (no picker UI yet, same
/// as this crate's non-macOS audio-input fallback) and forwards every
/// channel-voice message it receives into `queue`. Returns `None` if no
/// MIDI input exists in this environment -- not a hard requirement, since a
/// processor that accepts MIDI should still run (just never receive any)
/// when there's no MIDI hardware/virtual port to read from.
fn open_midi_input(queue: Arc<MidiEventQueue>) -> Option<midir::MidiInputConnection<()>> {
    let input = midir::MidiInput::new("mkapk-standalone").ok()?;
    let ports = input.ports();
    let port = ports.first()?;
    input
        .connect(
            port,
            "mkapk-standalone-input",
            move |_timestamp_micros, bytes, ()| {
                let status = *bytes.first().unwrap_or(&0);
                let data1 = bytes.get(1).copied().unwrap_or(0);
                let data2 = bytes.get(2).copied().unwrap_or(0);
                if let Some(message) = MidiMessage::from_bytes(status, data1, data2) {
                    queue.push(message);
                }
            },
            (),
        )
        .ok()
}

/// Number of `MAX_BLOCK_SIZE`-sized blocks of slack the input->output ring
/// buffer holds, absorbing scheduling jitter between the two independently
/// scheduled `cpal` callback threads without ever blocking either of them.
const RING_BUFFER_BLOCKS: usize = 8;

fn build_audio_stream<P>(
    mut processor: P,
    gateway: Arc<LockFreeParameterGateway>,
    meter: PeakMeter,
    output_device: Option<cpal::Device>,
    input_device: Option<cpal::Device>,
) -> Option<AudioStreams>
where
    P: Processor + Send + 'static,
{
    let host = cpal::default_host();
    let output_device = output_device.or_else(|| host.default_output_device())?;
    let supported_output_config = output_device.default_output_config().ok()?;
    if supported_output_config.sample_format() != cpal::SampleFormat::F32 {
        // MVP: only the F32 sample format is handled.
        return None;
    }
    let output_stream_config: cpal::StreamConfig = supported_output_config.into();

    let sample_rate = output_stream_config.sample_rate.0 as f64;
    let output_channels = output_stream_config.channels as usize;
    processor.prepare(sample_rate, MAX_BLOCK_SIZE);

    let wanted_input_channels = processor.channel_layout().input_channels as usize;

    // Only open a MIDI input port at all when the processor actually wants
    // MIDI -- an effect with no use for it shouldn't pay for a MIDI input
    // thread or claim a system MIDI port it never reads.
    let midi_queue = processor
        .accepts_midi()
        .then(|| Arc::new(MidiEventQueue::default()));
    let midi_input = midi_queue.as_ref().cloned().and_then(open_midi_input);
    if midi_queue.is_some() && midi_input.is_none() {
        eprintln!(
            "mkapk-standalone: this processor accepts MIDI, but no MIDI input port was found in this environment"
        );
    }

    // Try to open a real capture stream matching the output's sample rate.
    // If none was selected, or the selected device can't supply that rate,
    // real capture is skipped entirely and a quiet test tone fills `inputs`
    // instead (see the `else` branch in the output callback below), so the
    // processing pipeline stays audible end-to-end regardless.
    let input_setup = input_device.as_ref().and_then(|device| {
        let supported = device.default_input_config().ok()?;
        if supported.sample_format() != cpal::SampleFormat::F32 {
            return None;
        }
        if supported.sample_rate().0 != output_stream_config.sample_rate.0 {
            eprintln!(
                "mkapk-standalone: input device sample rate ({} Hz) doesn't match the output device's ({} Hz); falling back to a test tone",
                supported.sample_rate().0,
                output_stream_config.sample_rate.0
            );
            return None;
        }
        let channels = supported.channels() as usize;
        let input_stream_config: cpal::StreamConfig = supported.into();

        let rb = HeapRb::<f32>::new(MAX_BLOCK_SIZE * channels.max(1) * RING_BUFFER_BLOCKS);
        let (mut producer, consumer) = rb.split();

        let err_fn = |err| eprintln!("mkapk-standalone: input stream error: {err}");
        let stream = device
            .build_input_stream(
                &input_stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // Dropping overflow (rather than blocking) is the
                    // correct real-time-safe choice here: the ring buffer
                    // is sized generously, so overflow only happens if the
                    // output thread has stalled, in which case losing a
                    // few samples is preferable to stalling capture too.
                    let _ = producer.push_slice(data);
                },
                err_fn,
                None,
            )
            .ok()?;
        stream.play().ok()?;
        Some((stream, consumer, channels))
    });

    let (input_stream, mut input_consumer, captured_input_channels) = match input_setup {
        Some((stream, consumer, channels)) => (Some(stream), Some(consumer), channels),
        None => (None, None, 0),
    };

    let mut input_scratch = vec![vec![0.0_f32; MAX_BLOCK_SIZE]; wanted_input_channels.max(1)];
    let mut interleaved_scratch = vec![0.0_f32; MAX_BLOCK_SIZE * captured_input_channels.max(1)];
    let mut output_scratch = vec![vec![0.0_f32; MAX_BLOCK_SIZE]; output_channels.max(1)];
    let mut tone_phase = 0.0_f32;
    let tone_increment = 220.0 * core::f32::consts::TAU / sample_rate as f32;

    let err_fn = |err| eprintln!("mkapk-standalone: audio stream error: {err}");

    let output_stream = output_device
        .build_output_stream(
            &output_stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                apply_pending_parameters(&gateway, &mut processor);
                if let Some(queue) = midi_queue.as_ref() {
                    apply_pending_midi(queue, &mut processor);
                }

                let channels = output_channels.max(1);
                let frames = (data.len() / channels).min(MAX_BLOCK_SIZE);

                if let Some(consumer) = input_consumer.as_mut() {
                    let wanted_samples = frames * captured_input_channels.max(1);
                    let interleaved = &mut interleaved_scratch[..wanted_samples];
                    let filled = consumer.pop_slice(interleaved);
                    // Zero-fill whatever the ring buffer couldn't supply
                    // yet (e.g. right after the stream starts) instead of
                    // reusing stale samples.
                    interleaved[filled..].fill(0.0);

                    for (ch, channel_scratch) in input_scratch.iter_mut().enumerate() {
                        let source_ch = ch.min(captured_input_channels.saturating_sub(1));
                        for (frame, sample) in channel_scratch.iter_mut().take(frames).enumerate() {
                            *sample = interleaved[frame * captured_input_channels + source_ch];
                        }
                    }
                } else {
                    for sample_slot in input_scratch[0].iter_mut().take(frames) {
                        *sample_slot = tone_phase.sin() * 0.05;
                        tone_phase += tone_increment;
                        if tone_phase > core::f32::consts::TAU {
                            tone_phase -= core::f32::consts::TAU;
                        }
                    }
                    if wanted_input_channels > 1 {
                        let (first, rest) = input_scratch.split_at_mut(1);
                        for other in rest {
                            other[..frames].copy_from_slice(&first[0][..frames]);
                        }
                    }
                }

                {
                    let inputs: Vec<&[f32]> =
                        input_scratch.iter().map(|buf| &buf[..frames]).collect();
                    let mut outputs: Vec<&mut [f32]> = output_scratch
                        .iter_mut()
                        .map(|buf| &mut buf[..frames])
                        .collect();
                    processor.process(&inputs, &mut outputs);
                }

                let peak = output_scratch
                    .iter()
                    .flat_map(|channel| channel[..frames].iter())
                    .fold(0.0_f32, |max, &sample| max.max(sample.abs()));
                meter.write(peak);

                for (frame_idx, frame) in data.chunks_mut(channels).enumerate() {
                    for (ch, sample) in frame.iter_mut().enumerate() {
                        *sample = if frame_idx < frames {
                            output_scratch[ch][frame_idx]
                        } else {
                            0.0
                        };
                    }
                }
            },
            err_fn,
            None,
        )
        .ok()?;

    output_stream.play().ok()?;
    Some(AudioStreams {
        output: output_stream,
        input: input_stream,
        midi_input,
    })
}
