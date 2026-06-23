#![allow(unexpected_cfgs)]

use gui_core::Sizef;

#[cfg(target_os = "windows")]
use gui_core::Pointf;
use gui_host::{
    EditorHost, NormalizedValue, ParameterId, ParentWindowHandle, PluginEditor, SizeConstraints,
};

struct TextEditor {
    hwnd: *mut core::ffi::c_void,
    size: Sizef,
    scale: f32,
    #[cfg(target_os = "windows")]
    layout: gui_win32::TextLayout,
}

impl TextEditor {
    fn new() -> Self {
        Self {
            hwnd: core::ptr::null_mut(),
            size: Sizef::new(400.0, 300.0),
            scale: 1.0,
            #[cfg(target_os = "windows")]
            layout: gui_win32::TextLayout::new("Hello DirectWrite", 24.0),
        }
    }
}

impl PluginEditor for TextEditor {
    fn open(&mut self, parent: ParentWindowHandle, _host: &dyn EditorHost) {
        if let ParentWindowHandle::Windows(ptr) = parent {
            self.hwnd = ptr;
            self.scale = 1.0;
        }
    }

    fn close(&mut self) {}

    fn resize(&mut self, size: Sizef) {
        self.size = size;
    }

    fn idle(&mut self) {
        #[cfg(target_os = "windows")]
        {
            let position = Pointf::new(20.0, 20.0);
            gui_win32::draw_text_to_hwnd(self.hwnd, self.size, self.scale, &self.layout, position);
        }

        #[cfg(not(target_os = "windows"))]
        {
            println!("d2d-text: DirectWrite text rendering is Windows-only");
        }
    }

    fn on_parameter_changed(&mut self, _id: ParameterId, _value: NormalizedValue) {}

    fn size_constraints(&self) -> SizeConstraints {
        SizeConstraints::default()
    }
}

fn main() {
    gui_test_host::run_test_host_with_editor(1000, 400, 300, TextEditor::new());
}
