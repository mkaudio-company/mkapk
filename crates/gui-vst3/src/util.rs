//! Small string-copy helpers shared by the VST3 factory/component/controller
//! code, matching the conventions used throughout `vst3-sys`'s own examples
//! for filling fixed-size `char8`/`char16` buffers.
use std::os::raw::{c_char, c_short};
use std::ptr::copy_nonoverlapping;

/// Copies `src` (ASCII/UTF-8 bytes) into a fixed-size `char8` buffer.
pub(crate) unsafe fn strcpy(src: &str, dst: *mut c_char) {
    unsafe {
        copy_nonoverlapping(src.as_ptr() as *const c_char, dst, src.len());
        *dst.add(src.len()) = 0;
    }
}

/// Copies `src` into a fixed-size UTF-16 (`char16`) buffer, including the
/// trailing NUL terminator.
pub(crate) unsafe fn wstrcpy(src: &str, dst: *mut c_short) {
    let mut units: Vec<u16> = src.encode_utf16().collect();
    units.push(0);
    unsafe {
        copy_nonoverlapping(units.as_ptr() as *const c_short, dst, units.len());
    }
}
