#![cfg_attr(not(aax_sdk), deny(unsafe_code))]

use gui_core::Sizef;
use gui_host::{EditorHost, NormalizedValue, ParameterId, ParentWindowHandle, PluginEditor};

struct NoopHost;

impl EditorHost for NoopHost {
    fn request_resize(&self, _size: Sizef) {}
    fn start_parameter_gesture(&self, _id: ParameterId) {}
    fn end_parameter_gesture(&self, _id: ParameterId) {}
    fn set_parameter_normalized(&self, _id: ParameterId, _value: NormalizedValue) {}
}

pub struct AaxEditor {
    editor: Box<dyn PluginEditor>,
    host: Option<Box<dyn EditorHost>>,
}

impl AaxEditor {
    pub fn new(editor: Box<dyn PluginEditor>) -> Self {
        Self { editor, host: None }
    }

    pub fn set_host(&mut self, host: Box<dyn EditorHost>) {
        self.host = Some(host);
    }

    #[allow(clippy::result_unit_err)]
    pub fn create_view(&mut self, parent: ParentWindowHandle) -> Result<(), ()> {
        let host: &dyn EditorHost = match &self.host {
            Some(host) => &**host,
            None => &NoopHost,
        };
        self.editor.open(parent, host);
        Ok(())
    }

    pub fn view_size(&self) -> Option<Sizef> {
        Some(
            self.editor
                .size_constraints()
                .min_size
                .unwrap_or_else(|| Sizef::new(400.0, 300.0)),
        )
    }

    pub fn draw(&mut self) {}

    pub fn timer_wakeup(&mut self) {
        self.editor.idle();
    }

    pub fn set_parameter(&mut self, id: u32, normalized: f32) {
        self.editor
            .on_parameter_changed(ParameterId(id), NormalizedValue::new(f64::from(normalized)));
    }

    pub fn destroy_view(&mut self) {
        self.editor.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gui_host::{EditorHost, NormalizedValue, ParameterId, PluginEditor, SizeConstraints};

    struct MockEditor;

    impl PluginEditor for MockEditor {
        fn open(&mut self, _parent: ParentWindowHandle, _host: &dyn EditorHost) {}
        fn close(&mut self) {}
        fn resize(&mut self, _size: Sizef) {}
        fn idle(&mut self) {}
        fn on_parameter_changed(&mut self, _id: ParameterId, _value: NormalizedValue) {}
        fn size_constraints(&self) -> SizeConstraints {
            SizeConstraints::default()
        }
    }

    #[test]
    fn aax_editor_lifecycle() {
        let mut editor = AaxEditor::new(Box::new(MockEditor));
        editor
            .create_view(ParentWindowHandle::Mac(core::ptr::null_mut()))
            .unwrap();
        let _ = editor.view_size();
        editor.timer_wakeup();
        editor.set_parameter(1, 0.5);
        editor.destroy_view();
    }
}
