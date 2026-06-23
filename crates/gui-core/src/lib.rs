//! Core types for the plugin GUI framework: geometry, color, paint commands,
//! widget tree, layout, events, animation, and accessibility metadata.
#![no_std]
#![deny(unsafe_code)]

extern crate alloc;

pub mod accessibility;
pub mod animation;
pub mod color;
pub mod event;
pub mod geometry;
pub mod layout;
pub mod paint;
pub mod transform;
pub mod tree;
pub mod units;
pub mod widget;

#[cfg(feature = "gpu-surface")]
pub mod widgets;

pub use accessibility::{AccessibilityNode, AccessibilityTree, Role, State};
pub use animation::{
    Animatable, Animation, AnimationController, AnimationCurve, AnimationEvent, AnimationId,
    AnimationState, AnimationTick,
};
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

#[cfg(feature = "gpu-surface")]
pub use widgets::gpu_surface::{D3D11Context, GpuContext, GpuSurface, MetalContext};
