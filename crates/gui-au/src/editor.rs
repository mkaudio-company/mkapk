use core::ffi::c_void;
use std::ptr;

use gui_host::PluginEditor;

pub struct AuEditor {
    editor: Box<dyn PluginEditor>,
}

impl AuEditor {
    pub fn new(editor: Box<dyn PluginEditor>) -> Self {
        Self { editor }
    }

    pub fn create_view(self, width: f64, height: f64) -> *mut c_void {
        create_au_view(self.editor, width, height)
    }
}

#[repr(C)]
pub struct AudioComponentDescription {
    pub component_type: u32,
    pub component_sub_type: u32,
    pub component_manufacturer: u32,
    pub component_flags: u32,
    pub component_flags_mask: u32,
}

#[repr(C)]
pub struct AuCocoaViewInfo {
    pub ui_component_desc: AudioComponentDescription,
    pub view_class: *const c_void,
}

#[cfg(target_os = "macos")]
mod macos {
    #![allow(deprecated)]

    use super::*;
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use gui_core::Sizef;
    use gui_host::{EditorHost, NormalizedValue, ParameterId, ParentWindowHandle};
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel};
    use std::cell::RefCell;
    use std::sync::Once;

    const VIEW_CLASS_NAME: &str = "GuiAuView";
    const IVAR_NAME: &str = "_auState";

    struct AuEditorState {
        editor: Box<dyn PluginEditor>,
        host: Box<dyn EditorHost>,
    }

    struct AuEditorHost;

    impl EditorHost for AuEditorHost {
        fn request_resize(&self, _size: Sizef) {}
        fn start_parameter_gesture(&self, _id: ParameterId) {}
        fn end_parameter_gesture(&self, _id: ParameterId) {}
        fn set_parameter_normalized(&self, _id: ParameterId, _value: NormalizedValue) {}
    }

    thread_local! {
        static PENDING_STATE: RefCell<*mut c_void> = const { RefCell::new(ptr::null_mut()) };
    }

    fn set_pending_state(state: *mut c_void) {
        PENDING_STATE.with(|s| *s.borrow_mut() = state);
    }

    fn take_pending_state() -> *mut c_void {
        PENDING_STATE.with(|s| {
            let mut s = s.borrow_mut();
            let ptr = *s;
            *s = ptr::null_mut();
            ptr
        })
    }

    fn clear_pending_state() {
        PENDING_STATE.with(|s| *s.borrow_mut() = ptr::null_mut());
    }

    extern "C" fn init_with_frame(this: &mut Object, _cmd: Sel, frame: NSRect) -> *mut Object {
        unsafe {
            let view: *mut Object = msg_send![super(this, class!(NSView)), initWithFrame: frame];

            if view == nil {
                let state_ptr = take_pending_state();
                if !state_ptr.is_null() {
                    let _ = Box::from_raw(state_ptr as *mut AuEditorState);
                }
                return nil;
            }

            let state_ptr = take_pending_state();
            if !state_ptr.is_null() {
                let state = &mut *(state_ptr as *mut AuEditorState);
                let handle = ParentWindowHandle::Mac(view as *mut c_void);
                state.editor.open(handle, &*state.host);
                state.editor.resize(Sizef::new(
                    frame.size.width as f32,
                    frame.size.height as f32,
                ));

                let view_ref: &mut Object = &mut *view;
                view_ref.set_ivar::<*mut c_void>(IVAR_NAME, state_ptr);
            }

            view
        }
    }

    extern "C" fn set_frame_size(this: &mut Object, _cmd: Sel, size: NSSize) {
        unsafe {
            let _: () = msg_send![super(this, class!(NSView)), setFrameSize: size];

            let state_ptr: *mut c_void = *this.get_ivar(IVAR_NAME);
            if !state_ptr.is_null() {
                let state = &mut *(state_ptr as *mut AuEditorState);
                state
                    .editor
                    .resize(Sizef::new(size.width as f32, size.height as f32));
            }
        }
    }

    extern "C" fn dealloc(this: &mut Object, _cmd: Sel) {
        unsafe {
            let state_ptr: *mut c_void = *this.get_ivar(IVAR_NAME);
            if !state_ptr.is_null() {
                let state = &mut *(state_ptr as *mut AuEditorState);
                state.editor.close();
                let _ = Box::from_raw(state_ptr as *mut AuEditorState);
            }

            let _: () = msg_send![super(this, class!(NSView)), dealloc];
        }
    }

    static REGISTER_CLASS: Once = Once::new();
    static mut VIEW_CLASS: *const Class = ptr::null();

    fn au_view_class() -> &'static Class {
        unsafe {
            REGISTER_CLASS.call_once(|| {
                let superclass = class!(NSView);
                let mut decl = ClassDecl::new(VIEW_CLASS_NAME, superclass).unwrap();
                decl.add_ivar::<*mut c_void>(IVAR_NAME);
                let init_with_frame_imp: extern "C" fn(&mut Object, Sel, NSRect) -> *mut Object =
                    init_with_frame;
                let set_frame_size_imp: extern "C" fn(&mut Object, Sel, NSSize) = set_frame_size;
                let dealloc_imp: extern "C" fn(&mut Object, Sel) = dealloc;
                decl.add_method(sel!(initWithFrame:), init_with_frame_imp);
                decl.add_method(sel!(setFrameSize:), set_frame_size_imp);
                decl.add_method(sel!(dealloc), dealloc_imp);
                VIEW_CLASS = decl.register();
            });
            &*VIEW_CLASS
        }
    }

    pub fn create_au_view(editor: Box<dyn PluginEditor>, width: f64, height: f64) -> *mut c_void {
        unsafe {
            let cls = au_view_class();
            let state = Box::new(AuEditorState {
                editor,
                host: Box::new(AuEditorHost),
            });
            let state_ptr = Box::into_raw(state) as *mut c_void;

            set_pending_state(state_ptr);

            let view: id = msg_send![cls, alloc];
            let view: id = msg_send![view, initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height))];

            if view == nil {
                let _ = Box::from_raw(state_ptr as *mut AuEditorState);
                clear_pending_state();
                return ptr::null_mut();
            }

            clear_pending_state();
            view as *mut c_void
        }
    }

    pub fn get_cocoa_view_info() -> AuCocoaViewInfo {
        AuCocoaViewInfo {
            ui_component_desc: AudioComponentDescription {
                component_type: 0,
                component_sub_type: 0,
                component_manufacturer: 0,
                component_flags: 0,
                component_flags_mask: 0,
            },
            view_class: au_view_class() as *const _ as *const c_void,
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::{create_au_view, get_cocoa_view_info};

#[cfg(not(target_os = "macos"))]
pub fn create_au_view(_editor: Box<dyn PluginEditor>, _width: f64, _height: f64) -> *mut c_void {
    ptr::null_mut()
}

#[cfg(not(target_os = "macos"))]
pub fn get_cocoa_view_info() -> AuCocoaViewInfo {
    AuCocoaViewInfo {
        ui_component_desc: AudioComponentDescription {
            component_type: 0,
            component_sub_type: 0,
            component_manufacturer: 0,
            component_flags: 0,
            component_flags_mask: 0,
        },
        view_class: ptr::null(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn au_editor_exports() {
        let _ = std::mem::size_of::<AuEditor>();
    }
}
