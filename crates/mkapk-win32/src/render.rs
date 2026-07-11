#![allow(unexpected_cfgs)]

use gui_core::{CommandList, RenderBackend, Sizef};

#[cfg(target_os = "windows")]
use gui_core::{Color, PaintCommand, Pointf, Rectf};

#[cfg(target_os = "windows")]
use windows::{
    Win32::Foundation::{HMODULE, HWND},
    Win32::Graphics::Direct2D::{
        Common::{
            D2D_RECT_F as D2D1_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_IGNORE,
            D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED,
            D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN,
            D2D1_GRADIENT_STOP, D2D1_PIXEL_FORMAT,
        },
        D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_OPTIONS_TARGET,
        D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_NONE,
        D2D1_EXTEND_MODE_CLAMP, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_GAMMA_2_2,
        D2D1_INTERPOLATION_MODE_LINEAR, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_ROUNDED_RECT,
        D2D1CreateFactory, ID2D1DeviceContext, ID2D1Factory1, ID2D1PathGeometry1,
        ID2D1RenderTarget, ID2D1SolidColorBrush,
    },
    Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP},
    Win32::Graphics::Direct3D11::{
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
    },
    Win32::Graphics::DirectWrite::IDWriteTextLayout,
    Win32::Graphics::Dxgi::{
        Common::{
            DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM,
            DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC,
        },
        DXGI_PRESENT, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG,
        DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice,
        IDXGISurface, IDXGISwapChain1,
    },
    core::Interface,
};
#[cfg(target_os = "windows")]
use windows_numerics::Vector2;

#[cfg(target_os = "windows")]
pub struct D2DRenderBackend<'a> {
    factory: ID2D1Factory1,
    context: ID2D1DeviceContext,
    swap_chain: IDXGISwapChain1,
    size: Sizef,
    scale: f32,
    solid_brush: ID2D1SolidColorBrush,
    point_buffer: Vec<Vector2>,
    gradient_stop_buffer: Vec<D2D1_GRADIENT_STOP>,
    images: Option<&'a crate::ImageRegistry>,
    texts: Option<&'a crate::TextRegistry>,
}

#[cfg(target_os = "windows")]
impl<'a> D2DRenderBackend<'a> {
    pub fn new(hwnd: HWND) -> Option<Self> {
        unsafe {
            let factory =
                D2D1CreateFactory::<ID2D1Factory1>(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).ok()?;

            let d3d_device = create_d3d_device().ok()?;
            let swap_chain = create_swap_chain(&d3d_device, hwnd).ok()?;
            let context = create_device_context(&factory, &d3d_device).ok()?;

            let solid_brush = context
                .CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                    None,
                )
                .ok()?;

            Some(Self {
                factory,
                context,
                swap_chain,
                size: Sizef::new(0.0, 0.0),
                scale: 1.0,
                solid_brush,
                point_buffer: Vec::new(),
                gradient_stop_buffer: Vec::new(),
                images: None,
                texts: None,
            })
        }
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
    }

    pub fn set_registries(
        &mut self,
        images: Option<&'a crate::ImageRegistry>,
        texts: Option<&'a crate::TextRegistry>,
    ) {
        self.images = images;
        self.texts = texts;
    }

    pub(crate) fn draw_text_layout(
        &mut self,
        layout: &IDWriteTextLayout,
        position: Pointf,
        color: Color,
    ) {
        unsafe {
            let [r, g, b, a] = color.to_premultiplied_f32();
            self.solid_brush.SetColor(&D2D1_COLOR_F { r, g, b, a });
            self.context.DrawTextLayout(
                Vector2 {
                    X: position.x,
                    Y: position.y,
                },
                layout,
                &self.solid_brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
        }
    }

    fn bind_target_bitmap(&mut self) {
        unsafe {
            let surface = match self.swap_chain.GetBuffer::<IDXGISurface>(0) {
                Ok(surface) => surface,
                Err(_) => return,
            };

            let props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_IGNORE,
                },
                dpiX: 96.0 * self.scale,
                dpiY: 96.0 * self.scale,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                ..Default::default()
            };

            if let Ok(bitmap) = self
                .context
                .CreateBitmapFromDxgiSurface(&surface, Some(&props))
            {
                self.context.SetTarget(&bitmap);
            }
        }
    }
}

#[cfg(target_os = "windows")]
impl<'a> RenderBackend for D2DRenderBackend<'a> {
    fn begin(&mut self, size: Sizef) {
        unsafe {
            if size.width != self.size.width || size.height != self.size.height {
                self.size = size;
                let physical_width = (size.width * self.scale).max(1.0) as u32;
                let physical_height = (size.height * self.scale).max(1.0) as u32;
                let _ = self.swap_chain.ResizeBuffers(
                    0,
                    physical_width,
                    physical_height,
                    DXGI_FORMAT_UNKNOWN,
                    DXGI_SWAP_CHAIN_FLAG(0),
                );
            }

            self.context.SetDpi(96.0 * self.scale, 96.0 * self.scale);
            self.bind_target_bitmap();
            self.context.BeginDraw();
        }
    }

    fn replay(&mut self, commands: &CommandList) {
        for command in commands {
            self.draw_command(command);
        }
    }

    fn end(&mut self) {
        unsafe {
            let _ = self.context.EndDraw(None, None);
            let _ = self.swap_chain.Present(1, DXGI_PRESENT(0));
        }
    }
}

#[cfg(target_os = "windows")]
impl<'a> D2DRenderBackend<'a> {
    fn draw_command(&mut self, command: &PaintCommand) {
        match *command {
            PaintCommand::Clear { color } => unsafe {
                self.context.Clear(Some(&color_to_d2d(color)));
            },
            PaintCommand::FillRect { rect, color } => unsafe {
                self.solid_brush.SetColor(&color_to_d2d(color));
                self.context
                    .FillRectangle(&rect_to_d2d(rect), &self.solid_brush);
            },
            PaintCommand::StrokeRect { rect, color, width } => unsafe {
                self.solid_brush.SetColor(&color_to_d2d(color));
                self.context
                    .DrawRectangle(&rect_to_d2d(rect), &self.solid_brush, width, None);
            },
            PaintCommand::FillRoundedRect {
                rect,
                radius,
                color,
            } => {
                let rounded = D2D1_ROUNDED_RECT {
                    rect: rect_to_d2d(rect),
                    radiusX: radius,
                    radiusY: radius,
                };
                unsafe {
                    self.solid_brush.SetColor(&color_to_d2d(color));
                    self.context
                        .FillRoundedRectangle(&rounded, &self.solid_brush);
                }
            }
            PaintCommand::StrokeRoundedRect {
                rect,
                radius,
                color,
                width,
            } => {
                let rounded = D2D1_ROUNDED_RECT {
                    rect: rect_to_d2d(rect),
                    radiusX: radius,
                    radiusY: radius,
                };
                unsafe {
                    self.solid_brush.SetColor(&color_to_d2d(color));
                    self.context
                        .DrawRoundedRectangle(&rounded, &self.solid_brush, width, None);
                }
            }
            PaintCommand::FillPath { points, color } => {
                if points.len() < 2 {
                    return;
                }
                if let Some(geometry) = self.create_path_geometry(points, true) {
                    unsafe {
                        self.solid_brush.SetColor(&color_to_d2d(color));
                        self.context
                            .FillGeometry(&geometry, &self.solid_brush, None);
                    }
                }
            }
            PaintCommand::StrokePath {
                points,
                color,
                width,
            } => {
                if points.len() < 2 {
                    return;
                }
                if let Some(geometry) = self.create_path_geometry(points, false) {
                    unsafe {
                        self.solid_brush.SetColor(&color_to_d2d(color));
                        self.context
                            .DrawGeometry(&geometry, &self.solid_brush, width, None);
                    }
                }
            }
            PaintCommand::LinearGradient {
                rect,
                start,
                end,
                stops,
            } => {
                if stops.len() < 2 {
                    return;
                }
                self.draw_linear_gradient(rect, start, end, stops);
            }
            PaintCommand::DrawImage { rect, image } => {
                if let Some((width, height, rgba)) =
                    self.images.and_then(|images| images.get(image))
                {
                    self.draw_image(rect, width, height, rgba);
                }
            }
            PaintCommand::DrawText { position, text } => {
                let layout = self.texts.and_then(|texts| texts.get(text));
                if let Some(layout) = layout {
                    layout.draw(self, position);
                }
            }
            PaintCommand::DrawGpuSurface { .. } => {}
        }
    }

    fn create_path_geometry(
        &mut self,
        points: &[Pointf],
        close: bool,
    ) -> Option<ID2D1PathGeometry1> {
        self.point_buffer.clear();
        self.point_buffer.reserve(points.len());
        for point in points {
            self.point_buffer.push(Vector2 {
                X: point.x,
                Y: point.y,
            });
        }

        unsafe {
            let geometry = self.factory.CreatePathGeometry().ok()?;
            let sink = geometry.Open().ok()?;

            let figure_begin = if close {
                D2D1_FIGURE_BEGIN_FILLED
            } else {
                D2D1_FIGURE_BEGIN_HOLLOW
            };
            sink.BeginFigure(self.point_buffer[0], figure_begin);
            sink.AddLines(&self.point_buffer[1..]);
            let figure_end = if close {
                D2D1_FIGURE_END_CLOSED
            } else {
                D2D1_FIGURE_END_OPEN
            };
            sink.EndFigure(figure_end);
            let _ = sink.Close();

            Some(geometry)
        }
    }

    fn draw_linear_gradient(
        &mut self,
        rect: Rectf,
        start: Pointf,
        end: Pointf,
        stops: &[gui_core::ColorStop],
    ) {
        self.gradient_stop_buffer.clear();
        self.gradient_stop_buffer.reserve(stops.len());
        for stop in stops {
            let [r, g, b, a] = stop.color.to_premultiplied_f32();
            self.gradient_stop_buffer.push(D2D1_GRADIENT_STOP {
                position: stop.position,
                color: D2D1_COLOR_F { r, g, b, a },
            });
        }

        unsafe {
            let collection = match ID2D1RenderTarget::CreateGradientStopCollection(
                &self.context,
                &self.gradient_stop_buffer,
                D2D1_GAMMA_2_2,
                D2D1_EXTEND_MODE_CLAMP,
            ) {
                Ok(collection) => collection,
                Err(_) => return,
            };

            let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                startPoint: Vector2 {
                    X: start.x,
                    Y: start.y,
                },
                endPoint: Vector2 { X: end.x, Y: end.y },
            };

            if let Ok(brush) = self
                .context
                .CreateLinearGradientBrush(&props, None, &collection)
            {
                self.context.FillRectangle(&rect_to_d2d(rect), &brush);
            }
        }
    }

    /// Creates a transient `ID2D1Bitmap1` from premultiplied-alpha RGBA8
    /// bytes and draws it into `rect`. The bitmap is not cached since this
    /// backend is reconstructed every frame (see `render_to_hwnd`).
    fn draw_image(&mut self, rect: Rectf, width: u32, height: u32, rgba: &[u8]) {
        let props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_R8G8B8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            ..Default::default()
        };

        unsafe {
            let bitmap = self.context.CreateBitmap(
                D2D_SIZE_U { width, height },
                Some(rgba.as_ptr() as *const core::ffi::c_void),
                width * 4,
                &props,
            );

            if let Ok(bitmap) = bitmap {
                self.context.DrawBitmap(
                    &bitmap,
                    Some(&rect_to_d2d(rect)),
                    1.0,
                    D2D1_INTERPOLATION_MODE_LINEAR,
                    None,
                    None,
                );
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) unsafe fn create_d3d_device() -> windows::core::Result<ID3D11Device> {
    let mut device = None;
    let mut result = D3D11CreateDevice(
        None,
        D3D_DRIVER_TYPE_HARDWARE,
        HMODULE::default(),
        D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        None,
        D3D11_SDK_VERSION,
        Some(&mut device),
        None,
        None,
    );

    if result.is_err() {
        result = D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_WARP,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        );
    }

    result.map(|()| device.unwrap())
}

#[cfg(target_os = "windows")]
unsafe fn create_device_context(
    factory: &ID2D1Factory1,
    d3d_device: &ID3D11Device,
) -> windows::core::Result<ID2D1DeviceContext> {
    let dxgi_device: IDXGIDevice = d3d_device.cast()?;
    let d2d_device = factory.CreateDevice(&dxgi_device)?;
    d2d_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)
}

#[cfg(target_os = "windows")]
pub(crate) unsafe fn create_swap_chain(
    d3d_device: &ID3D11Device,
    hwnd: HWND,
) -> windows::core::Result<IDXGISwapChain1> {
    let dxgi_device: IDXGIDevice = d3d_device.cast()?;
    let dxgi_adapter = dxgi_device.GetAdapter()?;
    let dxgi_factory: windows::Win32::Graphics::Dxgi::IDXGIFactory2 = dxgi_adapter.GetParent()?;

    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: 1,
        Height: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        Stereo: windows::core::BOOL(0),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_IGNORE,
        Flags: 0,
    };

    dxgi_factory.CreateSwapChainForHwnd(d3d_device, hwnd, &desc, None, None)
}

#[cfg(target_os = "windows")]
fn color_to_d2d(color: Color) -> D2D1_COLOR_F {
    let [r, g, b, a] = color.to_premultiplied_f32();
    D2D1_COLOR_F { r, g, b, a }
}

#[cfg(target_os = "windows")]
fn rect_to_d2d(rect: Rectf) -> D2D1_RECT_F {
    D2D1_RECT_F {
        left: rect.origin.x,
        top: rect.origin.y,
        right: rect.origin.x + rect.size.width,
        bottom: rect.origin.y + rect.size.height,
    }
}

#[cfg(target_os = "windows")]
pub fn render_to_hwnd(
    hwnd: *mut core::ffi::c_void,
    size: Sizef,
    scale: f32,
    commands: &CommandList,
) -> Option<()> {
    if size.width <= 0.0 || size.height <= 0.0 {
        return None;
    }

    let hwnd = HWND(hwnd);
    let mut backend = D2DRenderBackend::new(hwnd)?;
    backend.set_scale(scale);
    backend.begin(size);
    backend.replay(commands);
    backend.end();
    Some(())
}

/// Like [`render_to_hwnd`], but resolves `DrawImage`/`DrawText` paint
/// commands against the provided registries.
#[cfg(target_os = "windows")]
pub fn render_to_hwnd_with_registries(
    hwnd: *mut core::ffi::c_void,
    size: Sizef,
    scale: f32,
    commands: &CommandList,
    images: Option<&crate::ImageRegistry>,
    texts: Option<&crate::TextRegistry>,
) -> Option<()> {
    if size.width <= 0.0 || size.height <= 0.0 {
        return None;
    }

    let hwnd = HWND(hwnd);
    let mut backend = D2DRenderBackend::new(hwnd)?;
    backend.set_scale(scale);
    backend.set_registries(images, texts);
    backend.begin(size);
    backend.replay(commands);
    backend.end();
    Some(())
}

#[cfg(not(target_os = "windows"))]
pub struct D2DRenderBackend {
    _private: (),
}

#[cfg(not(target_os = "windows"))]
impl D2DRenderBackend {
    pub fn new(_hwnd: *mut core::ffi::c_void) -> Option<Self> {
        Some(Self { _private: () })
    }
}

#[cfg(not(target_os = "windows"))]
impl RenderBackend for D2DRenderBackend {}

#[cfg(not(target_os = "windows"))]
pub fn render_to_hwnd(
    _hwnd: *mut core::ffi::c_void,
    _size: Sizef,
    _scale: f32,
    _commands: &CommandList,
) -> Option<()> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn render_to_hwnd_with_registries(
    _hwnd: *mut core::ffi::c_void,
    _size: Sizef,
    _scale: f32,
    _commands: &CommandList,
    _images: Option<&crate::ImageRegistry>,
    _texts: Option<&crate::TextRegistry>,
) -> Option<()> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d2d_render_backend_exports() {
        let _ = core::mem::size_of::<D2DRenderBackend>();
    }
}
