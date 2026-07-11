//! Real `NSAccessibility` bridging: mirrors a `gui_core::AccessibilityTree`
//! into a tree of `NSAccessibilityElement` proxy objects and attaches it to
//! a live `NSView`, so external tools (VoiceOver, Accessibility Inspector)
//! can query widget roles/labels/values.
#![allow(unexpected_cfgs)]

#[cfg(not(target_os = "macos"))]
use gui_core::AccessibilityTree;

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;
    use std::ptr;
    use std::sync::Once;

    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
    use gui_core::{AccessibilityNode, AccessibilityTree, Rectf, Role};
    use objc::declare::ClassDecl;
    use objc::rc::StrongPtr;
    use objc::runtime::{BOOL, Class, NO, Object, Sel, YES};
    use objc::{class, msg_send, sel};

    const ELEMENT_CLASS_NAME: &str = "GuiAccessibilityElement";
    const IVAR_NAME: &str = "_a11yState";

    struct ElementState {
        role: Role,
        label: Option<String>,
        value: Option<String>,
        bounds: Rectf,
        children: Vec<StrongPtr>,
    }

    extern "C" fn dealloc(this: &mut Object, _cmd: Sel) {
        unsafe {
            let state_ptr: *mut c_void = *this.get_ivar(IVAR_NAME);
            if !state_ptr.is_null() {
                let _ = Box::from_raw(state_ptr as *mut ElementState);
            }
            let _: () = msg_send![super(this, class!(NSAccessibilityElement)), dealloc];
        }
    }

    unsafe fn state_of(this: &Object) -> &ElementState {
        unsafe { &*(*this.get_ivar::<*mut c_void>(IVAR_NAME) as *const ElementState) }
    }

    extern "C" fn accessibility_role(this: &Object, _cmd: Sel) -> id {
        let role = unsafe { state_of(this) }.role;
        role_to_nsstring(role)
    }

    extern "C" fn accessibility_label(this: &Object, _cmd: Sel) -> id {
        match &unsafe { state_of(this) }.label {
            Some(s) => unsafe { NSString::alloc(nil).init_str(s) },
            None => nil,
        }
    }

    extern "C" fn accessibility_value(this: &Object, _cmd: Sel) -> id {
        match &unsafe { state_of(this) }.value {
            Some(s) => unsafe { NSString::alloc(nil).init_str(s) },
            None => nil,
        }
    }

    extern "C" fn accessibility_children(this: &Object, _cmd: Sel) -> id {
        let state = unsafe { state_of(this) };
        unsafe {
            let array: id =
                msg_send![class!(NSMutableArray), arrayWithCapacity: state.children.len()];
            for child in &state.children {
                let _: () = msg_send![array, addObject: **child];
            }
            array
        }
    }

    extern "C" fn is_accessibility_element(_this: &Object, _cmd: Sel) -> BOOL {
        YES
    }

    extern "C" fn accessibility_frame(this: &Object, _cmd: Sel) -> NSRect {
        let bounds = unsafe { state_of(this) }.bounds;
        NSRect::new(
            NSPoint::new(bounds.origin.x as f64, bounds.origin.y as f64),
            NSSize::new(bounds.size.width as f64, bounds.size.height as f64),
        )
    }

    fn role_to_nsstring(role: Role) -> id {
        let s = match role {
            Role::Button => "AXButton",
            Role::Slider => "AXSlider",
            // AppKit has no dedicated knob role; a slider is the closest
            // semantic analogue for a bounded, continuously adjustable value.
            Role::Knob => "AXSlider",
            Role::Label | Role::Text => "AXStaticText",
            Role::Panel => "AXGroup",
            Role::None => "AXUnknown",
        };
        unsafe { NSString::alloc(nil).init_str(s) }
    }

    static REGISTER_CLASS: Once = Once::new();
    static mut ELEMENT_CLASS: *const Class = ptr::null();

    fn element_class() -> &'static Class {
        unsafe {
            REGISTER_CLASS.call_once(|| {
                let superclass = class!(NSAccessibilityElement);
                let mut decl = ClassDecl::new(ELEMENT_CLASS_NAME, superclass)
                    .expect("GuiAccessibilityElement class should register exactly once");
                decl.add_ivar::<*mut c_void>(IVAR_NAME);
                decl.add_method(sel!(dealloc), dealloc as extern "C" fn(&mut Object, Sel));
                decl.add_method(
                    sel!(accessibilityRole),
                    accessibility_role as extern "C" fn(&Object, Sel) -> id,
                );
                decl.add_method(
                    sel!(accessibilityLabel),
                    accessibility_label as extern "C" fn(&Object, Sel) -> id,
                );
                decl.add_method(
                    sel!(accessibilityValue),
                    accessibility_value as extern "C" fn(&Object, Sel) -> id,
                );
                decl.add_method(
                    sel!(accessibilityChildren),
                    accessibility_children as extern "C" fn(&Object, Sel) -> id,
                );
                decl.add_method(
                    sel!(isAccessibilityElement),
                    is_accessibility_element as extern "C" fn(&Object, Sel) -> BOOL,
                );
                decl.add_method(
                    sel!(accessibilityFrame),
                    accessibility_frame as extern "C" fn(&Object, Sel) -> NSRect,
                );
                ELEMENT_CLASS = decl.register();
            });
            &*ELEMENT_CLASS
        }
    }

    fn build_element(node: &AccessibilityNode) -> StrongPtr {
        let children: Vec<StrongPtr> = node.children.iter().map(build_element).collect();
        let state = Box::new(ElementState {
            role: node.role,
            label: node.label.clone(),
            value: node.value.clone(),
            bounds: node.bounds,
            children,
        });
        let state_ptr = Box::into_raw(state) as *mut c_void;

        unsafe {
            let obj: id = msg_send![element_class(), alloc];
            let obj: id = msg_send![obj, init];
            let obj_ref: &mut Object = &mut *obj;
            obj_ref.set_ivar::<*mut c_void>(IVAR_NAME, state_ptr);
            StrongPtr::new(obj)
        }
    }

    /// Owns a tree of `NSAccessibilityElement` proxies mirroring an
    /// `AccessibilityTree`. Dropping this releases the whole tree.
    pub struct AccessibilityElementHandle(StrongPtr);

    pub fn build_accessibility_tree(tree: &AccessibilityTree) -> AccessibilityElementHandle {
        AccessibilityElementHandle(build_element(tree.root()))
    }

    /// Makes `root` the sole accessibility child of `view`, so assistive
    /// tools querying the view discover the whole mirrored widget tree.
    pub fn attach_to_view(view: *mut c_void, root: &AccessibilityElementHandle) {
        if view.is_null() {
            return;
        }
        unsafe {
            let view = view as id;
            let array: id = msg_send![class!(NSMutableArray), arrayWithCapacity: 1usize];
            let _: () = msg_send![array, addObject: *root.0];
            let _: () = msg_send![view, setAccessibilityChildren: array];
            let _: () = msg_send![view, setAccessibilityElement: NO];
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use gui_core::{Pointf, Sizef, WidgetId};

        unsafe fn nsstring_to_string(s: id) -> String {
            unsafe {
                if s == nil {
                    return String::new();
                }
                let cstr: *const std::os::raw::c_char = msg_send![s, UTF8String];
                std::ffi::CStr::from_ptr(cstr)
                    .to_string_lossy()
                    .into_owned()
            }
        }

        #[test]
        fn accessibility_element_exposes_role_label_value_and_frame() {
            let node = AccessibilityNode::new(WidgetId::new())
                .with_role(Role::Slider)
                .with_label("Gain")
                .with_value("75%")
                .with_bounds(Rectf::new(Pointf::new(1.0, 2.0), Sizef::new(3.0, 4.0)));

            let handle = build_accessibility_tree(&AccessibilityTree::new(node));
            let obj = *handle.0;

            unsafe {
                let role: id = msg_send![obj, accessibilityRole];
                assert_eq!(nsstring_to_string(role), "AXSlider");

                let label: id = msg_send![obj, accessibilityLabel];
                assert_eq!(nsstring_to_string(label), "Gain");

                let value: id = msg_send![obj, accessibilityValue];
                assert_eq!(nsstring_to_string(value), "75%");

                let is_element: BOOL = msg_send![obj, isAccessibilityElement];
                assert_eq!(is_element, YES);

                let frame: NSRect = msg_send![obj, accessibilityFrame];
                assert_eq!(frame.origin.x, 1.0);
                assert_eq!(frame.origin.y, 2.0);
                assert_eq!(frame.size.width, 3.0);
                assert_eq!(frame.size.height, 4.0);
            }
        }

        #[test]
        fn accessibility_element_exposes_children() {
            let child = AccessibilityNode::new(WidgetId::new())
                .with_role(Role::Button)
                .with_label("Click");
            let root = AccessibilityNode::new(WidgetId::new())
                .with_role(Role::Panel)
                .with_child(child);

            let handle = build_accessibility_tree(&AccessibilityTree::new(root));
            let obj = *handle.0;

            unsafe {
                let root_role: id = msg_send![obj, accessibilityRole];
                assert_eq!(nsstring_to_string(root_role), "AXGroup");

                let children: id = msg_send![obj, accessibilityChildren];
                let count: usize = msg_send![children, count];
                assert_eq!(count, 1);

                let first: id = msg_send![children, objectAtIndex: 0usize];
                let role: id = msg_send![first, accessibilityRole];
                assert_eq!(nsstring_to_string(role), "AXButton");

                let label: id = msg_send![first, accessibilityLabel];
                assert_eq!(nsstring_to_string(label), "Click");
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::{AccessibilityElementHandle, attach_to_view, build_accessibility_tree};

#[cfg(not(target_os = "macos"))]
pub struct AccessibilityElementHandle;

#[cfg(not(target_os = "macos"))]
pub fn build_accessibility_tree(_tree: &AccessibilityTree) -> AccessibilityElementHandle {
    AccessibilityElementHandle
}

#[cfg(not(target_os = "macos"))]
pub fn attach_to_view(_view: *mut core::ffi::c_void, _root: &AccessibilityElementHandle) {}
