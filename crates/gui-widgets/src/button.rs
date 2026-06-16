use core::cell::Cell;

use gui_core::{
    AccessibilityNode, CommandList, EventResponse, LayoutConstraints, MouseButton, MouseEvent,
    PaintCommand, Pointf, Rectf, Role, Sizef, TextLayoutId, Widget, WidgetId,
};

use crate::Theme;

const MIN_BUTTON_WIDTH: f32 = 80.0;
const BUTTON_HEIGHT: f32 = 36.0;

pub struct Button {
    id: WidgetId,
    label: String,
    theme: Theme,
    frame: Cell<Rectf>,
    on_click: Cell<Option<Box<dyn FnMut()>>>,
}

impl Button {
    pub fn new(label: impl Into<String>, theme: Theme) -> Self {
        Self {
            id: WidgetId::new(),
            label: label.into(),
            theme,
            frame: Cell::new(Rectf::default()),
            on_click: Cell::new(None),
        }
    }

    pub fn set_frame(&self, frame: Rectf) {
        self.frame.set(frame);
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    pub fn on_click<F>(&self, callback: F)
    where
        F: FnMut() + 'static,
    {
        self.on_click.set(Some(Box::new(callback)));
    }
}

impl Widget for Button {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout(&self, _constraints: LayoutConstraints) -> Sizef {
        let text_width = self.label.len() as f32 * self.theme.font_size * 0.6;
        let width = (text_width + self.theme.padding.horizontal()).max(MIN_BUTTON_WIDTH);
        Sizef::new(width, BUTTON_HEIGHT)
    }

    fn paint(&self, commands: &mut CommandList) {
        let frame = self.frame.get();
        if frame.size.width <= 0.0 || frame.size.height <= 0.0 {
            return;
        }

        commands.push(PaintCommand::FillRoundedRect {
            rect: frame,
            radius: self.theme.corner_radius,
            color: self.theme.surface,
        });

        commands.push(PaintCommand::StrokeRoundedRect {
            rect: frame,
            radius: self.theme.corner_radius,
            color: self.theme.border,
            width: self.theme.border_width,
        });

        commands.push(PaintCommand::DrawText {
            position: Pointf::new(
                frame.origin.x + self.theme.padding.left,
                frame.origin.y + self.theme.padding.top,
            ),
            text: TextLayoutId(0),
        });
    }

    fn on_mouse_down(&mut self, event: &MouseEvent) -> EventResponse {
        if event.button != MouseButton::Left {
            return EventResponse::Bubble;
        }
        let frame = self.frame.get();
        if !frame.contains(event.position) {
            return EventResponse::Bubble;
        }
        if let Some(mut callback) = self.on_click.take() {
            callback();
            self.on_click.set(Some(callback));
        }
        EventResponse::Handled
    }

    fn accessibility(&self) -> AccessibilityNode {
        AccessibilityNode::new(self.id)
            .with_role(Role::Button)
            .with_label(self.label.clone())
    }
}
