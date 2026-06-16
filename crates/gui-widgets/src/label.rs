use core::cell::Cell;

use gui_core::{
    AccessibilityNode, CommandList, LayoutConstraints, PaintCommand, Pointf, Rectf, Role, Sizef,
    TextLayoutId, Widget, WidgetId,
};

use crate::Theme;

pub struct Label {
    id: WidgetId,
    text: String,
    theme: Theme,
    frame: Cell<Rectf>,
}

impl Label {
    pub fn new(text: impl Into<String>, theme: Theme) -> Self {
        Self {
            id: WidgetId::new(),
            text: text.into(),
            theme,
            frame: Cell::new(Rectf::default()),
        }
    }

    pub fn set_frame(&self, frame: Rectf) {
        self.frame.set(frame);
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }
}

impl Widget for Label {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout(&self, _constraints: LayoutConstraints) -> Sizef {
        let text_width = self.text.len() as f32 * self.theme.font_size * 0.6;
        Sizef::new(
            text_width + self.theme.padding.horizontal(),
            self.theme.font_size + self.theme.padding.vertical(),
        )
    }

    fn paint(&self, commands: &mut CommandList) {
        let frame = self.frame.get();
        commands.push(PaintCommand::DrawText {
            position: Pointf::new(
                frame.origin.x + self.theme.padding.left,
                frame.origin.y + self.theme.padding.top,
            ),
            text: TextLayoutId(0),
        });
    }

    fn accessibility(&self) -> AccessibilityNode {
        AccessibilityNode::new(self.id)
            .with_role(Role::Label)
            .with_label(self.text.clone())
    }
}
