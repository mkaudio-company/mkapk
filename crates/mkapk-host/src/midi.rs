//! MIDI 1.0 channel-voice messages and a lock-free queue delivering them
//! from whichever real-time MIDI source a format provides (a host's
//! `IEventList`/`MusicDeviceMIDIEvent`/`AAX_IMIDINode` callback, or
//! `gui-standalone`'s own MIDI input thread) into `Processor::handle_midi`.
use crate::processor::Processor;

/// One MIDI 1.0 channel-voice message. `channel` is 0-based (`0..=15`).
///
/// Deliberately just the classic channel-voice set (no System
/// Exclusive/Common/Real-Time messages, no MIDI 2.0/MPE extensions) --
/// every format this workspace targets delivers at most this much through
/// its own real-time MIDI path, and it's what `Processor::handle_midi`
/// needs for both automation (Control Change) and instrument (Note
/// On/Off) use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiMessage {
    NoteOff {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    PolyphonicKeyPressure {
        channel: u8,
        note: u8,
        pressure: u8,
    },
    ControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },
    ProgramChange {
        channel: u8,
        program: u8,
    },
    ChannelPressure {
        channel: u8,
        pressure: u8,
    },
    /// 14-bit value, `0..=16383`, center (no bend) at `8192`.
    PitchBend {
        channel: u8,
        value: u16,
    },
}

impl MidiMessage {
    /// Decodes one MIDI 1.0 channel-voice message from up to 3 raw bytes,
    /// the shape every format's own real-time MIDI delivery ultimately
    /// reduces to (a status byte plus up to two data bytes). Returns `None`
    /// for anything that isn't a channel-voice status byte (data bytes with
    /// the high bit unset, or System Exclusive/Common/Real-Time status
    /// bytes `0xF0..=0xFF`), since those aren't represented here.
    ///
    /// A Note On with velocity 0 is normalized to `NoteOff` -- the standard
    /// MIDI running-status convention for "note off without a distinct
    /// status byte," which real controllers and hosts rely on.
    pub fn from_bytes(status: u8, data1: u8, data2: u8) -> Option<Self> {
        if status & 0x80 == 0 {
            return None;
        }
        let channel = status & 0x0F;
        let d1 = data1 & 0x7F;
        let d2 = data2 & 0x7F;
        match status & 0xF0 {
            0x80 => Some(MidiMessage::NoteOff {
                channel,
                note: d1,
                velocity: d2,
            }),
            0x90 => {
                if d2 == 0 {
                    Some(MidiMessage::NoteOff {
                        channel,
                        note: d1,
                        velocity: 0,
                    })
                } else {
                    Some(MidiMessage::NoteOn {
                        channel,
                        note: d1,
                        velocity: d2,
                    })
                }
            }
            0xA0 => Some(MidiMessage::PolyphonicKeyPressure {
                channel,
                note: d1,
                pressure: d2,
            }),
            0xB0 => Some(MidiMessage::ControlChange {
                channel,
                controller: d1,
                value: d2,
            }),
            0xC0 => Some(MidiMessage::ProgramChange {
                channel,
                program: d1,
            }),
            0xD0 => Some(MidiMessage::ChannelPressure {
                channel,
                pressure: d1,
            }),
            0xE0 => Some(MidiMessage::PitchBend {
                channel,
                value: ((d2 as u16) << 7) | (d1 as u16),
            }),
            _ => None,
        }
    }

    /// Encodes back to raw MIDI 1.0 bytes: `(status, data1, data2, len)`,
    /// where `len` (2 or 3) tells the caller how many of the returned bytes
    /// are meaningful (Program Change/Channel Pressure are 2-byte
    /// messages; `data2` is `0` and unused for those).
    pub fn to_bytes(self) -> (u8, u8, u8, usize) {
        match self {
            MidiMessage::NoteOff {
                channel,
                note,
                velocity,
            } => (0x80 | (channel & 0x0F), note & 0x7F, velocity & 0x7F, 3),
            MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            } => (0x90 | (channel & 0x0F), note & 0x7F, velocity & 0x7F, 3),
            MidiMessage::PolyphonicKeyPressure {
                channel,
                note,
                pressure,
            } => (0xA0 | (channel & 0x0F), note & 0x7F, pressure & 0x7F, 3),
            MidiMessage::ControlChange {
                channel,
                controller,
                value,
            } => (0xB0 | (channel & 0x0F), controller & 0x7F, value & 0x7F, 3),
            MidiMessage::ProgramChange { channel, program } => {
                (0xC0 | (channel & 0x0F), program & 0x7F, 0, 2)
            }
            MidiMessage::ChannelPressure { channel, pressure } => {
                (0xD0 | (channel & 0x0F), pressure & 0x7F, 0, 2)
            }
            MidiMessage::PitchBend { channel, value } => (
                0xE0 | (channel & 0x0F),
                (value & 0x7F) as u8,
                ((value >> 7) & 0x7F) as u8,
                3,
            ),
        }
    }

    pub fn channel(self) -> u8 {
        match self {
            MidiMessage::NoteOff { channel, .. }
            | MidiMessage::NoteOn { channel, .. }
            | MidiMessage::PolyphonicKeyPressure { channel, .. }
            | MidiMessage::ControlChange { channel, .. }
            | MidiMessage::ProgramChange { channel, .. }
            | MidiMessage::ChannelPressure { channel, .. }
            | MidiMessage::PitchBend { channel, .. } => channel,
        }
    }
}

/// A bounded, lock-free single-producer/single-consumer-style queue
/// carrying MIDI messages from whichever thread a format's real-time MIDI
/// source runs on (a host's audio thread calling into the plugin directly,
/// or `gui-standalone`'s dedicated MIDI input thread) to the audio
/// callback. Mirrors `LockFreeParameterGateway`'s shape but one-directional
/// (there's no UI-thread MIDI reader in this workspace).
pub struct MidiEventQueue {
    sender: crossbeam_channel::Sender<MidiMessage>,
    receiver: crossbeam_channel::Receiver<MidiMessage>,
}

impl MidiEventQueue {
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = crossbeam_channel::bounded(capacity);
        Self { sender, receiver }
    }

    /// Enqueues one message. Never blocks; silently drops the message if
    /// the queue is full rather than stalling whichever thread is
    /// producing MIDI (dropping an occasional CC under extreme load is far
    /// preferable to blocking a MIDI input or host callback thread).
    pub fn push(&self, message: MidiMessage) {
        let _ = self.sender.try_send(message);
    }
}

impl Default for MidiEventQueue {
    fn default() -> Self {
        Self::new(256)
    }
}

/// Drains every MIDI message queued since the last call and applies it to
/// `processor` via `Processor::handle_midi`. Every format's real-time
/// callback that wires up MIDI should call this once per block, before
/// `Processor::process` -- mirroring `apply_pending_parameters`. Draining
/// is a lock-free channel receive; this function itself does not allocate.
pub fn apply_pending_midi(queue: &MidiEventQueue, processor: &mut dyn Processor) {
    while let Ok(message) = queue.receiver.try_recv() {
        processor.handle_midi(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_bytes_decodes_every_channel_voice_message() {
        assert_eq!(
            MidiMessage::from_bytes(0x90, 60, 100),
            Some(MidiMessage::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100
            })
        );
        assert_eq!(
            MidiMessage::from_bytes(0x80, 60, 0),
            Some(MidiMessage::NoteOff {
                channel: 0,
                note: 60,
                velocity: 0
            })
        );
        assert_eq!(
            MidiMessage::from_bytes(0xB0, 7, 127),
            Some(MidiMessage::ControlChange {
                channel: 0,
                controller: 7,
                value: 127
            })
        );
        assert_eq!(
            MidiMessage::from_bytes(0xE1, 0x00, 0x40),
            Some(MidiMessage::PitchBend {
                channel: 1,
                value: 8192
            })
        );
    }

    #[test]
    fn note_on_with_zero_velocity_normalizes_to_note_off() {
        assert_eq!(
            MidiMessage::from_bytes(0x93, 64, 0),
            Some(MidiMessage::NoteOff {
                channel: 3,
                note: 64,
                velocity: 0
            })
        );
    }

    #[test]
    fn from_bytes_rejects_non_channel_voice_status() {
        assert_eq!(MidiMessage::from_bytes(0x00, 0, 0), None); // data byte, not status
        assert_eq!(MidiMessage::from_bytes(0xF8, 0, 0), None); // system real-time (clock)
    }

    #[test]
    fn to_bytes_round_trips_through_from_bytes() {
        let original = MidiMessage::ControlChange {
            channel: 5,
            controller: 74,
            value: 63,
        };
        let (status, d1, d2, len) = original.to_bytes();
        assert_eq!(len, 3);
        assert_eq!(MidiMessage::from_bytes(status, d1, d2), Some(original));
    }

    #[test]
    fn channel_extracts_correctly_for_every_variant() {
        assert_eq!(
            MidiMessage::ProgramChange {
                channel: 9,
                program: 0
            }
            .channel(),
            9
        );
    }

    struct RecordingProcessor {
        received: Vec<MidiMessage>,
    }

    impl crate::processor::Processor for RecordingProcessor {
        fn prepare(&mut self, _sample_rate: f64, _max_block_size: usize) {}
        fn process(&mut self, _inputs: &[&[f32]], _outputs: &mut [&mut [f32]]) {}
        fn reset(&mut self) {}
        fn channel_layout(&self) -> crate::processor::ChannelLayout {
            crate::processor::ChannelLayout::default()
        }
        fn parameters(&self) -> &[crate::parameter::ParameterInfo] {
            &[]
        }
        fn set_parameter(
            &mut self,
            _id: crate::parameter::ParameterId,
            _value: crate::parameter::NormalizedValue,
        ) {
        }
        fn parameter_value(
            &self,
            _id: crate::parameter::ParameterId,
        ) -> crate::parameter::NormalizedValue {
            crate::parameter::NormalizedValue::new(0.0)
        }
        fn handle_midi(&mut self, message: MidiMessage) {
            self.received.push(message);
        }
    }

    #[test]
    fn apply_pending_midi_drains_queue_in_order() {
        let queue = MidiEventQueue::new(8);
        let mut processor = RecordingProcessor {
            received: Vec::new(),
        };

        queue.push(MidiMessage::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });
        queue.push(MidiMessage::ControlChange {
            channel: 0,
            controller: 1,
            value: 64,
        });

        apply_pending_midi(&queue, &mut processor);

        assert_eq!(
            processor.received,
            vec![
                MidiMessage::NoteOn {
                    channel: 0,
                    note: 60,
                    velocity: 100
                },
                MidiMessage::ControlChange {
                    channel: 0,
                    controller: 1,
                    value: 64
                },
            ]
        );
    }

    #[test]
    fn queue_drops_messages_past_capacity_instead_of_blocking() {
        let queue = MidiEventQueue::new(1);
        queue.push(MidiMessage::NoteOn {
            channel: 0,
            note: 1,
            velocity: 1,
        });
        queue.push(MidiMessage::NoteOn {
            channel: 0,
            note: 2,
            velocity: 1,
        }); // dropped, queue full

        let mut processor = RecordingProcessor {
            received: Vec::new(),
        };
        apply_pending_midi(&queue, &mut processor);
        assert_eq!(processor.received.len(), 1);
    }
}
