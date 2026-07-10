use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// A lock-free, single-scalar level meter shared between an audio-thread
/// writer and a UI-thread reader, mirroring `LockFreeParameterGateway` but
/// for the opposite direction (audio -> UI instead of UI -> audio).
///
/// Not sample-accurate: `Ordering::Relaxed` is enough since this only ever
/// drives a visual meter, never audio-affecting logic.
#[derive(Clone)]
pub struct PeakMeter {
    bits: Arc<AtomicU32>,
}

impl PeakMeter {
    pub fn new() -> Self {
        Self {
            bits: Arc::new(AtomicU32::new(0.0_f32.to_bits())),
        }
    }

    /// Called from the real-time callback with the peak absolute sample
    /// value of the block just processed.
    pub fn write(&self, value: f32) {
        self.bits.store(value.to_bits(), Ordering::Relaxed);
    }

    /// Called from the UI thread; returns the most recently written value.
    pub fn read(&self) -> f32 {
        f32::from_bits(self.bits.load(Ordering::Relaxed))
    }
}

impl Default for PeakMeter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_then_read_round_trips() {
        let meter = PeakMeter::new();
        assert_eq!(meter.read(), 0.0);
        meter.write(0.42);
        assert_eq!(meter.read(), 0.42);
    }

    #[test]
    fn clones_share_the_same_underlying_value() {
        let meter = PeakMeter::new();
        let reader = meter.clone();
        meter.write(0.75);
        assert_eq!(reader.read(), 0.75);
    }
}
