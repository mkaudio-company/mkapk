#![allow(unexpected_cfgs, deprecated)]

use mkapk_core::{GpuContext, GpuSurface, Pointf, Rectf, Sizef};
use mkapk_host::{
    EditorHost, NormalizedValue, ParameterId, ParentWindowHandle, PluginEditor, SizeConstraints,
};

struct GpuEditor {
    view: *mut core::ffi::c_void,
    size: Sizef,
    surface: GpuSurface,
}

impl GpuEditor {
    fn new() -> Self {
        let surface = GpuSurface::new();
        surface.set_frame(Rectf::new(Pointf::new(0.0, 0.0), Sizef::new(400.0, 300.0)));
        surface.on_render(|ctx| match ctx {
            #[cfg(target_os = "windows")]
            GpuContext::D3D11(ctx) => {
                mkapk_win32::clear_render_target(ctx, [0.05, 0.05, 0.2, 1.0]);
            }
            #[cfg(target_os = "macos")]
            GpuContext::Metal(ctx) => {
                mkapk_mac::clear_to_color(ctx, 0.05, 0.05, 0.2, 1.0);
            }
            _ => {}
        });
        Self {
            view: core::ptr::null_mut(),
            size: Sizef::new(400.0, 300.0),
            surface,
        }
    }
}

impl PluginEditor for GpuEditor {
    fn open(&mut self, parent: ParentWindowHandle, _host: &dyn EditorHost) {
        match parent {
            ParentWindowHandle::Mac(ptr) | ParentWindowHandle::Windows(ptr) => {
                self.view = ptr;
            }
        }
    }

    fn close(&mut self) {}

    fn resize(&mut self, size: Sizef) {
        self.size = size;
        self.surface
            .set_frame(Rectf::new(Pointf::new(0.0, 0.0), size));
    }

    fn idle(&mut self) {
        let surface = &self.surface;
        let mut callback = |ctx: &mut GpuContext| {
            surface.render(ctx);
        };

        #[cfg(target_os = "macos")]
        {
            mkapk_mac::render_gpu_surface_to_view(self.view, self.size, &mut callback);
        }
        #[cfg(target_os = "windows")]
        {
            mkapk_win32::render_gpu_surface_to_hwnd(self.view, self.size, &mut callback);
        }
    }

    fn on_parameter_changed(&mut self, _id: ParameterId, _value: NormalizedValue) {}

    fn size_constraints(&self) -> SizeConstraints {
        SizeConstraints::default()
    }
}

fn main() {
    mkapk_test_host::run_test_host_with_editor(1000, 400, 300, GpuEditor::new());
}
