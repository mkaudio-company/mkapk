use core::sync::atomic::{AtomicU64, Ordering};

use crate::{CommandList, Pointf, Sizef};

static WIDGET_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct WidgetId(pub u64);

impl WidgetId {
    pub fn new() -> Self {
        Self(WIDGET_ID_COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

pub trait Widget {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayoutConstraints {
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
}
