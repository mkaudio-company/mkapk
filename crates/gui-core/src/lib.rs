#![no_std]
#![deny(unsafe_code)]

extern crate alloc;

pub mod color;
pub mod geometry;
pub mod paint;
pub mod transform;
pub mod tree;
pub mod units;
pub mod widget;

pub use color::{BLACK, BLUE, Color, GREEN, RED, TRANSPARENT, WHITE};
pub use geometry::{Insets, Insetsf, Point, Pointf, Rect, Rectf, Size, Sizef};
pub use paint::{ColorStop, CommandList, ImageId, PaintCommand, RenderBackend, TextLayoutId};
pub use transform::Transform;
pub use tree::{TraverseOrder, Tree};
pub use units::{Dp, Px};
pub use widget::{LayoutConstraints, Widget, WidgetId};
