use gui_host::{EditorHost, ParentWindowHandle, PluginEditor};

#[cfg(target_os = "windows")]
use std::ptr::null_mut;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::*;
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::*;
#[cfg(target_os = "windows")]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
#[cfg(target_os = "windows")]
use windows::Win32::UI::HiDpi::*;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::*;

#[cfg(target_os = "windows")]
struct EditorWindowState {
    editor: Box<dyn PluginEditor>,
    host: Box<dyn EditorHost>,
    size: gui_core::Sizef,
}

#[cfg(target_os = "windows")]
pub struct Win32Window {
    hwnd: HWND,
}

#[cfg(not(target_os = "windows"))]
pub struct Win32Window {
    _private: (),
}

#[cfg(target_os = "windows")]
impl Win32Window {
    pub fn create(
        parent: ParentWindowHandle,
        width: u32,
        height: u32,
        editor: Box<dyn PluginEditor>,
        host: Box<dyn EditorHost>,
    ) -> Option<Self> {
        unsafe {
            let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }

        let parent_hwnd = match parent {
            ParentWindowHandle::Windows(ptr) => HWND(ptr),
            _ => return None,
        };

        let hinstance = unsafe { GetModuleHandleW(None).ok()? };
        let class_name = windows::core::w!("GuiPluginEditorClass");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: WNDCLASS_STYLES(0),
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: HINSTANCE(hinstance.0),
            hIcon: HICON(null_mut()),
            hCursor: HCURSOR(null_mut()),
            hbrBackground: HBRUSH(null_mut()),
            lpszMenuName: windows::core::PCWSTR(null_mut()),
            lpszClassName: class_name,
            hIconSm: HICON(null_mut()),
        };

        let atom = unsafe { RegisterClassExW(&wc) };
        if atom == 0 {
            let err = unsafe { GetLastError() };
            if err != ERROR_CLASS_ALREADY_EXISTS {
                return None;
            }
        }

        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class_name,
                windows::core::w!(""),
                WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN,
                0,
                0,
                width as i32,
                height as i32,
                parent_hwnd,
                HMENU(null_mut()),
                HINSTANCE(hinstance.0),
                None,
            )
        };

        if hwnd.0.is_null() {
            return None;
        }

        let size = gui_core::Sizef::new(width as f32, height as f32);
        let state = Box::new(EditorWindowState { editor, host, size });
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
        }

        Some(Self { hwnd })
    }

    pub fn request_repaint(&self) {
        unsafe {
            let _ = InvalidateRect(self.hwnd, None, false);
            let _ = UpdateWindow(self.hwnd);
        }
    }

    pub fn client_size(&self) -> gui_core::Sizef {
        unsafe {
            let mut rect = RECT::default();
            if GetClientRect(self.hwnd, &mut rect).as_bool() {
                gui_core::Sizef::new(
                    (rect.right - rect.left) as f32,
                    (rect.bottom - rect.top) as f32,
                )
            } else {
                gui_core::Sizef::new(0.0, 0.0)
            }
        }
    }

    pub fn dpi(&self) -> u32 {
        unsafe {
            let dpi = GetDpiForWindow(self.hwnd);
            if dpi == 0 { 96 } else { dpi }
        }
    }
}

#[cfg(not(target_os = "windows"))]
impl Win32Window {
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

    pub fn client_size(&self) -> gui_core::Sizef {
        gui_core::Sizef::new(0.0, 0.0)
    }

    pub fn dpi(&self) -> u32 {
        96
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_SIZE => {
            let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut EditorWindowState;
            if !state.is_null() {
                let mut rect = RECT::default();
                if GetClientRect(hwnd, &mut rect).as_bool() {
                    let size = gui_core::Sizef::new(
                        (rect.right - rect.left) as f32,
                        (rect.bottom - rect.top) as f32,
                    );
                    (*state).size = size;
                    (*state).editor.resize(size);
                    let _ = InvalidateRect(hwnd, None, false);
                    let _ = UpdateWindow(hwnd);
                }
            }
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut EditorWindowState;
            if !state.is_null() {
                let _ = InvalidateRect(hwnd, None, false);
                let _ = UpdateWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let _ = ValidateRect(hwnd, None);
            LRESULT(0)
        }
        WM_DESTROY => {
            let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut EditorWindowState;
            if !state.is_null() {
                drop(Box::from_raw(state));
                let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let x = GET_X_LPARAM(lparam);
            let y = GET_Y_LPARAM(lparam);
            println!("mouse_down x={} y={}", x, y);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let x = GET_X_LPARAM(lparam);
            let y = GET_Y_LPARAM(lparam);
            println!("mouse_up x={} y={}", x, y);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let x = GET_X_LPARAM(lparam);
            let y = GET_Y_LPARAM(lparam);
            println!("mouse_move x={} y={}", x, y);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let vk = wparam.0 as u32;
            println!("key_down vk={}", vk);
            LRESULT(0)
        }
        WM_KEYUP => {
            let vk = wparam.0 as u32;
            println!("key_up vk={}", vk);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win32_window_exports() {
        let _ = std::mem::size_of::<Win32Window>();
    }
}
