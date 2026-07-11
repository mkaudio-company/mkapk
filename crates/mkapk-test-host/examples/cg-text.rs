#![allow(unexpected_cfgs, deprecated)]

use gui_core::{CommandList, PaintCommand, Pointf, Sizef, TextLayoutId};
use gui_host::{
    EditorHost, NormalizedValue, ParameterId, ParentWindowHandle, PluginEditor, SizeConstraints,
};

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

struct TextEditor {
    view: *mut core::ffi::c_void,
    size: Sizef,
    scale: f32,
    #[cfg(target_os = "macos")]
    layout: gui_mac::TextLayout,
}

impl TextEditor {
    fn new() -> Self {
        Self {
            view: core::ptr::null_mut(),
            size: Sizef::new(400.0, 300.0),
            scale: 1.0,
            #[cfg(target_os = "macos")]
            layout: gui_mac::TextLayout::new("Hello 世界 🎹", 24.0),
        }
    }
}

#[cfg(target_os = "macos")]
fn backing_scale_for_view(view: *mut core::ffi::c_void) -> f32 {
    use objc::runtime::{BOOL, Object, YES};

    if view.is_null() {
        return 1.0;
    }
    unsafe {
        let view = view as *mut Object;
        let can_draw: BOOL = msg_send![view, canDraw];
        if can_draw != YES {
            return 1.0;
        }
        let window: *mut Object = msg_send![view, window];
        if window.is_null() {
            return 1.0;
        }
        let scale: f64 = msg_send![window, backingScaleFactor];
        scale as f32
    }
}

#[cfg(not(target_os = "macos"))]
fn backing_scale_for_view(_view: *mut core::ffi::c_void) -> f32 {
    1.0
}

impl PluginEditor for TextEditor {
    fn open(&mut self, parent: ParentWindowHandle, _host: &dyn EditorHost) {
        if let ParentWindowHandle::Mac(ptr) = parent {
            self.view = ptr;
            self.scale = backing_scale_for_view(ptr);
        }
    }

    fn close(&mut self) {}

    fn resize(&mut self, size: Sizef) {
        self.size = size;
    }

    fn idle(&mut self) {
        let commands = build_commands(self.size);
        gui_mac::render_to_view(self.view, self.size, self.scale, &commands);

        #[cfg(target_os = "macos")]
        {
            let position = Pointf::new(20.0, 20.0);
            gui_mac::render_text_to_view(self.view, self.size, self.scale, &self.layout, position);
        }
    }

    fn on_parameter_changed(&mut self, _id: ParameterId, _value: NormalizedValue) {}

    fn size_constraints(&self) -> SizeConstraints {
        SizeConstraints::default()
    }
}

fn build_commands(size: Sizef) -> CommandList {
    let mut commands = CommandList::new();

    commands.push(PaintCommand::Clear {
        color: gui_core::Color::new(40, 40, 40, 255),
    });

    commands.push(PaintCommand::FillRect {
        rect: gui_core::Rectf::new(
            Pointf::new(10.0, 10.0),
            Sizef::new(size.width - 20.0, size.height - 20.0),
        ),
        color: gui_core::Color::new(60, 60, 60, 255),
    });

    commands.push(PaintCommand::DrawText {
        position: Pointf::new(20.0, 20.0),
        text: TextLayoutId(0),
    });

    commands
}

fn main() {
    gui_test_host::run_test_host_with_editor(1000, 400, 300, TextEditor::new());
}
