//! A real `drawRect:`-based `NSView` subclass for rendering `PaintCommand`
//! lists via `CoreGraphicsRenderBackend`.
//!
//! Earlier versions of this crate drew directly into a host-provided view
//! using `lockFocus`/`unlockFocus` from `idle()`, outside AppKit's normal
//! display cycle. That succeeds at the API level (valid `CGContext`, no
//! errors) but Apple has deprecated `lockFocus` since 10.14 specifically
//! because the WindowServer's Core Animation compositor is not guaranteed
//! to pick up content drawn that way -- confirmed here: real hardware
//! testing showed exactly that failure mode (every draw call succeeds,
//! nothing ever appears on screen). `drawRect:`, invoked by AppKit itself
//! in response to `setNeedsDisplay:`, is the only rendering path Apple
//! still guarantees composites correctly, so this view exists to make
//! that the only path this crate uses.
#![allow(unexpected_cfgs)]

#[cfg(not(target_os = "macos"))]
use gui_core::{CommandList, Sizef};

/// A boxed callback that turns a real OS mouse event (already converted
/// into the framework's top-left-origin coordinate space) into a widget
/// tree dispatch, returning whether some widget handled it. Owned by the
/// `GuiPaintView` (via `PaintState`) so `mouseDown:`/`mouseDragged:`/
/// `mouseUp:` can forward events synchronously as AppKit delivers them,
/// mirroring how `gui-test-host`'s Win32 window forwards `WM_LBUTTONDOWN`
/// etc.
pub type InputSink = Box<dyn FnMut(gui_core::Event) -> gui_core::EventResponse>;

#[cfg(target_os = "macos")]
mod macos {
    use std::cell::RefCell;
    use std::os::raw::c_void;
    use std::ptr;
    use std::sync::Once;

    use cocoa::base::{YES, id, nil};
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use gui_core::{
        CommandList, Event, Modifiers, MouseButton, MouseEvent, PaintCommand, PointerEvent, Pointf,
        Rectf, RenderBackend, Sizef,
    };
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};

    use super::InputSink;
    use crate::image::ImageRegistry;
    use crate::render::CoreGraphicsRenderBackend;
    use crate::text::TextRegistry;

    const VIEW_CLASS_NAME: &str = "GuiPaintView";
    const IVAR_NAME: &str = "_guiPaintState";

    /// Everything `drawRect:`/mouse handling needs, owned by the view
    /// itself so there is no risk of it outliving whatever created it.
    /// `ImageRegistry`/`TextRegistry` wrap Core Foundation/Core Graphics
    /// handles that are cheap to clone (reference-counted, not deep pixel
    /// copies), so refreshing this each frame is inexpensive.
    struct PaintState {
        commands: Vec<PaintCommand>,
        size: Sizef,
        scale: f32,
        images: Option<ImageRegistry>,
        texts: Option<TextRegistry>,
        input_sink: Option<InputSink>,
    }

    extern "C" fn draw_rect(this: &Object, _cmd: Sel, _dirty_rect: NSRect) {
        unsafe {
            let state_ptr: *mut c_void = *this.get_ivar(IVAR_NAME);
            if state_ptr.is_null() {
                return;
            }
            let cell = &*(state_ptr as *const RefCell<PaintState>);
            let state = cell.borrow();

            let Some(context_class) = Class::get("NSGraphicsContext") else {
                return;
            };
            let ns_context: *mut Object = msg_send![context_class, currentContext];
            if ns_context.is_null() {
                return;
            }
            let cg_ptr: *mut core_graphics::sys::CGContext = msg_send![ns_context, CGContext];
            if cg_ptr.is_null() {
                return;
            }
            let cg_context = core_graphics::context::CGContext::from_existing_context_ptr(cg_ptr);

            let mut commands = CommandList::with_capacity(state.commands.len());
            for cmd in &state.commands {
                commands.push(*cmd);
            }

            let mut backend = CoreGraphicsRenderBackend::with_registries(
                cg_context,
                Rectf::default(),
                state.scale,
                state.images.as_ref(),
                state.texts.as_ref(),
            );
            backend.begin(state.size);
            backend.replay(&commands);
            backend.end();
        }
    }

    extern "C" fn is_flipped(_this: &Object, _cmd: Sel) -> bool {
        // Keep AppKit's default (bottom-left) view coordinate system; our
        // `CoreGraphicsRenderBackend::begin` already flips to a top-left
        // origin internally for command replay.
        false
    }

    /// Converts an `NSEvent`'s window-space location into this view's
    /// local space, then flips it into the framework's top-left-origin
    /// space (the view itself stays bottom-left/unflipped per
    /// `is_flipped`, matching `CoreGraphicsRenderBackend::begin`'s own
    /// flip for painting).
    unsafe fn event_position(this: &Object, event: id, view_height: f32) -> Pointf {
        unsafe {
            let window_point: NSPoint = msg_send![event, locationInWindow];
            let local: NSPoint = msg_send![this, convertPoint:window_point fromView:nil];
            Pointf::new(local.x as f32, view_height - local.y as f32)
        }
    }

    extern "C" fn mouse_down(this: &Object, _cmd: Sel, event: id) {
        unsafe {
            let state_ptr: *mut c_void = *this.get_ivar(IVAR_NAME);
            if state_ptr.is_null() {
                return;
            }
            let cell = &*(state_ptr as *const RefCell<PaintState>);
            let size = cell.borrow().size;
            let position = event_position(this, event, size.height);
            let click_count: isize = msg_send![event, clickCount];

            let mut state = cell.borrow_mut();
            if let Some(sink) = state.input_sink.as_mut() {
                let _ = sink(Event::MouseDown(MouseEvent {
                    button: MouseButton::Left,
                    position,
                    modifiers: Modifiers::default(),
                    click_count: click_count.max(1) as u32,
                }));
            }
        }
    }

    extern "C" fn mouse_dragged(this: &Object, _cmd: Sel, event: id) {
        unsafe {
            let state_ptr: *mut c_void = *this.get_ivar(IVAR_NAME);
            if state_ptr.is_null() {
                return;
            }
            let cell = &*(state_ptr as *const RefCell<PaintState>);
            let size = cell.borrow().size;
            let position = event_position(this, event, size.height);

            let mut state = cell.borrow_mut();
            if let Some(sink) = state.input_sink.as_mut() {
                let _ = sink(Event::MouseMove(PointerEvent {
                    position,
                    modifiers: Modifiers::default(),
                }));
            }
        }
    }

    extern "C" fn mouse_up(this: &Object, _cmd: Sel, event: id) {
        unsafe {
            let state_ptr: *mut c_void = *this.get_ivar(IVAR_NAME);
            if state_ptr.is_null() {
                return;
            }
            let cell = &*(state_ptr as *const RefCell<PaintState>);
            let size = cell.borrow().size;
            let position = event_position(this, event, size.height);

            let mut state = cell.borrow_mut();
            if let Some(sink) = state.input_sink.as_mut() {
                let _ = sink(Event::MouseUp(MouseEvent {
                    button: MouseButton::Left,
                    position,
                    modifiers: Modifiers::default(),
                    click_count: 1,
                }));
            }
        }
    }

    extern "C" fn dealloc(this: &mut Object, _cmd: Sel) {
        unsafe {
            let state_ptr: *mut c_void = *this.get_ivar(IVAR_NAME);
            if !state_ptr.is_null() {
                let _ = Box::from_raw(state_ptr as *mut RefCell<PaintState>);
            }
            let _: () = msg_send![super(this, class!(NSView)), dealloc];
        }
    }

    static REGISTER_CLASS: Once = Once::new();
    static mut VIEW_CLASS: *const Class = ptr::null();

    fn paint_view_class() -> &'static Class {
        unsafe {
            REGISTER_CLASS.call_once(|| {
                let superclass = class!(NSView);
                let mut decl = ClassDecl::new(VIEW_CLASS_NAME, superclass)
                    .expect("GuiPaintView class should register exactly once");
                decl.add_ivar::<*mut c_void>(IVAR_NAME);
                decl.add_method(
                    sel!(drawRect:),
                    draw_rect as extern "C" fn(&Object, Sel, NSRect),
                );
                decl.add_method(
                    sel!(isFlipped),
                    is_flipped as extern "C" fn(&Object, Sel) -> bool,
                );
                decl.add_method(
                    sel!(mouseDown:),
                    mouse_down as extern "C" fn(&Object, Sel, id),
                );
                decl.add_method(
                    sel!(mouseDragged:),
                    mouse_dragged as extern "C" fn(&Object, Sel, id),
                );
                decl.add_method(sel!(mouseUp:), mouse_up as extern "C" fn(&Object, Sel, id));
                decl.add_method(sel!(dealloc), dealloc as extern "C" fn(&mut Object, Sel));
                VIEW_CLASS = decl.register();
            });
            &*VIEW_CLASS
        }
    }

    /// Creates a `GuiPaintView` sized to `size` and adds it as a subview of
    /// `parent` (a host-provided or test-host-created `NSView*`),
    /// returning the new child view's raw pointer. All future rendering
    /// (`update_paint_view`) and resizing (`resize_paint_view`) should
    /// target the returned pointer, not `parent`.
    pub fn attach_paint_view(parent: *mut c_void, size: Sizef) -> Option<*mut c_void> {
        if parent.is_null() {
            return None;
        }
        unsafe {
            let frame = NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(size.width as f64, size.height as f64),
            );
            let cls = paint_view_class();
            let view: id = msg_send![cls, alloc];
            let view: id = msg_send![view, initWithFrame: frame];
            if view == nil {
                return None;
            }

            let state = Box::new(RefCell::new(PaintState {
                commands: Vec::new(),
                size,
                scale: 1.0,
                images: None,
                texts: None,
                input_sink: None,
            }));
            let state_ptr = Box::into_raw(state) as *mut c_void;
            let view_ref: &mut Object = &mut *view;
            view_ref.set_ivar::<*mut c_void>(IVAR_NAME, state_ptr);

            // `NSViewWidthSizable | NSViewHeightSizable` keeps the child
            // filling the parent across parent resizes without needing a
            // manual observer.
            let _: () = msg_send![view, setAutoresizingMask: 18u64];

            let parent = parent as id;
            let _: () = msg_send![parent, addSubview: view];
            let _: () = msg_send![view, release];

            Some(view as *mut c_void)
        }
    }

    /// Updates the view's stored paint state and marks it dirty; the
    /// actual CoreGraphics replay happens later, inside `drawRect:`,
    /// whenever AppKit next processes the display cycle for this view.
    pub fn update_paint_view(
        view: *mut c_void,
        size: Sizef,
        scale: f32,
        commands: &CommandList,
        images: Option<&ImageRegistry>,
        texts: Option<&TextRegistry>,
    ) -> Option<()> {
        if view.is_null() {
            return None;
        }
        unsafe {
            let view = view as *mut Object;
            let state_ptr: *mut c_void = *(&*view).get_ivar(IVAR_NAME);
            if state_ptr.is_null() {
                return None;
            }
            let cell = &*(state_ptr as *const RefCell<PaintState>);
            {
                let mut state = cell.borrow_mut();
                state.commands.clear();
                state.commands.extend(commands.iter().copied());
                state.size = size;
                state.scale = scale;
                state.images = images.cloned();
                state.texts = texts.cloned();
            }
            let _: () = msg_send![view, setNeedsDisplay: YES];
        }
        Some(())
    }

    /// Installs the callback that `mouseDown:`/`mouseDragged:`/`mouseUp:`
    /// forward converted events to. Replaces any previously installed sink.
    pub fn set_input_sink(view: *mut c_void, sink: InputSink) -> Option<()> {
        if view.is_null() {
            return None;
        }
        unsafe {
            let view = view as *mut Object;
            let state_ptr: *mut c_void = *(&*view).get_ivar(IVAR_NAME);
            if state_ptr.is_null() {
                return None;
            }
            let cell = &*(state_ptr as *const RefCell<PaintState>);
            cell.borrow_mut().input_sink = Some(sink);
        }
        Some(())
    }

    /// Drops the installed input sink, if any, so `mouseDown:`/
    /// `mouseDragged:`/`mouseUp:` stop calling back into whatever
    /// constructed it. Callers must call this before the sink's captured
    /// state (e.g. the owning editor) is dropped, since the sink may hold a
    /// raw pointer back to it.
    pub fn clear_input_sink(view: *mut c_void) -> Option<()> {
        if view.is_null() {
            return None;
        }
        unsafe {
            let view = view as *mut Object;
            let state_ptr: *mut c_void = *(&*view).get_ivar(IVAR_NAME);
            if state_ptr.is_null() {
                return None;
            }
            let cell = &*(state_ptr as *const RefCell<PaintState>);
            cell.borrow_mut().input_sink = None;
        }
        Some(())
    }

    /// Resizes the child paint view's frame to match a new parent/editor
    /// size.
    pub fn resize_paint_view(view: *mut c_void, size: Sizef) -> Option<()> {
        if view.is_null() {
            return None;
        }
        unsafe {
            let view = view as id;
            let frame = NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(size.width as f64, size.height as f64),
            );
            let _: () = msg_send![view, setFrame: frame];
        }
        Some(())
    }
}

#[cfg(target_os = "macos")]
pub use macos::{
    attach_paint_view, clear_input_sink, resize_paint_view, set_input_sink, update_paint_view,
};

#[cfg(not(target_os = "macos"))]
pub fn attach_paint_view(
    _parent: *mut core::ffi::c_void,
    _size: Sizef,
) -> Option<*mut core::ffi::c_void> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn set_input_sink(_view: *mut core::ffi::c_void, _sink: InputSink) -> Option<()> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn clear_input_sink(_view: *mut core::ffi::c_void) -> Option<()> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn update_paint_view(
    _view: *mut core::ffi::c_void,
    _size: Sizef,
    _scale: f32,
    _commands: &CommandList,
    _images: Option<&crate::ImageRegistry>,
    _texts: Option<&crate::TextRegistry>,
) -> Option<()> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn resize_paint_view(_view: *mut core::ffi::c_void, _size: Sizef) -> Option<()> {
    None
}
