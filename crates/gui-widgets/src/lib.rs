#![deny(unsafe_code)]

pub mod button;
pub mod knob;
pub mod label;
pub mod slider;
pub mod theme;

pub use button::Button;
pub use knob::Knob;
pub use label::Label;
pub use slider::Slider;
pub use theme::Theme;

#[cfg(test)]
mod tests {
    use gui_core::Widget;
    use gui_host::{NormalizedValue, ParameterId};

    use super::*;

    fn assert_widget<T: Widget>(_: &T) {}

    #[test]
    fn slider_can_be_constructed_and_implements_widget() {
        let slider = Slider::new(ParameterId(1), NormalizedValue::new(0.5), Theme::default());
        assert_widget(&slider);
        assert_ne!(slider.id(), gui_core::WidgetId::default());
    }

    #[test]
    fn knob_can_be_constructed_and_implements_widget() {
        let knob = Knob::new(ParameterId(2), NormalizedValue::new(0.25), Theme::default());
        assert_widget(&knob);
        assert_ne!(knob.id(), gui_core::WidgetId::default());
    }

    #[test]
    fn button_can_be_constructed_and_implements_widget() {
        let button = Button::new("Click", Theme::default());
        assert_widget(&button);
        assert_ne!(button.id(), gui_core::WidgetId::default());
    }

    #[test]
    fn label_can_be_constructed_and_implements_widget() {
        let label = Label::new("Gain", Theme::default());
        assert_widget(&label);
        assert_ne!(label.id(), gui_core::WidgetId::default());
    }
}
