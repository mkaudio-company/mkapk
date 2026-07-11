use core::cell::Cell;

use gui_core::{
    AccessibilityNode, CommandList, EventResponse, LayoutConstraints, MouseButton, MouseEvent,
    PaintCommand, PointerEvent, Pointf, Rectf, Role, Sizef, Widget, WidgetId,
};
use gui_host::{NormalizedValue, ParameterId};

use crate::Theme;

type ParameterCallback = Option<Box<dyn FnMut(ParameterId, NormalizedValue)>>;

const PREFERRED_WIDTH: f32 = 200.0;
const PREFERRED_HEIGHT: f32 = 24.0;
const TRACK_HEIGHT: f32 = 8.0;
const THUMB_SIZE: f32 = 16.0;

pub struct Slider {
    id: WidgetId,
    parameter_id: ParameterId,
    value: Cell<NormalizedValue>,
    label: Option<String>,
    theme: Theme,
    frame: Cell<Rectf>,
    on_change: Cell<ParameterCallback>,
}

impl Slider {
    pub fn new(parameter_id: ParameterId, value: NormalizedValue, theme: Theme) -> Self {
        Self {
            id: WidgetId::new(),
            parameter_id,
            value: Cell::new(value),
            label: None,
            theme,
            frame: Cell::new(Rectf::default()),
            on_change: Cell::new(None),
        }
    }

    pub fn set_frame(&self, frame: Rectf) {
        self.frame.set(frame);
    }

    pub fn value(&self) -> NormalizedValue {
        self.value.get()
    }

    pub fn set_value(&self, value: NormalizedValue) {
        self.value.set(value);
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = Some(label.into());
    }

    pub fn on_changed<F>(&self, callback: F)
    where
        F: FnMut(ParameterId, NormalizedValue) + 'static,
    {
        self.on_change.set(Some(Box::new(callback)));
    }

    fn update_from_x(&self, x: f32) {
        let frame = self.frame.get();
        if frame.size.width <= 0.0 {
            return;
        }
        let normalized = ((x - frame.origin.x) / frame.size.width) as f64;
        let new_value = NormalizedValue::new(normalized);
        self.value.set(new_value);
        if let Some(mut callback) = self.on_change.take() {
            callback(self.parameter_id, new_value);
            self.on_change.set(Some(callback));
        }
    }
}

impl Widget for Slider {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout(&self, _constraints: LayoutConstraints) -> Sizef {
        Sizef::new(PREFERRED_WIDTH, PREFERRED_HEIGHT)
    }

    fn paint(&self, commands: &mut CommandList) {
        let frame = self.frame.get();
        if frame.size.width <= 0.0 || frame.size.height <= 0.0 {
            return;
        }

        let track_y = frame.origin.y + (frame.size.height - TRACK_HEIGHT) / 2.0;
        let track_rect = Rectf::new(
            Pointf::new(frame.origin.x, track_y),
            Sizef::new(frame.size.width, TRACK_HEIGHT),
        );

        commands.push(PaintCommand::FillRoundedRect {
            rect: track_rect,
            radius: TRACK_HEIGHT / 2.0,
            color: self.theme.surface,
        });

        let value = self.value.get().get() as f32;
        let fill_width = frame.size.width * value;
        if fill_width > 0.0 {
            let fill_rect = Rectf::new(
                Pointf::new(frame.origin.x, track_y),
                Sizef::new(fill_width, TRACK_HEIGHT),
            );
            commands.push(PaintCommand::FillRoundedRect {
                rect: fill_rect,
                radius: TRACK_HEIGHT / 2.0,
                color: self.theme.primary,
            });
        }

        let thumb_center_x = frame.origin.x + fill_width;
        let thumb_center_y = frame.origin.y + frame.size.height / 2.0;
        let thumb_rect = Rectf::new(
            Pointf::new(
                thumb_center_x - THUMB_SIZE / 2.0,
                thumb_center_y - THUMB_SIZE / 2.0,
            ),
            Sizef::new(THUMB_SIZE, THUMB_SIZE),
        );
        commands.push(PaintCommand::FillRoundedRect {
            rect: thumb_rect,
            radius: THUMB_SIZE / 2.0,
            color: self.theme.text,
        });
    }

    fn on_mouse_down(&mut self, event: &MouseEvent) -> EventResponse {
        if event.button != MouseButton::Left {
            return EventResponse::Bubble;
        }
        self.update_from_x(event.position.x);
        EventResponse::Handled
    }

    fn on_mouse_move(&mut self, event: &PointerEvent) -> EventResponse {
        self.update_from_x(event.position.x);
        EventResponse::Handled
    }

    fn accessibility(&self) -> AccessibilityNode {
        let mut node = AccessibilityNode::new(self.id).with_role(Role::Slider);
        node.label = self.label.clone();
        let value = self.value.get().get();
        node.value = Some(format!("{:.0}%", value * 100.0));
        node
    }
}
