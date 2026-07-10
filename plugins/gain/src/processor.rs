//! The plugin's audio processor. This is the ONE place DSP logic lives —
//! every build target (Standalone/VST3/AU/AAX) shares this same
//! implementation via the `gui_host::Processor` trait.
use gui_host::{ChannelLayout, NormalizedValue, ParameterId, ParameterInfo, Processor};

pub const GAIN_PARAM: ParameterId = ParameterId(1);

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
}
