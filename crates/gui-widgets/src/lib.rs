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
    use gui_core::{AccessibilityNode, Role, Widget};
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

    #[test]
    fn slider_accessibility_uses_role_label_and_percentage_value() {
        let mut slider = Slider::new(ParameterId(1), NormalizedValue::new(0.75), Theme::default());
        slider.set_label("Gain");
        let node = slider.accessibility();
        assert_eq!(node.role, Role::Slider);
        assert_eq!(node.label.as_deref(), Some("Gain"));
        assert_eq!(node.value.as_deref(), Some("75%"));
    }

    #[test]
    fn knob_accessibility_uses_role_label_and_value() {
        let mut knob = Knob::new(ParameterId(2), NormalizedValue::new(0.25), Theme::default());
        knob.set_label("Tone");
        let node = knob.accessibility();
        assert_eq!(node.role, Role::Knob);
        assert_eq!(node.label.as_deref(), Some("Tone"));
        assert_eq!(node.value.as_deref(), Some("0.25"));
    }

    #[test]
    fn button_accessibility_uses_role_and_label() {
        let button = Button::new("Click Me", Theme::default());
        let node = button.accessibility();
        assert_eq!(node.role, Role::Button);
        assert_eq!(node.label.as_deref(), Some("Click Me"));
    }

    #[test]
    fn label_accessibility_uses_role_and_text() {
        let label = Label::new("Master", Theme::default());
        let node = label.accessibility();
        assert_eq!(node.role, Role::Label);
        assert_eq!(node.label.as_deref(), Some("Master"));
    }

    #[test]
    fn widget_trait_accessibility_can_be_called_explicitly() {
        let button = Button::new("Click", Theme::default());
        let node: AccessibilityNode = gui_core::Widget::accessibility(&button);
        assert_eq!(node.role, Role::Button);
    }
}
