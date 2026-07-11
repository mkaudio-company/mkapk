//! The plugin's audio processor. This is the ONE place DSP logic lives —
//! every build target (Standalone/VST3/AU/AAX) shares this same
//! implementation via the `gui_host::Processor` trait.
use gui_host::{
    ChannelLayout, MidiMessage, NormalizedValue, ParameterId, ParameterInfo, Processor,
};

pub const GAIN_PARAM: ParameterId = ParameterId(1);

/// MIDI CC 7 is the standard "Channel Volume" controller -- the natural
/// automation source for a gain parameter, and every format this workspace
/// targets that wires up real MIDI (see each format's `component`/`view`
/// module) delivers it here via `Processor::handle_midi`.
pub const GAIN_MIDI_CC: u8 = 7;

pub struct GainProcessor {
    gain: NormalizedValue,
    parameters: [ParameterInfo; 1],
}

impl GainProcessor {
    pub fn new() -> Self {
        Self {
            gain: NormalizedValue::new(1.0),
            parameters: [ParameterInfo {
                id: GAIN_PARAM,
                name: "Gain",
                default_value: NormalizedValue::new(1.0),
                min_value: NormalizedValue::new(0.0),
                max_value: NormalizedValue::new(1.0),
                step_count: None,
            }],
        }
    }
}

impl Default for GainProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for GainProcessor {
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
        &self.parameters
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

    fn accepts_midi(&self) -> bool {
        true
    }

    /// Demonstrates MIDI automation: CC 7 (Channel Volume) drives the same
    /// `gain` parameter a host's own automation lane or this plugin's UI
    /// would. Every other channel-voice message is ignored -- `GainProcessor`
    /// is an effect (`plugin_kind` stays the default `Effect`), not an
    /// instrument, so notes/pitch-bend/etc. have nothing to act on here.
    fn handle_midi(&mut self, message: MidiMessage) {
        if let MidiMessage::ControlChange {
            controller: GAIN_MIDI_CC,
            value,
            ..
        } = message
        {
            self.gain = NormalizedValue::new(f64::from(value) / 127.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_scales_by_gain_parameter() {
        let mut processor = GainProcessor::new();
        processor.set_parameter(GAIN_PARAM, NormalizedValue::new(0.5));

        let input = [1.0_f32, -1.0, 0.5];
        let mut output = [0.0_f32; 3];
        {
            let inputs: [&[f32]; 1] = [&input];
            let mut outputs: [&mut [f32]; 1] = [&mut output];
            processor.process(&inputs, &mut outputs);
        }
        assert_eq!(output, [0.5, -0.5, 0.25]);
    }

    #[test]
    fn default_gain_is_unity() {
        let processor = GainProcessor::new();
        assert_eq!(
            processor.parameter_value(GAIN_PARAM),
            NormalizedValue::new(1.0)
        );
    }

    #[test]
    fn midi_cc7_automates_gain() {
        let mut processor = GainProcessor::new();
        assert!(processor.accepts_midi());

        processor.handle_midi(MidiMessage::ControlChange {
            channel: 0,
            controller: GAIN_MIDI_CC,
            value: 64,
        });
        assert_eq!(
            processor.parameter_value(GAIN_PARAM),
            NormalizedValue::new(64.0 / 127.0)
        );
    }

    #[test]
    fn other_midi_messages_do_not_change_gain() {
        let mut processor = GainProcessor::new();
        processor.handle_midi(MidiMessage::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });
        processor.handle_midi(MidiMessage::ControlChange {
            channel: 0,
            controller: 1,
            value: 127,
        });
        assert_eq!(
            processor.parameter_value(GAIN_PARAM),
            NormalizedValue::new(1.0)
        );
    }
}
