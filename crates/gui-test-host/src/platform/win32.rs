#![allow(unsafe_op_in_unsafe_fn)]

use core::ffi::c_void;
use std::ptr::null_mut;

use gui_host::ParentWindowHandle;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetStockObject, HBRUSH, WHITE_BRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, HCURSOR, HICON, HMENU, MSG, PM_REMOVE, PeekMessageW, RegisterClassExW,
    TranslateMessage, WINDOW_EX_STYLE, WM_QUIT, WNDCLASS_STYLES, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
};
use windows::core::PCWSTR;

pub struct PlatformWindow {
    hwnd: HWND,
    _hinstance: HINSTANCE,
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
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

        let handle = ParentWindowHandle::Windows(hwnd.0 as *mut c_void);
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
