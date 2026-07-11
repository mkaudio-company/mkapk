#![allow(unexpected_cfgs)]

use gui_core::{CommandList, Pointf, Rectf, RenderBackend, Sizef};

#[cfg(target_os = "macos")]
use gui_core::{Color, PaintCommand};

#[cfg(target_os = "macos")]
use core_graphics::base::CGFloat;
#[cfg(target_os = "macos")]
use core_graphics::color_space::CGColorSpace;
#[cfg(target_os = "macos")]
use core_graphics::context::{CGContext, CGContextRef};
#[cfg(target_os = "macos")]
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
#[cfg(target_os = "macos")]
use core_graphics::gradient::{CGGradient, CGGradientDrawingOptions};

#[cfg(target_os = "macos")]
use core_graphics::path::CGPath;
#[cfg(target_os = "macos")]
use foreign_types::ForeignType;

#[cfg(target_os = "macos")]
pub struct CoreGraphicsRenderBackend<'a> {
    context: CGContext,
    clip: Rectf,
    #[allow(dead_code)]
    scale: f32,
    images: Option<&'a crate::ImageRegistry>,
    texts: Option<&'a crate::TextRegistry>,
}

#[cfg(target_os = "macos")]
impl<'a> CoreGraphicsRenderBackend<'a> {
    pub fn new(context: CGContext, clip: Rectf, scale: f32) -> Self {
        Self {
            context,
            clip,
            scale,
            images: None,
            texts: None,
        }
    }

    pub fn with_registries(
        context: CGContext,
        clip: Rectf,
        scale: f32,
        images: Option<&'a crate::ImageRegistry>,
        texts: Option<&'a crate::TextRegistry>,
    ) -> Self {
        Self {
            context,
            clip,
            scale,
            images,
            texts,
        }
    }
}

#[cfg(target_os = "macos")]
impl<'a> RenderBackend for CoreGraphicsRenderBackend<'a> {
    fn begin(&mut self, size: Sizef) {
        let ctx = self.context.as_ref();
        ctx.save();

        let bounds = CGRect::new(
            &CGPoint::new(0.0, 0.0),
            &CGSize::new(size.width as CGFloat, size.height as CGFloat),
        );
        ctx.clip_to_rect(bounds);

        // Convert from the framework's top-left origin to CoreGraphics's
        // bottom-left origin while keeping the clip rectangle unchanged.
        ctx.translate(0.0, size.height as CGFloat);
        ctx.scale(1.0, -1.0);

        self.clip = Rectf::new(Pointf::new(0.0, 0.0), size);
    }

    fn replay(&mut self, commands: &CommandList) {
        for cmd in commands {
            self.draw_command(cmd);
        }
    }

    fn end(&mut self) {
        self.context.as_ref().restore();
    }
}

#[cfg(target_os = "macos")]
impl<'a> CoreGraphicsRenderBackend<'a> {
    fn draw_command(&mut self, command: &PaintCommand) {
        let ctx = self.context.as_ref();
        match *command {
            PaintCommand::Clear { color } => {
                let (r, g, b, a) = color_components(color);
                ctx.set_rgb_fill_color(r, g, b, a);
                ctx.fill_rect(rect_to_cg(self.clip));
            }
            PaintCommand::FillRect { rect, color } => {
                let (r, g, b, a) = color_components(color);
                ctx.set_rgb_fill_color(r, g, b, a);
                ctx.fill_rect(rect_to_cg(rect));
            }
            PaintCommand::StrokeRect { rect, color, width } => {
                let (r, g, b, a) = color_components(color);
                ctx.set_rgb_stroke_color(r, g, b, a);
                ctx.set_line_width(width as CGFloat);
                ctx.stroke_rect_with_width(rect_to_cg(rect), width as CGFloat);
            }
            PaintCommand::FillRoundedRect {
                rect,
                radius,
                color,
            } => {
                let (r, g, b, a) = color_components(color);
                add_rounded_rect_path(ctx, rect, radius);
                ctx.set_rgb_fill_color(r, g, b, a);
                ctx.fill_path();
            }
            PaintCommand::StrokeRoundedRect {
                rect,
                radius,
                color,
                width,
            } => {
                let (r, g, b, a) = color_components(color);
                add_rounded_rect_path(ctx, rect, radius);
                ctx.set_rgb_stroke_color(r, g, b, a);
                ctx.set_line_width(width as CGFloat);
                ctx.stroke_path();
            }
            PaintCommand::FillPath { points, color } => {
                if points.is_empty() {
                    return;
                }
                let (r, g, b, a) = color_components(color);
                add_polyline_path(ctx, points, true);
                ctx.set_rgb_fill_color(r, g, b, a);
                ctx.fill_path();
            }
            PaintCommand::StrokePath {
                points,
                color,
                width,
            } => {
                if points.is_empty() {
                    return;
                }
                let (r, g, b, a) = color_components(color);
                add_polyline_path(ctx, points, false);
                ctx.set_rgb_stroke_color(r, g, b, a);
                ctx.set_line_width(width as CGFloat);
                ctx.stroke_path();
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
                draw_linear_gradient(ctx, rect, start, end, stops);
            }
            PaintCommand::DrawImage { rect, image } => {
                if let Some(cg_image) = self.images.and_then(|images| images.get(image)) {
                    // `CGContextDrawImage` draws bottom-up relative to the
                    // current transform. The context is already globally
                    // flipped to a top-left origin (see `begin`), so a local
                    // flip around the destination rect is needed to draw the
                    // image right-side up.
                    ctx.save();
                    ctx.translate(
                        rect.origin.x as CGFloat,
                        (rect.origin.y + rect.size.height) as CGFloat,
                    );
                    ctx.scale(1.0, -1.0);
                    ctx.draw_image(
                        CGRect::new(
                            &CGPoint::new(0.0, 0.0),
                            &CGSize::new(rect.size.width as CGFloat, rect.size.height as CGFloat),
                        ),
                        cg_image,
                    );
                    ctx.restore();
                }
            }
            PaintCommand::DrawText { position, text } => {
                if let Some(layout) = self.texts.and_then(|texts| texts.get(text)) {
                    layout.draw(&self.context, position);
                }
            }
            PaintCommand::DrawGpuSurface { .. } => {}
        }
    }
}

#[cfg(target_os = "macos")]
fn color_components(color: Color) -> (CGFloat, CGFloat, CGFloat, CGFloat) {
    const SCALE: f32 = 1.0 / 255.0;
    (
        (color.r as f32 * SCALE) as CGFloat,
        (color.g as f32 * SCALE) as CGFloat,
        (color.b as f32 * SCALE) as CGFloat,
        (color.a as f32 * SCALE) as CGFloat,
    )
}

#[cfg(target_os = "macos")]
fn rect_to_cg(rect: Rectf) -> CGRect {
    CGRect::new(
        &CGPoint::new(rect.origin.x as CGFloat, rect.origin.y as CGFloat),
        &CGSize::new(rect.size.width as CGFloat, rect.size.height as CGFloat),
    )
}

#[cfg(target_os = "macos")]
fn point_to_cg(point: Pointf) -> CGPoint {
    CGPoint::new(point.x as CGFloat, point.y as CGFloat)
}

#[cfg(target_os = "macos")]
fn add_polyline_path(ctx: &CGContextRef, points: &[Pointf], close: bool) {
    ctx.begin_path();
    ctx.move_to_point(points[0].x as CGFloat, points[0].y as CGFloat);
    for point in &points[1..] {
        ctx.add_line_to_point(point.x as CGFloat, point.y as CGFloat);
    }
    if close {
        ctx.close_path();
    }
}

#[cfg(target_os = "macos")]
fn add_rounded_rect_path(ctx: &CGContextRef, rect: Rectf, radius: f32) {
    let x = rect.origin.x as CGFloat;
    let y = rect.origin.y as CGFloat;
    let w = rect.size.width as CGFloat;
    let h = rect.size.height as CGFloat;
    let r = radius
        .min(rect.size.width / 2.0)
        .min(rect.size.height / 2.0) as CGFloat;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPathCreateWithRoundedRect(
            rect: CGRect,
            cornerWidth: CGFloat,
            cornerHeight: CGFloat,
            transform: *const core_graphics::geometry::CGAffineTransform,
        ) -> core_graphics::sys::CGPathRef;
    }

    unsafe {
        let path = CGPath::from_ptr(CGPathCreateWithRoundedRect(
            CGRect::new(&CGPoint::new(x, y), &CGSize::new(w, h)),
            r,
            r,
            core::ptr::null(),
        ));
        ctx.add_path(path.as_ref());
    }
}

#[cfg(target_os = "macos")]
fn draw_linear_gradient(
    ctx: &CGContextRef,
    rect: Rectf,
    start: Pointf,
    end: Pointf,
    stops: &[gui_core::ColorStop],
) {
    let mut components = Vec::with_capacity(stops.len() * 4);
    let mut locations = Vec::with_capacity(stops.len());
    for stop in stops {
        let (r, g, b, a) = color_components(stop.color);
        components.push(r);
        components.push(g);
        components.push(b);
        components.push(a);
        locations.push(stop.position as CGFloat);
    }

    let color_space = CGColorSpace::create_device_rgb();
    let gradient = CGGradient::create_with_color_components(
        &color_space,
        &components,
        &locations,
        stops.len(),
    );

    ctx.save();
    ctx.clip_to_rect(rect_to_cg(rect));
    ctx.draw_linear_gradient(
        &gradient,
        point_to_cg(start),
        point_to_cg(end),
        CGGradientDrawingOptions::empty(),
    );
    ctx.restore();
}

#[cfg(target_os = "macos")]
pub fn render_to_view(
    view: *mut core::ffi::c_void,
    size: Sizef,
    scale: f32,
    commands: &CommandList,
) -> Option<()> {
    let (view, cg_context) = lock_view_context(view)?;
    let mut backend = CoreGraphicsRenderBackend::new(cg_context, Rectf::default(), scale);
    backend.begin(size);
    backend.replay(commands);
    backend.end();
    unlock_view(view);
    Some(())
}

/// Like [`render_to_view`], but resolves `DrawImage`/`DrawText` paint
/// commands against the provided registries.
#[cfg(target_os = "macos")]
pub fn render_to_view_with_registries(
    view: *mut core::ffi::c_void,
    size: Sizef,
    scale: f32,
    commands: &CommandList,
    images: Option<&crate::ImageRegistry>,
    texts: Option<&crate::TextRegistry>,
) -> Option<()> {
    let (view, cg_context) = lock_view_context(view)?;
    let mut backend = CoreGraphicsRenderBackend::with_registries(
        cg_context,
        Rectf::default(),
        scale,
        images,
        texts,
    );
    backend.begin(size);
    backend.replay(commands);
    backend.end();
    unlock_view(view);
    Some(())
}

#[cfg(target_os = "macos")]
pub fn render_text_to_view(
    view: *mut core::ffi::c_void,
    _size: Sizef,
    _scale: f32,
    layout: &crate::TextLayout,
    position: Pointf,
) -> Option<()> {
    let (view, cg_context) = lock_view_context(view)?;
    layout.draw(&cg_context, position);
    unlock_view(view);
    Some(())
}

#[cfg(target_os = "macos")]
fn lock_view_context(
    view: *mut core::ffi::c_void,
) -> Option<(*mut objc::runtime::Object, CGContext)> {
    use objc::runtime::{BOOL, Object, YES};

    if view.is_null() {
        return None;
    }

    let view = view as *mut Object;

    let locked: BOOL = unsafe { msg_send![view, lockFocusIfCanDraw] };
    if locked != YES {
        return None;
    }

    let context_class = objc::runtime::Class::get("NSGraphicsContext")?;
    let ns_context: *mut Object = unsafe { msg_send![context_class, currentContext] };
    if ns_context.is_null() {
        unsafe {
            let _: () = msg_send![view, unlockFocus];
        }
        return None;
    }

    let cg_ptr: *mut core_graphics::sys::CGContext = unsafe { msg_send![ns_context, CGContext] };
    if cg_ptr.is_null() {
        unsafe {
            let _: () = msg_send![view, unlockFocus];
        }
        return None;
    }

    Some((view, unsafe {
        CGContext::from_existing_context_ptr(cg_ptr)
    }))
}

#[cfg(target_os = "macos")]
fn unlock_view(view: *mut objc::runtime::Object) {
    unsafe {
        let _: () = msg_send![view, unlockFocus];
        // `lockFocus`-based immediate drawing writes into the window's
        // buffered backing store but is not otherwise scheduled for
        // display; without an explicit flush here the drawn content never
        // reaches the screen.
        let window: *mut objc::runtime::Object = msg_send![view, window];
        if !window.is_null() {
            let _: () = msg_send![window, flushWindow];
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub struct CoreGraphicsRenderBackend {
    _private: (),
}

#[cfg(not(target_os = "macos"))]
impl CoreGraphicsRenderBackend {
    pub fn new(_clip: Rectf, _scale: f32) -> Self {
        Self { _private: () }
    }
}

#[cfg(not(target_os = "macos"))]
impl RenderBackend for CoreGraphicsRenderBackend {}

#[cfg(not(target_os = "macos"))]
pub fn render_to_view(
    _view: *mut core::ffi::c_void,
    _size: Sizef,
    _scale: f32,
    _commands: &CommandList,
) -> Option<()> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn render_to_view_with_registries(
    _view: *mut core::ffi::c_void,
    _size: Sizef,
    _scale: f32,
    _commands: &CommandList,
    _images: Option<&crate::ImageRegistry>,
    _texts: Option<&crate::TextRegistry>,
) -> Option<()> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn render_text_to_view(
    _view: *mut core::ffi::c_void,
    _size: Sizef,
    _scale: f32,
    _layout: &crate::TextLayout,
    _position: Pointf,
) -> Option<()> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_graphics_render_backend_size() {
        let _ = core::mem::size_of::<CoreGraphicsRenderBackend>();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn draw_command_writes_expected_pixels_to_offscreen_bitmap() {
        let width = 8usize;
        let height = 8usize;
        let color_space = CGColorSpace::create_device_rgb();
        let ctx = CGContext::create_bitmap_context(
            None,
            width,
            height,
            8,
            width * 4,
            &color_space,
            core_graphics::image::CGImageAlphaInfo::CGImageAlphaPremultipliedLast as u32,
        );
        let mut readback_ctx = ctx.clone();

        let mut images = crate::ImageRegistry::new();
        let red_pixels: [u8; 16] = [
            255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
        ];
        images.register_rgba(gui_core::ImageId(1), 2, 2, &red_pixels);

        let mut commands = CommandList::new();
        commands.push(PaintCommand::Clear {
            color: Color::new(30, 30, 30, 255),
        });
        commands.push(PaintCommand::FillRect {
            rect: Rectf::new(Pointf::new(0.0, 0.0), gui_core::Sizef::new(4.0, 4.0)),
            color: Color::new(50, 50, 50, 255),
        });
        commands.push(PaintCommand::DrawImage {
            rect: Rectf::new(Pointf::new(4.0, 4.0), gui_core::Sizef::new(4.0, 4.0)),
            image: gui_core::ImageId(1),
        });

        let mut backend = CoreGraphicsRenderBackend::with_registries(
            ctx,
            Rectf::default(),
            1.0,
            Some(&images),
            None,
        );
        backend.begin(Sizef::new(width as f32, height as f32));
        backend.replay(&commands);
        backend.end();
        drop(backend);

        let bytes_per_row = readback_ctx.bytes_per_row();
        let data = readback_ctx.data();

        let pixel_at = |x: usize, y: usize| -> (u8, u8, u8, u8) {
            let offset = y * bytes_per_row + x * 4;
            (
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            )
        };

        // Top-left: FillRect(50,50,50) should have overwritten Clear(30,30,30).
        assert_eq!(pixel_at(1, 1), (50, 50, 50, 255));
        // Bottom-right quadrant: the drawn red image (bitmap row 0 is at the
        // top since CoreGraphics context memory is top-down for this API).
        assert_eq!(pixel_at(5, 5), (255, 0, 0, 255));
        // Untouched region should still show the Clear color.
        assert_eq!(pixel_at(1, 6), (30, 30, 30, 255));
    }
}
