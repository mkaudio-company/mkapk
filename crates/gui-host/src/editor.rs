use crate::parameter::{NormalizedValue, ParameterId};

pub trait PluginEditor {
    fn open(&mut self, parent: ParentWindowHandle, host: &dyn EditorHost);
    fn close(&mut self);
    fn resize(&mut self, size: gui_core::Sizef);
    fn idle(&mut self);
    fn on_parameter_changed(&mut self, id: ParameterId, value: NormalizedValue);
    fn size_constraints(&self) -> SizeConstraints;
}

pub trait EditorHost {
    fn request_resize(&self, size: gui_core::Sizef);
    fn start_parameter_gesture(&self, id: ParameterId);
    fn end_parameter_gesture(&self, id: ParameterId);
    fn set_parameter_normalized(&self, id: ParameterId, value: NormalizedValue);
}

#[derive(Clone, Copy, Debug)]
pub enum ParentWindowHandle {
    Windows(*mut core::ffi::c_void),
    Mac(*mut core::ffi::c_void),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SizeConstraints {
    pub min_size: Option<gui_core::Sizef>,
    pub max_size: Option<gui_core::Sizef>,
    pub aspect_ratio: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ptr::null_mut;

    #[derive(Default)]
    struct MockEditor {
        log: Vec<&'static str>,
    }

    impl PluginEditor for MockEditor {
        fn open(&mut self, _parent: ParentWindowHandle, _host: &dyn EditorHost) {
            self.log.push("open");
        }

        fn close(&mut self) {
            self.log.push("close");
        }

        fn resize(&mut self, _size: gui_core::Sizef) {
            self.log.push("resize");
        }

        fn idle(&mut self) {
            self.log.push("idle");
        }

        fn on_parameter_changed(&mut self, _id: ParameterId, _value: NormalizedValue) {
            self.log.push("parameter_changed");
        }

        fn size_constraints(&self) -> SizeConstraints {
            SizeConstraints::default()
        }
    }

    struct MockHost;

    impl EditorHost for MockHost {
        fn request_resize(&self, _size: gui_core::Sizef) {}
        fn start_parameter_gesture(&self, _id: ParameterId) {}
        fn end_parameter_gesture(&self, _id: ParameterId) {}
        fn set_parameter_normalized(&self, _id: ParameterId, _value: NormalizedValue) {}
    }

    #[test]
    fn lifecycle_methods_are_called() {
        let mut editor = MockEditor::default();
        let host = MockHost;

        editor.open(ParentWindowHandle::Windows(null_mut()), &host);
        editor.resize(gui_core::Sizef::new(100.0, 200.0));
        editor.idle();
        editor.close();

        assert_eq!(editor.log, vec!["open", "resize", "idle", "close"]);
    }
}
