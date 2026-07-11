#![allow(unsafe_op_in_unsafe_fn, deprecated)]

use mkapk_host::{EditorHost, ParentWindowHandle, PluginEditor};

#[cfg(target_os = "macos")]
pub struct MacWindow {
    view: *mut core::ffi::c_void,
    window: Option<*mut core::ffi::c_void>,
}

#[cfg(not(target_os = "macos"))]
pub struct MacWindow {
    _private: (),
}

#[cfg(target_os = "macos")]
struct EditorWindowState {
    editor: Box<dyn PluginEditor>,
    host: Box<dyn EditorHost>,
    last_size: mkapk_core::Sizef,
}

#[cfg(target_os = "macos")]
impl MacWindow {
    pub fn create(
        parent: ParentWindowHandle,
        width: u32,
        height: u32,
        editor: Box<dyn PluginEditor>,
        host: Box<dyn EditorHost>,
    ) -> Option<Self> {
        unsafe { create_mac(parent, width, height, editor, host) }
    }

    pub fn request_repaint(&self) {
        unsafe {
            let view = self.view as id;
            let _: () = msg_send![view, setNeedsDisplay: YES];
        }
    }

    pub fn backing_scale(&self) -> f32 {
        unsafe {
            let view = self.view as id;
            let window: id = msg_send![view, window];
            if window.is_null() {
                1.0
            } else {
                let scale: f64 = msg_send![window, backingScaleFactor];
                scale as f32
            }
        }
    }

    pub fn view(&self) -> *mut core::ffi::c_void {
        self.view
    }

    pub fn bounds(&self) -> mkapk_core::Rectf {
        unsafe { bounds_mac(self) }
    }
}

#[cfg(not(target_os = "macos"))]
impl MacWindow {
    pub fn create(
        _parent: ParentWindowHandle,
        _width: u32,
        _height: u32,
        _editor: Box<dyn PluginEditor>,
        _host: Box<dyn EditorHost>,
    ) -> Option<Self> {
        None
    }

    pub fn request_repaint(&self) {}

    pub fn backing_scale(&self) -> f32 {
        1.0
    }

    pub fn view(&self) -> *mut core::ffi::c_void {
        core::ptr::null_mut()
    }

    pub fn bounds(&self) -> mkapk_core::Rectf {
        mkapk_core::Rectf::default()
    }
}

#[cfg(target_os = "macos")]
impl Drop for MacWindow {
    fn drop(&mut self) {
        unsafe {
            if !self.view.is_null() {
                if let Some(state) = state_for_view(self.view as id) {
                    (*state).editor.close();
                    let _ = Box::from_raw(state);
                }
                remove_state(self.view as id);
            }

            if let Some(window) = self.window {
                let w = window as id;
                let _: () = msg_send![w, close];
                let _: () = msg_send![w, release];
            }
        }
    }
}

#[cfg(target_os = "macos")]
use cocoa::appkit::{NSBackingStoreBuffered, NSView, NSWindow, NSWindowStyleMask};
#[cfg(target_os = "macos")]
use cocoa::base::{NO, YES, id, nil};
#[cfg(target_os = "macos")]
use cocoa::foundation::{NSPoint, NSRect, NSSize};
#[cfg(target_os = "macos")]
use std::collections::HashMap;
#[cfg(target_os = "macos")]
use std::ffi::c_void;
#[cfg(target_os = "macos")]
use std::sync::{LazyLock, Mutex};

#[cfg(target_os = "macos")]
static STATE_MAP: LazyLock<Mutex<HashMap<usize, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[cfg(target_os = "macos")]
unsafe fn create_mac(
    parent: ParentWindowHandle,
    width: u32,
    height: u32,
    editor: Box<dyn PluginEditor>,
    host: Box<dyn EditorHost>,
) -> Option<MacWindow> {
    let (view, window) = match parent {
        ParentWindowHandle::Mac(ptr) => {
            if ptr.is_null() {
                let (view, window) = create_standalone_view(width, height)?;
                (view, Some(window as *mut c_void))
            } else {
                (ptr as id, None)
            }
        }
        _ => return None,
    };

    let size = mkapk_core::Sizef::new(width as f32, height as f32);
    let state = Box::into_raw(Box::new(EditorWindowState {
        editor,
        host,
        last_size: size,
    }));

    register_view_did_resize(view, state);

    let handle = ParentWindowHandle::Mac(view as *mut c_void);
    (*state).editor.open(handle, (*state).host.as_ref());

    Some(MacWindow {
        view: view as *mut c_void,
        window,
    })
}

#[cfg(target_os = "macos")]
unsafe fn create_standalone_view(width: u32, height: u32) -> Option<(id, id)> {
    let rect = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(width as f64, height as f64),
    );
    let style = NSWindowStyleMask::NSTitledWindowMask
        | NSWindowStyleMask::NSClosableWindowMask
        | NSWindowStyleMask::NSMiniaturizableWindowMask
        | NSWindowStyleMask::NSResizableWindowMask;

    let window = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
        rect,
        style,
        NSBackingStoreBuffered,
        NO,
    );
    if window.is_null() {
        return None;
    }

    window.cascadeTopLeftFromPoint_(NSPoint::new(20.0, 20.0));
    window.makeKeyAndOrderFront_(nil);

    let view = NSView::alloc(nil).initWithFrame_(rect);
    if view.is_null() {
        let _: () = msg_send![window, release];
        return None;
    }
    window.setContentView_(view);
    let _: () = msg_send![view, release];

    Some((view, window))
}

#[cfg(target_os = "macos")]
unsafe fn bounds_mac(window: &MacWindow) -> mkapk_core::Rectf {
    let view = window.view as id;
    let nsrect: NSRect = msg_send![view, bounds];

    if let Some(state) = state_for_view(view) {
        let size = mkapk_core::Sizef::new(nsrect.size.width as f32, nsrect.size.height as f32);
        if size != (*state).last_size {
            (*state).last_size = size;
            (*state).editor.resize(size);
        }
    }

    mkapk_core::Rectf::new(
        mkapk_core::Point::new(nsrect.origin.x as f32, nsrect.origin.y as f32),
        mkapk_core::Size::new(nsrect.size.width as f32, nsrect.size.height as f32),
    )
}

#[cfg(target_os = "macos")]
fn register_view_did_resize(view: id, state: *mut EditorWindowState) {
    STATE_MAP
        .lock()
        .unwrap()
        .insert(view as usize, state as usize);
}

#[cfg(target_os = "macos")]
fn state_for_view(view: id) -> Option<*mut EditorWindowState> {
    STATE_MAP
        .lock()
        .unwrap()
        .get(&(view as usize))
        .copied()
        .map(|p| p as *mut EditorWindowState)
}

#[cfg(target_os = "macos")]
fn remove_state(view: id) {
    STATE_MAP.lock().unwrap().remove(&(view as usize));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_window_exports() {
        let _ = std::mem::size_of::<MacWindow>();
    }
}
