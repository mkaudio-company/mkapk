use gui_core::{CommandList, PaintCommand, Sizef};
use gui_host::{
    EditorHost, NormalizedValue, ParameterId, ParentWindowHandle, PluginEditor, SizeConstraints,
};

struct GainEditor {
    view: Option<*mut core::ffi::c_void>,
    size: Sizef,
}

impl GainEditor {
    fn new() -> Self {
        Self {
            view: None,
            size: Sizef::new(400.0, 300.0),
        }
    }
}

impl PluginEditor for GainEditor {
    fn open(&mut self, parent: ParentWindowHandle, _host: &dyn EditorHost) {
        if let ParentWindowHandle::Mac(view) = parent {
            self.view = Some(view);
        }
    }

    fn resize(&mut self, size: Sizef) {
        self.size = size;
    }

    fn idle(&mut self) {
        let Some(view) = self.view else {
            return;
        };

        let mut commands = CommandList::new();
        commands.push(PaintCommand::Clear {
            color: gui_core::Color::from_rgb(30, 30, 30),
        });
        let _ = gui_mac::render_to_view(view, self.size, 1.0, &commands);
    }

    fn close(&mut self) {}

    fn on_parameter_changed(&mut self, _id: ParameterId, _value: NormalizedValue) {}

    fn size_constraints(&self) -> SizeConstraints {
        SizeConstraints::default()
    }
}

#[cfg(target_os = "macos")]
fn main() {
    gui_test_host::run_test_host_with_editor(1000, 400, 300, GainEditor::new());
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("This example is only supported on macOS.");
}
