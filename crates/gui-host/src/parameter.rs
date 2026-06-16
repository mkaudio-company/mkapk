#[derive(Clone, Copy, Eq, Hash, Debug, Ord, PartialEq, PartialOrd)]
pub struct ParameterId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalizedValue(f64);

impl NormalizedValue {
    pub fn new(value: f64) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    pub fn new_unchecked(value: f64) -> Self {
        Self(value)
    }

    pub fn get(&self) -> f64 {
        self.0
    }
}

pub struct ParameterInfo {
    pub id: ParameterId,
    pub name: &'static str,
    pub default_value: NormalizedValue,
    pub min_value: NormalizedValue,
    pub max_value: NormalizedValue,
    pub step_count: Option<u32>,
}

pub trait ParameterGateway {
    fn begin_gesture(&self, id: ParameterId);
    fn end_gesture(&self, id: ParameterId);
    fn set_normalized(&self, id: ParameterId, value: NormalizedValue);
    fn get_normalized(&self, id: ParameterId) -> Option<NormalizedValue>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParameterMessage {
    BeginGesture(ParameterId),
    EndGesture(ParameterId),
    SetNormalized(ParameterId, NormalizedValue),
}

pub struct LockFreeParameterGateway {
    ui_to_audio: crossbeam_channel::Sender<ParameterMessage>,
    ui_to_audio_rx: crossbeam_channel::Receiver<ParameterMessage>,
    audio_to_ui: crossbeam_channel::Sender<ParameterMessage>,
    audio_to_ui_rx: crossbeam_channel::Receiver<ParameterMessage>,
    current_values: std::sync::Mutex<std::collections::BTreeMap<ParameterId, NormalizedValue>>,
}

impl LockFreeParameterGateway {
    pub fn new(capacity: usize) -> Self {
        let (ui_to_audio, ui_to_audio_rx) = crossbeam_channel::bounded(capacity);
        let (audio_to_ui, audio_to_ui_rx) = crossbeam_channel::bounded(capacity);
        Self {
            ui_to_audio,
            ui_to_audio_rx,
            audio_to_ui,
            audio_to_ui_rx,
            current_values: std::sync::Mutex::new(std::collections::BTreeMap::new()),
        }
    }

    pub fn poll_ui_changes(&self, mut f: impl FnMut(ParameterMessage)) {
        while let Ok(msg) = self.audio_to_ui_rx.try_recv() {
            f(msg);
        }
    }

    pub fn poll_audio_changes(&self, mut f: impl FnMut(ParameterMessage)) {
        while let Ok(msg) = self.ui_to_audio_rx.try_recv() {
            f(msg);
        }
    }

    pub fn send_from_audio(
        &self,
        id: ParameterId,
        value: NormalizedValue,
    ) -> Result<(), crossbeam_channel::TrySendError<ParameterMessage>> {
        self.audio_to_ui
            .try_send(ParameterMessage::SetNormalized(id, value))
    }
}

impl Default for LockFreeParameterGateway {
    fn default() -> Self {
        Self::new(256)
    }
}

impl ParameterGateway for LockFreeParameterGateway {
    fn begin_gesture(&self, id: ParameterId) {
        let _result = self
            .ui_to_audio
            .try_send(ParameterMessage::BeginGesture(id));
    }

    fn end_gesture(&self, id: ParameterId) {
        let _result = self.ui_to_audio.try_send(ParameterMessage::EndGesture(id));
    }

    fn set_normalized(&self, id: ParameterId, value: NormalizedValue) {
        let _result = self
            .ui_to_audio
            .try_send(ParameterMessage::SetNormalized(id, value));
        if let Ok(mut values) = self.current_values.lock() {
            values.insert(id, value);
        }
    }

    fn get_normalized(&self, id: ParameterId) -> Option<NormalizedValue> {
        self.current_values.lock().ok()?.get(&id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::TrySendError;

    #[test]
    fn normalized_value_clamps_below_zero() {
        assert_eq!(NormalizedValue::new(-0.5).get(), 0.0);
    }

    #[test]
    fn normalized_value_clamps_above_one() {
        assert_eq!(NormalizedValue::new(1.5).get(), 1.0);
    }

    #[test]
    fn normalized_value_preserves_in_range() {
        assert_eq!(NormalizedValue::new(0.3).get(), 0.3);
    }

    #[test]
    fn normalized_value_unchecked_does_not_clamp() {
        assert_eq!(NormalizedValue::new_unchecked(2.0).get(), 2.0);
    }

    #[test]
    fn ui_to_audio_changes_are_drained_in_order() {
        let gateway = LockFreeParameterGateway::new(8);
        let id = ParameterId(7);
        gateway.begin_gesture(id);
        gateway.set_normalized(id, NormalizedValue::new(0.5));
        gateway.end_gesture(id);

        let mut messages = Vec::new();
        gateway.poll_audio_changes(|msg| messages.push(msg));

        assert_eq!(
            messages,
            vec![
                ParameterMessage::BeginGesture(id),
                ParameterMessage::SetNormalized(id, NormalizedValue::new(0.5)),
                ParameterMessage::EndGesture(id),
            ]
        );
    }

    #[test]
    fn audio_to_ui_changes_are_drained() {
        let gateway = LockFreeParameterGateway::new(8);
        let id = ParameterId(3);
        let value = NormalizedValue::new(0.75);
        gateway.send_from_audio(id, value).unwrap();

        let mut messages = Vec::new();
        gateway.poll_ui_changes(|msg| messages.push(msg));

        assert_eq!(messages, vec![ParameterMessage::SetNormalized(id, value)]);
    }

    #[test]
    fn get_normalized_returns_latest_ui_value() {
        let gateway = LockFreeParameterGateway::new(8);
        let id = ParameterId(1);
        gateway.set_normalized(id, NormalizedValue::new(0.2));
        gateway.set_normalized(id, NormalizedValue::new(0.9));
        assert_eq!(gateway.get_normalized(id), Some(NormalizedValue::new(0.9)));
    }

    #[test]
    fn audio_to_ui_channel_is_bounded() {
        let gateway = LockFreeParameterGateway::new(1);
        let id = ParameterId(5);
        gateway
            .send_from_audio(id, NormalizedValue::new(0.1))
            .unwrap();
        let result = gateway.send_from_audio(id, NormalizedValue::new(0.2));
        assert!(matches!(result, Err(TrySendError::Full(_))));
    }
}
