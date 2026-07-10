#![allow(unsafe_op_in_unsafe_fn)]

use std::ptr::null_mut;

use gui_core::{Event, Modifiers, MouseButton, MouseEvent, PointerEvent, Pointf};
use gui_host::ParentWindowHandle;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetStockObject, HBRUSH, WHITE_BRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, GWLP_USERDATA, GetWindowLongPtrW, HCURSOR, HICON, HMENU, MSG, PM_REMOVE,
    PeekMessageW, RegisterClassExW, SetWindowLongPtrW, TranslateMessage, WINDOW_EX_STYLE,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCDESTROY, WM_QUIT, WNDCLASS_STYLES,
    WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
};
use windows::core::PCWSTR;

/// A boxed callback that turns a real Win32 mouse message into a widget
/// tree dispatch, returning whether some widget handled it. Stored via
/// `GWLP_USERDATA` on the host window (double-boxed so the raw pointer
/// stashed there stays a single machine word, since `Box<dyn FnMut(..)>`
/// alone is a fat pointer and won't fit).
pub type InputSink = Box<dyn FnMut(Event) -> gui_core::EventResponse>;

pub struct PlatformWindow {
    hwnd: HWND,
    _hinstance: HINSTANCE,
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_MOUSEMOVE => {
            let sink_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut InputSink;
            if !sink_ptr.is_null() {
                let x = (lparam.0 as i32) & 0xFFFF;
                let y = ((lparam.0 >> 16) as i32) & 0xFFFF;
                let position = Pointf::new(x as f32, y as f32);
                let event = match msg {
                    WM_LBUTTONDOWN => Event::MouseDown(MouseEvent {
                        button: MouseButton::Left,
                        position,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                    }),
                    WM_LBUTTONUP => Event::MouseUp(MouseEvent {
                        button: MouseButton::Left,
                        position,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                    }),
                    _ => Event::MouseMove(PointerEvent {
                        position,
                        modifiers: Modifiers::default(),
                    }),
                };
                let sink = &mut *sink_ptr;
                let _ = sink(event);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_NCDESTROY => {
            let sink_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut InputSink;
            if !sink_ptr.is_null() {
                let _ = Box::from_raw(sink_ptr);
                let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn create_host_window(width: u32, height: u32) -> (PlatformWindow, ParentWindowHandle) {
    unsafe {
        let instance = GetModuleHandleW(None).expect("GetModuleHandleW failed");
        let hinstance = HINSTANCE(instance.0);

        let class_name = windows::core::HSTRING::from("GuiTestHostWindow");
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: WNDCLASS_STYLES(0),
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: HICON(null_mut()),
            hCursor: HCURSOR(null_mut()),
            hbrBackground: HBRUSH(GetStockObject(WHITE_BRUSH).0),
            lpszMenuName: PCWSTR(null_mut()),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            hIconSm: HICON(null_mut()),
        };
        let _atom = RegisterClassExW(&class);

        let mut rect = windows::Win32::Foundation::RECT {
            left: 0,
            top: 0,
            right: width as i32,
            bottom: height as i32,
        };
        let _ = AdjustWindowRectEx(&mut rect, WS_OVERLAPPEDWINDOW, false, WINDOW_EX_STYLE(0));

        let title = windows::core::HSTRING::from("GUI Test Host");
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            rect.right - rect.left,
            rect.bottom - rect.top,
            Some(HWND(null_mut())),
            Some(HMENU(null_mut())),
            Some(hinstance),
            None,
        )
        .expect("CreateWindowExW failed");

        let handle = ParentWindowHandle::Windows(hwnd.0);
        (
            PlatformWindow {
                hwnd,
                _hinstance: hinstance,
            },
            handle,
        )
    }
}

impl PlatformWindow {
    /// Installs the callback `wndproc` forwards `WM_LBUTTONDOWN`/
    /// `WM_LBUTTONUP`/`WM_MOUSEMOVE` to. Replaces any previously installed
    /// sink (dropping it), so this is safe to call more than once.
    pub fn set_input_sink(&self, sink: InputSink) {
        unsafe {
            let previous = GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut InputSink;
            let boxed: *mut InputSink = Box::into_raw(Box::new(sink));
            let _ = SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, boxed as isize);
            if !previous.is_null() {
                let _ = Box::from_raw(previous);
            }
        }
    }

    pub fn pump_events(&self) -> bool {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, Some(HWND(null_mut())), 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    return false;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            true
        }
    }

    pub fn destroy(self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}
