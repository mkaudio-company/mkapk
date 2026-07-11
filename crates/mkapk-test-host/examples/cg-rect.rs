#![allow(unexpected_cfgs, deprecated)]

use mkapk_core::{
    BLUE, Color, ColorStop, CommandList, GREEN, PaintCommand, Pointf, RED, Rectf, Sizef,
};
use mkapk_host::{
    EditorHost, NormalizedValue, ParameterId, ParentWindowHandle, PluginEditor, SizeConstraints,
};

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

struct RectEditor {
    view: *mut core::ffi::c_void,
    size: Sizef,
    scale: f32,
}

impl RectEditor {
    fn new() -> Self {
        Self {
            view: core::ptr::null_mut(),
            size: Sizef::new(400.0, 300.0),
            scale: 1.0,
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

impl PluginEditor for RectEditor {
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
        mkapk_mac::render_to_view(self.view, self.size, self.scale, &commands);
    }

    fn on_parameter_changed(&mut self, _id: ParameterId, _value: NormalizedValue) {}

    fn size_constraints(&self) -> SizeConstraints {
        SizeConstraints::default()
    }
}

static GRADIENT_STOPS: &[ColorStop] = &[
    ColorStop {
        position: 0.0,
        color: RED,
    },
    ColorStop {
        position: 1.0,
        color: BLUE,
    },
];

static TRIANGLE: &[Pointf] = &[
    Pointf::new(50.0, 50.0),
    Pointf::new(100.0, 150.0),
    Pointf::new(0.0, 150.0),
];

fn build_commands(size: Sizef) -> CommandList {
    let mut commands = CommandList::new();

    commands.push(PaintCommand::Clear {
        color: Color::new(30, 30, 30, 255),
    });

    commands.push(PaintCommand::FillRect {
        rect: Rectf::new(
            Pointf::new(0.0, 0.0),
            Sizef::new(size.width / 2.0, size.height / 2.0),
        ),
        color: RED,
    });

    commands.push(PaintCommand::StrokeRect {
        rect: Rectf::new(
            Pointf::new(size.width / 4.0, size.height / 4.0),
            Sizef::new(size.width / 2.0, size.height / 2.0),
        ),
        color: BLUE,
        width: 2.0,
    });

    commands.push(PaintCommand::FillRoundedRect {
        rect: Rectf::new(
            Pointf::new(size.width / 2.0, 0.0),
            Sizef::new(size.width / 2.0, size.height / 2.0),
        ),
        radius: 20.0,
        color: GREEN,
    });

    commands.push(PaintCommand::FillPath {
        points: TRIANGLE,
        color: GREEN,
    });

    commands.push(PaintCommand::LinearGradient {
        rect: Rectf::new(
            Pointf::new(0.0, size.height / 2.0),
            Sizef::new(size.width, size.height / 2.0),
        ),
        start: Pointf::new(0.0, size.height / 2.0),
        end: Pointf::new(size.width, size.height),
        stops: GRADIENT_STOPS,
    });

    commands
}

fn main() {
    mkapk_test_host::run_test_host_with_editor(1000, 400, 300, RectEditor::new());
}
