#![allow(unexpected_cfgs, deprecated)]

use gui_core::{GpuContext, MetalContext, Sizef};

#[cfg(target_os = "macos")]
use objc::runtime::{Object, YES};

/// Render a custom GPU surface into an `NSView` using a `CAMetalLayer` backing.
///
/// This configures the view with a `CAMetalLayer`, creates the default Metal
/// device and a command queue, then calls `callback` with a
/// [`GpuContext::Metal`]. The callback is responsible for encoding and
/// committing its own command buffer; a convenience helper
/// [`clear_to_color`] is provided for simple clears.
#[cfg(target_os = "macos")]
pub fn render_gpu_surface_to_view(
    view: *mut core::ffi::c_void,
    _size: Sizef,
    callback: &mut dyn FnMut(&mut GpuContext),
) -> Option<()> {
    if view.is_null() {
        return None;
    }

    unsafe {
        let view = view as *mut Object;
        let _: () = msg_send![view, setWantsLayer: YES];

        let metal_layer_class = objc::runtime::Class::get("CAMetalLayer")?;
        let metal_layer: *mut Object = msg_send![metal_layer_class, layer];
        if metal_layer.is_null() {
            return None;
        }
        // `+layer` returns an autoreleased object; retain it for the callback.
        let metal_layer: *mut Object = msg_send![metal_layer, retain];

        let device = mtl_create_system_default_device()?;
        if device.is_null() {
            return None;
        }

        let queue: *mut Object = msg_send![device, newCommandQueue];
        if queue.is_null() {
            return None;
        }

        let _: () = msg_send![metal_layer, setDevice: device];
        let _: () = msg_send![view, setLayer: metal_layer];

        let mut ctx = GpuContext::Metal(MetalContext {
            device: device as *mut core::ffi::c_void,
            command_queue: queue as *mut core::ffi::c_void,
            layer: metal_layer as *mut core::ffi::c_void,
        });

        callback(&mut ctx);

        Some(())
    }
}

/// Convenience helper that clears the Metal surface to the given color.
///
/// This creates a render pass descriptor that clears the next drawable to
/// `(red, green, blue, alpha)`, then presents and commits the command buffer.
#[cfg(target_os = "macos")]
pub fn clear_to_color(context: &MetalContext, red: f64, green: f64, blue: f64, alpha: f64) {
    unsafe {
        let device = context.device as *mut Object;
        let queue = context.command_queue as *mut Object;
        let layer = context.layer as *mut Object;
        if device.is_null() || queue.is_null() || layer.is_null() {
            return;
        }

        let command_buffer: *mut Object = msg_send![queue, commandBuffer];
        let drawable: *mut Object = msg_send![layer, nextDrawable];
        if drawable.is_null() {
            return;
        }

        let desc_class = objc::runtime::Class::get("MTLRenderPassDescriptor").unwrap();
        let desc: *mut Object = msg_send![desc_class, renderPassDescriptor];
        let attachments: *mut Object = msg_send![desc, colorAttachments];
        let attachment: *mut Object = msg_send![attachments, objectAtIndexedSubscript: 0usize];

        let texture: *mut Object = msg_send![drawable, texture];
        let _: () = msg_send![attachment, setTexture: texture];
        let _: () = msg_send![attachment, setLoadAction: 2u64]; // MTLLoadActionClear
        let _: () = msg_send![attachment, setStoreAction: 1u64]; // MTLStoreActionStore

        #[repr(C)]
        struct MTLClearColor {
            red: f64,
            green: f64,
            blue: f64,
            alpha: f64,
        }
        let color = MTLClearColor {
            red,
            green,
            blue,
            alpha,
        };
        let _: () = msg_send![attachment, setClearColor: color];

        let encoder: *mut Object =
            msg_send![command_buffer, renderCommandEncoderWithDescriptor: desc];
        let _: () = msg_send![encoder, endEncoding];
        let _: () = msg_send![command_buffer, presentDrawable: drawable];
        let _: () = msg_send![command_buffer, commit];
    }
}

#[cfg(target_os = "macos")]
unsafe fn mtl_create_system_default_device() -> Option<*mut Object> {
    #[link(name = "Metal", kind = "framework")]
    unsafe extern "C" {
        fn MTLCreateSystemDefaultDevice() -> *mut Object;
    }
    let device = unsafe { MTLCreateSystemDefaultDevice() };
    if device.is_null() { None } else { Some(device) }
}

#[cfg(not(target_os = "macos"))]
pub fn render_gpu_surface_to_view(
    _view: *mut core::ffi::c_void,
    _size: Sizef,
    _callback: &mut dyn FnMut(&mut GpuContext),
) -> Option<()> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn clear_to_color(_context: &MetalContext, _red: f64, _green: f64, _blue: f64, _alpha: f64) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_gpu_surface_to_view_exports() {
        let _ = core::mem::size_of::<MetalContext>();
    }
}
