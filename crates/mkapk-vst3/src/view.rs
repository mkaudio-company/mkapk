use std::cell::{Cell, RefCell};
use std::ffi::{CStr, c_void};
use std::mem;
use std::rc::Rc;

use mkapk_host::{EditorHost, NormalizedValue, ParameterId, ParentWindowHandle, PluginEditor};
use vst3_sys as vst3_com;
use vst3_sys::VST3;
use vst3_sys::VstPtr;
use vst3_sys::base::{FIDString, kInvalidArgument, kResultFalse, kResultOk, tresult};
use vst3_sys::gui::{IPlugFrame, IPlugView, ViewRect};
use vst3_sys::utils::SharedVstPtr;
use vst3_sys::vst::IComponentHandler;

/// Shared with the owning `IEditController`, so the view can forward
/// UI-originated parameter edits to the host via `IComponentHandler`
/// (`begin_edit`/`perform_edit`/`end_edit`), which is how the host learns
/// about automation and can round-trip the new value to the audio
/// processor. `None` outside a VST3 host (e.g. the DAW-less test host).
pub type ComponentHandlerCell = Rc<RefCell<Option<VstPtr<dyn IComponentHandler>>>>;

#[VST3(implements(IPlugView))]
pub struct PluginView {
    editor: RefCell<Box<dyn PluginEditor>>,
    frame: RefCell<Option<VstPtr<dyn IPlugFrame>>>,
    size: Cell<mkapk_core::Sizef>,
    component_handler: ComponentHandlerCell,
}

impl PluginView {
    pub fn new(editor: Box<dyn PluginEditor>) -> Box<Self> {
        Self::allocate(
            RefCell::new(editor),
            RefCell::new(None),
            Cell::new(mkapk_core::Sizef::new(400.0, 300.0)),
            Rc::new(RefCell::new(None)),
        )
    }

    /// Like `new`, but forwards parameter edits to `component_handler` (set
    /// by the owning `IEditController::set_component_handler`) instead of
    /// silently dropping them.
    pub fn with_component_handler(
        editor: Box<dyn PluginEditor>,
        component_handler: ComponentHandlerCell,
    ) -> Box<Self> {
        Self::allocate(
            RefCell::new(editor),
            RefCell::new(None),
            Cell::new(mkapk_core::Sizef::new(400.0, 300.0)),
            component_handler,
        )
    }
}

impl IPlugView for PluginView {
    unsafe fn is_platform_type_supported(&self, type_: FIDString) -> tresult {
        if type_.is_null() {
            return kInvalidArgument;
        }

        let type_ = unsafe { CStr::from_ptr(type_) };
        #[cfg(target_os = "windows")]
        {
            if type_.to_bytes() == b"HWND" {
                return kResultOk;
            }
        }
        #[cfg(target_os = "macos")]
        {
            let bytes = type_.to_bytes();
            if bytes == b"NSView" || bytes == b"Cocoa" {
                return kResultOk;
            }
        }

        kInvalidArgument
    }

    unsafe fn attached(&self, parent: *mut c_void, type_: FIDString) -> tresult {
        if parent.is_null() || type_.is_null() {
            return kInvalidArgument;
        }

        let type_ = unsafe { CStr::from_ptr(type_) };
        let bytes = type_.to_bytes();

        #[cfg(target_os = "windows")]
        {
            if bytes != b"HWND" {
                return kInvalidArgument;
            }
        }
        #[cfg(target_os = "macos")]
        {
            if bytes != b"NSView" && bytes != b"Cocoa" {
                return kInvalidArgument;
            }
        }

        let handle = {
            #[cfg(target_os = "windows")]
            {
                ParentWindowHandle::Windows(parent)
            }
            #[cfg(target_os = "macos")]
            {
                ParentWindowHandle::Mac(parent)
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos")))]
            {
                return kInvalidArgument;
            }
        };

        let host = ViewHost {
            frame: self.frame.borrow().clone(),
            view: self as *const _ as *mut c_void,
            component_handler: self.component_handler.clone(),
        };
        self.editor.borrow_mut().open(handle, &host);

        kResultOk
    }

    unsafe fn removed(&self) -> tresult {
        self.editor.borrow_mut().close();
        self.frame.borrow_mut().take();
        kResultOk
    }

    unsafe fn on_wheel(&self, _distance: f32) -> tresult {
        kResultFalse
    }

    unsafe fn on_key_down(
        &self,
        _key: vst3_sys::base::char16,
        _key_code: i16,
        _modifiers: i16,
    ) -> tresult {
        kResultFalse
    }

    unsafe fn on_key_up(
        &self,
        _key: vst3_sys::base::char16,
        _key_code: i16,
        _modifiers: i16,
    ) -> tresult {
        kResultFalse
    }

    unsafe fn get_size(&self, size: *mut ViewRect) -> tresult {
        if size.is_null() {
            return kInvalidArgument;
        }

        let s = self.size.get();
        unsafe {
            *size = ViewRect {
                left: 0,
                top: 0,
                right: s.width as i32,
                bottom: s.height as i32,
            };
        }

        kResultOk
    }

    unsafe fn on_size(&self, new_size: *mut ViewRect) -> tresult {
        if new_size.is_null() {
            return kInvalidArgument;
        }

        let rect = unsafe { &*new_size };
        let width = (rect.right - rect.left) as f32;
        let height = (rect.bottom - rect.top) as f32;
        let size = mkapk_core::Sizef::new(width, height);
        self.size.set(size);
        self.editor.borrow_mut().resize(size);

        kResultOk
    }

    unsafe fn on_focus(&self, _state: vst3_sys::base::TBool) -> tresult {
        kResultFalse
    }

    unsafe fn set_frame(&self, frame: *mut c_void) -> tresult {
        if frame.is_null() {
            *self.frame.borrow_mut() = None;
            return kResultOk;
        }

        let frame: SharedVstPtr<dyn IPlugFrame> = unsafe { mem::transmute(frame) };
        *self.frame.borrow_mut() = frame.upgrade();
        kResultOk
    }

    unsafe fn can_resize(&self) -> tresult {
        kResultOk
    }

    unsafe fn check_size_constraint(&self, _rect: *mut ViewRect) -> tresult {
        kResultOk
    }
}

struct ViewHost {
    frame: Option<VstPtr<dyn IPlugFrame>>,
    view: *mut c_void,
    component_handler: ComponentHandlerCell,
}

impl EditorHost for ViewHost {
    fn request_resize(&self, size: mkapk_core::Sizef) {
        if let Some(frame) = &self.frame {
            let mut rect = ViewRect {
                left: 0,
                top: 0,
                right: size.width as i32,
                bottom: size.height as i32,
            };
            let view_ptr: SharedVstPtr<dyn IPlugView> = unsafe { mem::transmute(self.view) };
            unsafe {
                frame.resize_view(view_ptr, &mut rect);
            }
        }
    }

    fn start_parameter_gesture(&self, id: ParameterId) {
        if let Some(handler) = self.component_handler.borrow().as_ref() {
            unsafe {
                handler.begin_edit(id.0);
            }
        }
    }

    fn end_parameter_gesture(&self, id: ParameterId) {
        if let Some(handler) = self.component_handler.borrow().as_ref() {
            unsafe {
                handler.end_edit(id.0);
            }
        }
    }

    fn set_parameter_normalized(&self, id: ParameterId, value: NormalizedValue) {
        if let Some(handler) = self.component_handler.borrow().as_ref() {
            unsafe {
                handler.perform_edit(id.0, value.get());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mkapk_host::SizeConstraints;
    use mkapk_host::editor::PluginEditor;

    struct TestEditor;

    impl PluginEditor for TestEditor {
        fn open(&mut self, _parent: ParentWindowHandle, _host: &dyn EditorHost) {}
        fn close(&mut self) {}
        fn resize(&mut self, _size: mkapk_core::Sizef) {}
        fn idle(&mut self) {}
        fn on_parameter_changed(&mut self, _id: ParameterId, _value: NormalizedValue) {}
        fn size_constraints(&self) -> SizeConstraints {
            SizeConstraints::default()
        }
    }

    #[test]
    fn plugin_view_has_size() {
        let view = PluginView::new(Box::new(TestEditor));
        assert_eq!(view.size.get().width, 400.0);
    }
}
