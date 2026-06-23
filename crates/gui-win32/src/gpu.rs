#![allow(unexpected_cfgs)]

use gui_core::{D3D11Context, GpuContext, Sizef};

#[cfg(target_os = "windows")]
use windows::{
    Win32::Foundation::HWND,
    Win32::Graphics::Direct3D11::{ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11Texture2D},
    Win32::Graphics::Dxgi::{DXGI_PRESENT, IDXGISwapChain1},
    core::Interface,
};

#[cfg(target_os = "windows")]
use crate::render::{create_d3d_device, create_swap_chain};

/// Render a custom GPU surface into an `HWND` using D3D11 + DXGI.
///
/// This creates a fresh D3D11 device and swap chain each call. The supplied
/// callback receives a [`GpuContext::D3D11`] with the device, immediate
/// context, and render target view. After the callback returns the swap chain
/// is presented.
#[cfg(target_os = "windows")]
pub fn render_gpu_surface_to_hwnd(
    hwnd: *mut core::ffi::c_void,
    size: Sizef,
    callback: &mut dyn FnMut(&mut GpuContext),
) -> Option<()> {
    if size.width <= 0.0 || size.height <= 0.0 {
        return None;
    }

    let hwnd = HWND(hwnd);

    unsafe {
        let device = create_d3d_device().ok()?;
        let swap_chain = create_swap_chain(&device, hwnd).ok()?;
        let context: ID3D11DeviceContext = device.GetImmediateContext().ok()?;
        let render_target = create_render_target(&device, &swap_chain).ok()?;

        context.OMSetRenderTargets(Some(&[Some(render_target.clone())]), None);

        let viewport = windows::Win32::Graphics::Direct3D11::D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: size.width,
            Height: size.height,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        context.RSSetViewports(Some(&[viewport]));

        let mut ctx = GpuContext::D3D11(D3D11Context {
            device: device.as_raw(),
            context: context.as_raw(),
            render_target: render_target.as_raw(),
        });

        callback(&mut ctx);

        let _ = swap_chain.Present(1, DXGI_PRESENT(0));
        Some(())
    }
}

#[cfg(target_os = "windows")]
unsafe fn create_render_target(
    device: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
    swap_chain: &IDXGISwapChain1,
) -> windows::core::Result<ID3D11RenderTargetView> {
    let back_buffer = swap_chain.GetBuffer::<ID3D11Texture2D>(0)?;
    let mut render_target = None;
    device.CreateRenderTargetView(&back_buffer, None, Some(&mut render_target))?;
    render_target
        .ok_or_else(|| windows::core::Error::from(windows::core::HRESULT(0x80004005u32 as i32)))
}

/// Convenience helper that clears the D3D11 render target to `color`.
#[cfg(target_os = "windows")]
pub fn clear_render_target(context: &D3D11Context, color: [f32; 4]) {
    unsafe {
        let d3d_context = ID3D11DeviceContext::from_raw(context.context as *mut _);
        let rtv = ID3D11RenderTargetView::from_raw(context.render_target as *mut _);
        d3d_context.ClearRenderTargetView(&rtv, &color);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn render_gpu_surface_to_hwnd(
    _hwnd: *mut core::ffi::c_void,
    _size: Sizef,
    _callback: &mut dyn FnMut(&mut GpuContext),
) -> Option<()> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn clear_render_target(_context: &D3D11Context, _color: [f32; 4]) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_gpu_surface_to_hwnd_exports() {
        let _ = core::mem::size_of::<D3D11Context>();
    }
}
