#![no_std]
#![deny(unsafe_code)]

extern crate alloc;

pub mod color;
pub mod event;
pub mod geometry;
pub mod layout;
pub mod paint;
pub mod transform;
pub mod tree;
pub mod units;
pub mod widget;

pub use color::{BLACK, BLUE, Color, GREEN, RED, TRANSPARENT, WHITE};
pub use event::{
    Event, EventDispatcher, EventResponse, KeyCode, KeyEvent, Modifiers, MouseButton, MouseEvent,
    PointerEvent,
};
pub use geometry::{Insets, Insetsf, Point, Pointf, Rect, Rectf, Size, Sizef};
pub use layout::{Alignment, LayoutBox, LayoutDirection, LayoutEngine, LayoutNode, LayoutResult};
pub use paint::{ColorStop, CommandList, ImageId, PaintCommand, RenderBackend, TextLayoutId};
pub use transform::Transform;
pub use tree::{TraverseOrder, Tree};
pub use units::{Dp, Px};
pub use widget::{LayoutConstraints, Widget, WidgetId, downcast_widget_mut, downcast_widget_ref};
