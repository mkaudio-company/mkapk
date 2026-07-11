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

/// Name of the `NSObject` subclass registered as the `AUCocoaUIBase`
/// factory (see `mod macos`'s `ui_view_for_audio_unit_with_size`), reported
/// to real hosts via `kAudioUnitProperty_CocoaUI` (`component.rs`'s
/// `build_cocoa_view_info`). Distinct from `GuiAuView` (`VIEW_CLASS_NAME`
/// below), which is the actual content view the factory creates and
/// returns -- the factory object itself is never shown.
#[cfg(target_os = "macos")]
pub(crate) const FACTORY_CLASS_NAME: &str = "GuiAuViewFactory";

#[cfg(target_os = "macos")]
mod macos {
    #![allow(deprecated)]

    use super::*;
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use gui_core::Sizef;
    use gui_host::{EditorHost, NormalizedValue, ParameterId, ParentWindowHandle};
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Protocol, Sel};
    use objc::{class, msg_send, sel};
    use std::cell::RefCell;
    use std::mem;
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

    // `AudioUnitGetProperty` is exported by the AudioToolbox framework
    // itself (this is the *real*, host-callable entry point -- calling it
    // routes through the Component Manager to whichever AU instance
    // `audio_unit` names, which in turn calls back into our own
    // `component::au_get_property`). Explicit linking is required under
    // macOS's default two-level-namespace dylib linking, even though the
    // host process will already have AudioToolbox loaded.
    #[link(name = "AudioToolbox", kind = "framework")]
    unsafe extern "C" {
        fn AudioUnitGetProperty(
            in_unit: au_sys::AudioUnit,
            in_id: au_sys::AudioUnitPropertyID,
            in_scope: au_sys::AudioUnitScope,
            in_element: au_sys::AudioUnitElement,
            out_data: *mut c_void,
            io_data_size: *mut au_sys::UInt32,
        ) -> au_sys::OSStatus;
    }

    /// The `AUCocoaUIBase` factory method a real host calls (after
    /// instantiating `GuiAuViewFactory` via `NSClassFromString`) to obtain
    /// the plugin's custom UI. Recovers this AU instance's Rust-side state
    /// from the opaque `audio_unit` handle via a private property (see
    /// `component::K_GUI_HOST_STATE_PROPERTY`), builds the `PluginEditor`
    /// from its stored factory closure, and hands off to the same
    /// `create_au_view` the direct-from-Rust path (`AuEditor::create_view`)
    /// already uses.
    // `objc::Encode` isn't implemented for `au_sys::AudioUnit`'s named
    // opaque pointee type, only for generic pointers, so the registered
    // method signature uses `*mut c_void` (identical ABI) and casts to
    // `au_sys::AudioUnit` before calling back into AudioToolbox.
    extern "C" fn ui_view_for_audio_unit_with_size(
        _this: &Object,
        _cmd: Sel,
        audio_unit: *mut c_void,
        preferred_size: NSSize,
    ) -> *mut Object {
        unsafe {
            let mut instance_ptr: *mut c_void = ptr::null_mut();
            let mut size: au_sys::UInt32 = mem::size_of::<*mut c_void>() as u32;
            let status = AudioUnitGetProperty(
                audio_unit as au_sys::AudioUnit,
                crate::component::K_GUI_HOST_STATE_PROPERTY,
                au_sys::kAudioUnitScope_Global,
                0,
                &mut instance_ptr as *mut *mut c_void as *mut c_void,
                &mut size,
            );
            if status != au_sys::noErr || instance_ptr.is_null() {
                return nil;
            }
            let Some(editor) = crate::component::build_editor(instance_ptr) else {
                return nil;
            };
            create_au_view(editor, preferred_size.width, preferred_size.height) as *mut Object
        }
    }

    static REGISTER_FACTORY_CLASS: Once = Once::new();
    static mut FACTORY_CLASS: *const Class = ptr::null();

    fn factory_class() -> &'static Class {
        unsafe {
            REGISTER_FACTORY_CLASS.call_once(|| {
                let superclass = class!(NSObject);
                let mut decl = ClassDecl::new(super::FACTORY_CLASS_NAME, superclass)
                    .expect("GuiAuViewFactory class should register exactly once");
                let imp: extern "C" fn(&Object, Sel, *mut c_void, NSSize) -> *mut Object =
                    ui_view_for_audio_unit_with_size;
                decl.add_method(sel!(uiViewForAudioUnit:withSize:), imp);
                // `AUCocoaUIBase` is an informal protocol (hosts are meant
                // to check `respondsToSelector:`), but some hosts also
                // check `conformsToProtocol:`; declare it when the runtime
                // already knows about it (it will, once AudioUnit-related
                // frameworks are loaded, which every real AU host does
                // before loading us).
                if let Some(proto) = Protocol::get("AUCocoaUIBase") {
                    decl.add_protocol(proto);
                }
                FACTORY_CLASS = decl.register();
            });
            &*FACTORY_CLASS
        }
    }

    /// Registers the `AUCocoaUIBase` factory class ahead of time. Not
    /// strictly required (the class also lazily registers itself the first
    /// time `kAudioUnitProperty_CocoaUI` is queried), but calling this once
    /// during plugin load avoids doing class registration work on whatever
    /// thread first happens to query that property.
    pub fn ensure_factory_class_registered() {
        let _ = factory_class();
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
pub use macos::{create_au_view, ensure_factory_class_registered, get_cocoa_view_info};

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
    use gui_core::Sizef;
    use gui_host::{
        EditorHost, NormalizedValue, ParameterId, ParentWindowHandle, PluginEditor, SizeConstraints,
    };

    struct DummyEditor;

    impl PluginEditor for DummyEditor {
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
    fn au_editor_exports() {
        let _ = std::mem::size_of::<AuEditor>();
        let _ = std::mem::size_of_val(&AuEditor::new(Box::new(DummyEditor)));
    }

    #[test]
    fn slider_mouse_down_invokes_parameter_callback() {
        use gui_core::{
            Event, EventDispatcher, EventResponse, LayoutConstraints, LayoutEngine, LayoutNode,
            Modifiers, MouseButton, MouseEvent, Pointf, Rectf, Tree, Widget,
        };
        use gui_widgets::{Slider, Theme};
        use std::cell::RefCell;
        use std::rc::Rc;

        let slider = Slider::new(ParameterId(1), NormalizedValue::new(0.0), Theme::default());
        let slider_id = slider.id();

        let values: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
        let values_for_cb = values.clone();
        slider.on_changed(move |_id, value| {
            values_for_cb.borrow_mut().push(value.get());
        });

        let mut tree = Tree::new();
        tree.insert(Box::new(slider), None);

        let mut engine = LayoutEngine::new();
        engine.set_node(LayoutNode {
            id: slider_id,
            ..LayoutNode::default()
        });
        let layout = engine.compute(
            &tree,
            LayoutConstraints {
                min_width: Some(100.0),
                max_width: Some(100.0),
                min_height: Some(24.0),
                max_height: Some(24.0),
            },
        );

        let layout_box = layout.get(slider_id).unwrap();
        let slider_node = tree.find(slider_id).unwrap();
        let widget = slider_node.widget.borrow();
        let slider_ref = gui_core::downcast_widget_ref::<gui_widgets::Slider>(&**widget).unwrap();
        slider_ref.set_frame(Rectf::new(layout_box.origin, layout_box.size));
        drop(widget);

        let mut dispatcher = EventDispatcher::new(&tree, &layout);
        let response = dispatcher.dispatch(Event::MouseDown(MouseEvent {
            button: MouseButton::Left,
            position: Pointf::new(50.0, 12.0),
            modifiers: Modifiers::default(),
            click_count: 1,
        }));

        assert_eq!(response, EventResponse::Handled);
        let captured = values.borrow();
        assert_eq!(captured.len(), 1);
        assert!(captured[0] > 0.4 && captured[0] < 0.6);
    }
}
