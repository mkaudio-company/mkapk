//! DAW-less standalone test host for exercising plugin editors outside a real
//! DAW.
#![allow(unexpected_cfgs, deprecated)]

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

use mkapk_core::Sizef;
#[cfg(target_os = "windows")]
use mkapk_core::{Event, EventResponse};
use mkapk_host::{
    EditorHost, NormalizedValue, ParameterId, ParentWindowHandle, PluginEditor, SizeConstraints,
};

pub mod platform;
pub use platform::{PlatformWindow, create_host_window};

/// Wires a `PlatformWindow`'s real mouse input through to `editor`. Only
/// meaningful on Windows: `PlatformWindow::set_input_sink` hooks the one
/// `wndproc` this crate owns, since a real Win32 host or DAW's window isn't
/// ours to subclass. On macOS, the editor's own view (see
/// `mkapk_mac::paint_view`) wires mouse handling directly during `open()`
/// instead, so this is a no-op there.
///
/// # Safety invariant
/// `editor` must outlive every future `window.pump_events()` call (the
/// sink dereferences a raw pointer back to it); call `window.destroy()`
/// (which clears the sink via `WM_NCDESTROY`) before dropping `editor`.
#[cfg(target_os = "windows")]
pub fn attach_mouse_input<E: PluginEditor + 'static>(window: &PlatformWindow, editor: &mut E) {
    let editor_ptr: *mut E = editor;
    window.set_input_sink(Box::new(move |event| {
        // SAFETY: see the function-level safety invariant above.
        let editor = unsafe { &mut *editor_ptr };
        match event {
            Event::MouseDown(e) => editor.on_mouse_down(&e),
            Event::MouseUp(e) => editor.on_mouse_up(&e),
            Event::MouseMove(e) => editor.on_mouse_move(&e),
            _ => EventResponse::Bubble,
        }
    }));
}

#[cfg(not(target_os = "windows"))]
pub fn attach_mouse_input<E: PluginEditor>(_window: &PlatformWindow, _editor: &mut E) {}

pub struct BlankEditor;

impl PluginEditor for BlankEditor {
    fn open(&mut self, _parent: ParentWindowHandle, _host: &dyn EditorHost) {
        println!("BlankEditor::open");
    }

    fn close(&mut self) {
        println!("BlankEditor::close");
    }

    fn resize(&mut self, _size: Sizef) {
        println!("BlankEditor::resize");
    }

    fn idle(&mut self) {
        println!("BlankEditor::idle");
    }

    fn on_parameter_changed(&mut self, _id: ParameterId, _value: NormalizedValue) {
        println!("BlankEditor::on_parameter_changed");
    }

    fn size_constraints(&self) -> SizeConstraints {
        SizeConstraints::default()
    }
}

struct TestHost;

impl EditorHost for TestHost {
    fn request_resize(&self, _size: Sizef) {}
    fn start_parameter_gesture(&self, _id: ParameterId) {}
    fn end_parameter_gesture(&self, _id: ParameterId) {}
    fn set_parameter_normalized(&self, _id: ParameterId, _value: NormalizedValue) {}
}

pub fn run_test_host_with_editor<E: PluginEditor + 'static>(
    duration_ms: u64,
    width: u32,
    height: u32,
    mut editor: E,
) {
    let (window, parent_handle) = create_host_window(width, height);
    let host = TestHost;

    println!("EditorAttached");
    editor.open(parent_handle, &host);

    let start = std::time::Instant::now();
    let duration = std::time::Duration::from_millis(duration_ms);
    while start.elapsed() < duration {
        if !window.pump_events() {
            break;
        }
        editor.idle();
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    editor.close();
    println!("EditorDetached");
    window.destroy();
}

pub fn run_test_host(duration_ms: u64, width: u32, height: u32) {
    run_test_host_with_editor(duration_ms, width, height, BlankEditor);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_editor_lifecycle_compiles_and_runs() {
        let mut editor = BlankEditor;
        let host = TestHost;
        editor.open(ParentWindowHandle::Mac(std::ptr::null_mut()), &host);
        editor.resize(Sizef::new(100.0, 200.0));
        editor.idle();
        editor.on_parameter_changed(ParameterId(0), NormalizedValue::new(0.5));
        editor.close();
    }

    #[test]
    fn size_constraints_default() {
        let editor = BlankEditor;
        assert_eq!(editor.size_constraints(), SizeConstraints::default());
    }
}
