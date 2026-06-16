use core::cell::Cell;
use core::f32::consts::PI;

use gui_core::{
    CommandList, EventResponse, LayoutConstraints, MouseButton, MouseEvent, PaintCommand,
    PointerEvent, Pointf, Rectf, Sizef, Widget, WidgetId,
};
use gui_host::{NormalizedValue, ParameterId};

use crate::Theme;

type ParameterCallback = Option<Box<dyn FnMut(ParameterId, NormalizedValue)>>;

const PREFERRED_SIZE: f32 = 60.0;
const INDICATOR_RADIUS: f32 = 5.0;

pub struct Knob {
    id: WidgetId,
    parameter_id: ParameterId,
    value: Cell<NormalizedValue>,
    theme: Theme,
    frame: Cell<Rectf>,
    on_change: Cell<ParameterCallback>,
}

impl Knob {
    pub fn new(parameter_id: ParameterId, value: NormalizedValue, theme: Theme) -> Self {
        Self {
            id: WidgetId::new(),
            parameter_id,
            value: Cell::new(value),
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

    pub fn on_changed<F>(&self, callback: F)
    where
        F: FnMut(ParameterId, NormalizedValue) + 'static,
    {
        self.on_change.set(Some(Box::new(callback)));
    }

    fn update_from_y(&self, y: f32) {
        let frame = self.frame.get();
        if frame.size.height <= 0.0 {
            return;
        }
        let normalized = (1.0 - (y - frame.origin.y) / frame.size.height) as f64;
        let new_value = NormalizedValue::new(normalized);
        self.value.set(new_value);
        if let Some(mut callback) = self.on_change.take() {
            callback(self.parameter_id, new_value);
            self.on_change.set(Some(callback));
        }
    }
}

impl Widget for Knob {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout(&self, _constraints: LayoutConstraints) -> Sizef {
        Sizef::new(PREFERRED_SIZE, PREFERRED_SIZE)
    }

    fn paint(&self, commands: &mut CommandList) {
        let frame = self.frame.get();
        if frame.size.width <= 0.0 || frame.size.height <= 0.0 {
            return;
        }

        let radius = (frame.size.width / 2.0).min(frame.size.height / 2.0);
        let center_x = frame.origin.x + frame.size.width / 2.0;
        let center_y = frame.origin.y + frame.size.height / 2.0;

        commands.push(PaintCommand::FillRoundedRect {
            rect: frame,
            radius,
            color: self.theme.surface,
        });

        commands.push(PaintCommand::StrokeRoundedRect {
            rect: frame,
            radius,
            color: self.theme.border,
            width: self.theme.border_width,
        });

        let value = self.value.get().get() as f32;
        let start_angle = -3.0 * PI / 4.0;
        let sweep = 3.0 * PI / 2.0;
        let angle = start_angle + value * sweep;
        let indicator_distance = radius - INDICATOR_RADIUS - self.theme.border_width;
        let indicator_x = center_x + indicator_distance * angle.cos();
        let indicator_y = center_y + indicator_distance * angle.sin();

        let indicator_rect = Rectf::new(
            Pointf::new(
                indicator_x - INDICATOR_RADIUS,
                indicator_y - INDICATOR_RADIUS,
            ),
            Sizef::new(INDICATOR_RADIUS * 2.0, INDICATOR_RADIUS * 2.0),
        );
        commands.push(PaintCommand::FillRoundedRect {
            rect: indicator_rect,
            radius: INDICATOR_RADIUS,
            color: self.theme.primary,
        });
    }

    fn on_mouse_down(&mut self, event: &MouseEvent) -> EventResponse {
        if event.button != MouseButton::Left {
            return EventResponse::Bubble;
        }
        self.update_from_y(event.position.y);
        EventResponse::Handled
    }

    fn on_mouse_move(&mut self, event: &PointerEvent) -> EventResponse {
        self.update_from_y(event.position.y);
        EventResponse::Handled
    }
}
