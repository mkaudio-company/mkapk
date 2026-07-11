//! `GpuSurface` widget - raw GPU drawing surface for advanced visualization.
//!
//! This is an opt-in, advanced path for visualizations such as spectrum
//! analyzers and oscilloscopes. It is **not** the default 2D rendering path;
//! basic widgets should continue to use the retained-mode paint command list.

use alloc::boxed::Box;
use core::cell::Cell;

use crate::{CommandList, LayoutConstraints, PaintCommand, Rectf, Sizef, Widget, WidgetId};

/// Raw D3D11 handles exposed to a user render callback.
///
/// All pointers are platform-owned and are only valid for the duration of the
/// callback invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct D3D11Context {
    pub device: *mut core::ffi::c_void,
    pub context: *mut core::ffi::c_void,
    pub render_target: *mut core::ffi::c_void,
}

/// Raw Metal handles exposed to a user render callback.
///
/// All pointers are platform-owned and are only valid for the duration of the
/// callback invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetalContext {
    pub device: *mut core::ffi::c_void,
    pub command_queue: *mut core::ffi::c_void,
    pub layer: *mut core::ffi::c_void,
}

/// Platform-specific GPU context passed to the user render callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GpuContext {
    #[default]
    Stub,
    D3D11(D3D11Context),
    Metal(MetalContext),
}

type RenderCallback = Box<dyn FnMut(&mut GpuContext) + 'static>;

/// A widget that delegates rendering to a user-provided GPU callback.
///
/// Intended for advanced visualization where direct GPU access is required.
/// The default 2D backend ignores this widget; the platform GPU helper renders
/// it after the 2D pass.
pub struct GpuSurface {
    id: WidgetId,
    frame: Cell<Rectf>,
    callback: Cell<Option<RenderCallback>>,
}

impl GpuSurface {
    pub fn new() -> Self {
        Self {
            id: WidgetId::new(),
            frame: Cell::new(Rectf::default()),
            callback: Cell::new(None),
        }
    }

    pub fn set_frame(&self, frame: Rectf) {
        self.frame.set(frame);
    }

    pub fn frame(&self) -> Rectf {
        self.frame.get()
    }

    /// Set the callback invoked each frame to render into the GPU surface.
    pub fn on_render<F>(&self, callback: F)
    where
        F: FnMut(&mut GpuContext) + 'static,
    {
        self.callback.set(Some(Box::new(callback)));
    }

    /// Invoke the stored render callback, if any.
    pub fn render(&self, context: &mut GpuContext) {
        if let Some(mut callback) = self.callback.take() {
            callback(context);
            self.callback.set(Some(callback));
        }
    }
}

impl Default for GpuSurface {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for GpuSurface {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout(&self, _constraints: LayoutConstraints) -> Sizef {
        let frame = self.frame.get();
        if frame.size.width > 0.0 && frame.size.height > 0.0 {
            frame.size
        } else {
            Sizef::new(400.0, 300.0)
        }
    }

    fn paint(&self, commands: &mut CommandList) {
        let frame = self.frame.get();
        if frame.size.width > 0.0 && frame.size.height > 0.0 {
            commands.push(PaintCommand::DrawGpuSurface { rect: frame });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    #[test]
    fn gpu_surface_constructs_and_implements_widget() {
        let surface = GpuSurface::new();
        let _: &dyn Widget = &surface;
        assert_ne!(surface.id(), WidgetId::default());
    }

    #[test]
    fn gpu_context_has_non_zero_size() {
        assert!(size_of::<GpuContext>() > 0);
    }

    #[test]
    fn gpu_surface_callback_can_be_set_and_invoked() {
        use alloc::rc::Rc;
        use core::cell::Cell;

        let surface = GpuSurface::new();
        let called = Rc::new(Cell::new(false));
        let called_clone = Rc::clone(&called);
        surface.on_render(move |ctx| {
            *ctx = GpuContext::Stub;
            called_clone.set(true);
        });
        let mut ctx = GpuContext::D3D11(D3D11Context {
            device: core::ptr::null_mut(),
            context: core::ptr::null_mut(),
            render_target: core::ptr::null_mut(),
        });
        surface.render(&mut ctx);
        assert!(called.get());
        assert_eq!(ctx, GpuContext::Stub);
    }
}
