use crate::parameter::{
    LockFreeParameterGateway, NormalizedValue, ParameterId, ParameterInfo, ParameterMessage,
};

/// Fixed input/output channel counts a `Processor` expects to be configured
/// with. Real hosts negotiate this before `Processor::prepare` is called.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ChannelLayout {
    pub input_channels: u32,
    pub output_channels: u32,
}

/// Real-time audio processing contract. Implementations must not allocate,
/// lock, or block inside `process`.
pub trait Processor: Send {
    /// Called once before playback starts (or when sample rate/block size
    /// changes), never from the real-time thread while `process` may run
    /// concurrently.
    fn prepare(&mut self, sample_rate: f64, max_block_size: usize);

    /// Processes one block. `inputs`/`outputs` are per-channel sample
    /// slices, each at most `max_block_size` samples (as given to
    /// `prepare`), and may be shorter for a partial final block.
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]]);

    /// Clears any internal state (filter memory, envelopes, etc.) without
    /// requiring a full `prepare` call.
    fn reset(&mut self);

    fn channel_layout(&self) -> ChannelLayout;

    fn parameters(&self) -> &[ParameterInfo];

    fn set_parameter(&mut self, id: ParameterId, value: NormalizedValue);

    fn parameter_value(&self, id: ParameterId) -> NormalizedValue;
}

/// Drains parameter changes queued from the UI thread and applies them to
/// `processor`. Every format's real-time callback should call this once per
/// block, before `Processor::process`. Draining is a lock-free channel
/// receive (see `LockFreeParameterGateway::poll_audio_changes`); this
/// function itself does not allocate.
pub fn apply_pending_parameters(gateway: &LockFreeParameterGateway, processor: &mut dyn Processor) {
    gateway.poll_audio_changes(|msg| {
        if let ParameterMessage::SetNormalized(id, value) = msg {
            processor.set_parameter(id, value);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parameter::ParameterGateway;

    struct TestGainProcessor {
        gain: NormalizedValue,
        info: [ParameterInfo; 1],
    }

    impl TestGainProcessor {
        fn new() -> Self {
            Self {
                gain: NormalizedValue::new(1.0),
                info: [ParameterInfo {
                    id: ParameterId(1),
                    name: "Gain",
                    default_value: NormalizedValue::new(1.0),
                    min_value: NormalizedValue::new(0.0),
                    max_value: NormalizedValue::new(1.0),
                    step_count: None,
                }],
            }
        }
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
            &self.info
        }

        fn set_parameter(&mut self, id: ParameterId, value: NormalizedValue) {
            if id == ParameterId(1) {
                self.gain = value;
            }
        }

        fn parameter_value(&self, id: ParameterId) -> NormalizedValue {
            if id == ParameterId(1) {
                self.gain
            } else {
                NormalizedValue::new(0.0)
            }
        }
    }

    #[test]
    fn process_scales_input_by_gain() {
        let mut processor = TestGainProcessor::new();
        processor.set_parameter(ParameterId(1), NormalizedValue::new(0.5));

        let input = [0.2_f32, 0.4, 1.0];
        let mut output = [0.0_f32; 3];
        {
            let inputs: [&[f32]; 1] = [&input];
            let mut outputs: [&mut [f32]; 1] = [&mut output];
            processor.process(&inputs, &mut outputs);
        }

        assert_eq!(output, [0.1, 0.2, 0.5]);
    }

    #[test]
    fn apply_pending_parameters_drains_gateway_and_updates_processor() {
        let gateway = LockFreeParameterGateway::new(8);
        let mut processor = TestGainProcessor::new();

        gateway.set_normalized(ParameterId(1), NormalizedValue::new(0.25));
        apply_pending_parameters(&gateway, &mut processor);

        assert_eq!(
            processor.parameter_value(ParameterId(1)),
            NormalizedValue::new(0.25)
        );
    }

    #[test]
    fn apply_pending_parameters_ignores_gesture_messages() {
        let gateway = LockFreeParameterGateway::new(8);
        let mut processor = TestGainProcessor::new();

        gateway.begin_gesture(ParameterId(1));
        gateway.end_gesture(ParameterId(1));
        // Should not panic and should leave the default gain untouched.
        apply_pending_parameters(&gateway, &mut processor);

        assert_eq!(
            processor.parameter_value(ParameterId(1)),
            NormalizedValue::new(1.0)
        );
    }
}
