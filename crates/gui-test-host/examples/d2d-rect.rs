#![allow(unexpected_cfgs)]

use gui_core::{
    BLUE, Color, ColorStop, CommandList, GREEN, PaintCommand, Pointf, RED, Rectf, Sizef,
};
use gui_host::{
    EditorHost, NormalizedValue, ParameterId, ParentWindowHandle, PluginEditor, SizeConstraints,
};

struct RectEditor {
    hwnd: *mut core::ffi::c_void,
    size: Sizef,
    scale: f32,
}

impl RectEditor {
    fn new() -> Self {
        Self {
            hwnd: core::ptr::null_mut(),
            size: Sizef::new(400.0, 300.0),
            scale: 1.0,
        }
    }
}

impl PluginEditor for RectEditor {
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
        let commands = build_commands(self.size);

        #[cfg(target_os = "windows")]
        {
            gui_win32::render_to_hwnd(self.hwnd, self.size, self.scale, &commands);
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = commands;
            println!("d2d-rect: Direct2D backend is Windows-only");
        }
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
    gui_test_host::run_test_host_with_editor(1000, 400, 300, RectEditor::new());
}
