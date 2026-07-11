//! Creates the standalone app's main window via `mkgraphic` (so the window
//! chrome -- the actual `NSWindow` -- comes from mkgraphic), while the
//! plugin content view attached inside it is created and rendered by this
//! project's own CoreGraphics pipeline (`gui-mac`), not mkgraphic's own
//! tiny-skia element tree. The two libraries' Objective-C bindings
//! (`objc2` for mkgraphic, the legacy `objc`/`cocoa` crates used elsewhere
//! in this project) both just message the same underlying, single
//! `NSApplication`/`NSWindow` objects, so this is safe to mix sequentially.
//!
//! Only implemented for macOS: mkgraphic's cross-platform `Window`/`App`
//! facade doesn't wire up Windows/Linux yet (the platform-specific code
//! exists in mkgraphic but isn't connected to the public API there yet).
#![cfg(target_os = "macos")]
#![allow(unsafe_code)]

use core::ffi::c_void;

use cocoa::appkit::{NSApplication, NSEventMask, NSView};
use cocoa::base::{BOOL, NO, YES, id, nil};
use cocoa::foundation::{NSDefaultRunLoopMode, NSPoint, NSRect, NSSize};
use gui_host::ParentWindowHandle;
use mkgraphic::prelude::{App, Extent, Window};
use objc::runtime::Class;

/// Owns the mkgraphic-created window (and the mkgraphic `App`, kept alive
/// only for its constructor side effects: activation policy and menu bar
/// setup) for the lifetime of the standalone app.
pub struct MainWindow {
    #[allow(dead_code)]
    app: Option<App>,
    #[allow(dead_code)]
    window: Window,
    ns_window: id,
}

impl MainWindow {
    /// Creates the window and a plain `NSView` as its content view, sized
    /// to fill it. Returns a handle to that view for `PluginEditor::open`.
    pub fn create(title: &str, width: u32, height: u32) -> (Self, ParentWindowHandle) {
        let app = App::new();

        let mut window = Window::new(title, Extent::new(width as f32, height as f32));
        let raw_window = window
            .handle()
            .expect("mkgraphic window should expose a native NSWindow handle on macOS");
        let ns_window = raw_window as id;

        let frame = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(width as f64, height as f64),
        );

        let content_view: id = unsafe {
            let view = NSView::alloc(nil).initWithFrame_(frame);
            let _: () = msg_send![ns_window, setContentView: view];
            let _: () = msg_send![view, release];
            view
        };

        window.show();

        (
            Self {
                app: Some(app),
                window,
                ns_window,
            },
            ParentWindowHandle::Mac(content_view as *mut c_void),
        )
    }

    /// Pumps the shared `NSApplication` event queue (the same singleton
    /// mkgraphic's own window belongs to) and reports whether this window
    /// is still visible. Mirrors `gui-test-host`'s `PlatformWindow::pump_events`.
    pub fn pump_events(&self) -> bool {
        unsafe {
            let pool: id = msg_send![
                Class::get("NSAutoreleasePool").expect("NSAutoreleasePool"),
                new
            ];
            let distant_past: id = msg_send![Class::get("NSDate").expect("NSDate"), distantPast];

            let app = cocoa::appkit::NSApp();
            loop {
                let event = app.nextEventMatchingMask_untilDate_inMode_dequeue_(
                    NSEventMask::NSAnyEventMask.bits(),
                    distant_past,
                    NSDefaultRunLoopMode,
                    YES,
                );
                if event == nil {
                    break;
                }
                app.sendEvent_(event);
            }

            let visible: BOOL = msg_send![self.ns_window, isVisible];
            let _: () = msg_send![pool, release];
            visible != NO
        }
    }

    pub fn destroy(self) {
        unsafe {
            let _: () = msg_send![self.ns_window, close];
        }
    }
}
