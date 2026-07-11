use core::any::Any;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::accessibility::AccessibilityNode;
use crate::event::{EventResponse, KeyEvent, MouseEvent, PointerEvent};
use crate::{CommandList, Pointf, Sizef};

static WIDGET_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct WidgetId(pub u64);

impl WidgetId {
    pub fn new() -> Self {
        Self(WIDGET_ID_COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

pub trait Widget: Any {
    fn id(&self) -> WidgetId;
    fn mount(&mut self) {}
    fn unmount(&mut self) {}
    fn update(&mut self) {}
    fn paint(&self, _commands: &mut CommandList) {}
    fn layout(&self, _constraints: LayoutConstraints) -> Sizef {
        Sizef::default()
    }
    fn hit_test(&self, _point: Pointf) -> bool {
        false
    }
    fn on_mouse_down(&mut self, _event: &MouseEvent) -> EventResponse {
        EventResponse::Bubble
    }
    fn on_mouse_up(&mut self, _event: &MouseEvent) -> EventResponse {
        EventResponse::Bubble
    }
    fn on_mouse_move(&mut self, _event: &PointerEvent) -> EventResponse {
        EventResponse::Bubble
    }
    fn on_key_down(&mut self, _event: &KeyEvent) -> EventResponse {
        EventResponse::Bubble
    }
    fn on_key_up(&mut self, _event: &KeyEvent) -> EventResponse {
        EventResponse::Bubble
    }
    fn accessibility(&self) -> AccessibilityNode {
        AccessibilityNode::new(self.id())
    }
}

pub fn downcast_widget_ref<T: Widget>(widget: &dyn Widget) -> Option<&T> {
    (widget as &dyn Any).downcast_ref::<T>()
}

pub fn downcast_widget_mut<T: Widget>(widget: &mut dyn Widget) -> Option<&mut T> {
    (widget as &mut dyn Any).downcast_mut::<T>()
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayoutConstraints {
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
}
