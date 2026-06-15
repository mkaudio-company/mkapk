#![allow(deprecated, unexpected_cfgs)]

use core::ffi::c_void;

use cocoa::appkit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreBuffered, NSEventMask, NSView,
    NSWindow, NSWindowStyleMask,
};
use cocoa::base::{NO, YES, id, nil};
use cocoa::foundation::{NSDefaultRunLoopMode, NSPoint, NSRect, NSSize};
use gui_host::ParentWindowHandle;
use objc::runtime::Class;

pub struct PlatformWindow {
    window: id,
}

pub fn create_host_window(width: u32, height: u32) -> (PlatformWindow, ParentWindowHandle) {
    unsafe {
        let app = cocoa::appkit::NSApp();
        let _ = app.setActivationPolicy_(
            NSApplicationActivationPolicy::NSApplicationActivationPolicyRegular,
        );
        app.activateIgnoringOtherApps_(YES);

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

        window.cascadeTopLeftFromPoint_(NSPoint::new(20.0, 20.0));
        window.makeKeyAndOrderFront_(nil);

        let view = NSView::alloc(nil).initWithFrame_(rect);
        window.setContentView_(view);
        let _: () = msg_send![view, release];

        let handle = ParentWindowHandle::Mac(view as *mut c_void);
        (PlatformWindow { window }, handle)
    }
}

impl PlatformWindow {
    pub fn pump_events(&self) -> bool {
        unsafe {
            let pool: id = msg_send![
                Class::get("NSAutoreleasePool").expect("NSAutoreleasePool"),
                new
            ];
            let distant_past: id = msg_send![Class::get("NSDate").expect("NSDate"), distantPast];
            let mode = NSDefaultRunLoopMode;

            let app = cocoa::appkit::NSApp();
            loop {
                let event = app.nextEventMatchingMask_untilDate_inMode_dequeue_(
                    NSEventMask::NSAnyEventMask.bits(),
                    distant_past,
                    mode,
                    YES,
                );
                if event == nil {
                    break;
                }
                app.sendEvent_(event);
            }

            let visible = self.window.isVisible() == YES;
            let _: () = msg_send![pool, release];
            visible
        }
    }

    pub fn destroy(self) {
        unsafe {
            let _: () = msg_send![self.window, close];
            let _: () = msg_send![self.window, release];
        }
    }
}
