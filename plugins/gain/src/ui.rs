//! The plugin's editor UI. This is the ONE UI file — every build target
//! (Standalone/VST3/AU/AAX) shares this same `GainEditor` implementation of
//! `mkapk_host::PluginEditor`. The processor lives separately in
//! `crate::processor`; the two are wired together only via
//! `mkapk_host::LockFreeParameterGateway`, never a direct reference to each
//! other, so this file has no audio-thread/real-time concerns at all.
use std::cell::Cell;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mkapk_core::{
    Color, CommandList, Event, EventDispatcher, EventResponse, ImageId, Insetsf, LayoutConstraints,
    LayoutDirection, LayoutEngine, LayoutNode, LayoutResult, MouseEvent, PaintCommand,
    PointerEvent, Pointf, Rectf, Sizef, TextLayoutId, TraverseOrder, Tree, Widget, WidgetId,
    downcast_widget_ref,
};
use mkapk_host::{
    EditorHost, LockFreeParameterGateway, NormalizedValue, ParameterGateway, ParameterId,
    ParentWindowHandle, PeakMeter, PluginEditor, SizeConstraints,
};
use mkapk_res::{
    PngImage, Resource, ResourceBundle, ResourceHandle, ResourceId, ResourceRegistry,
    generated::EMBEDDED,
};
use mkapk_widgets::{Knob, Label, Slider, Theme};

use crate::processor::GAIN_PARAM;

#[cfg(target_os = "macos")]
type ImageRegistry = mkapk_mac::ImageRegistry;
#[cfg(target_os = "macos")]
type TextRegistry = mkapk_mac::TextRegistry;
#[cfg(target_os = "macos")]
type PlatformTextLayout = mkapk_mac::TextLayout;

#[cfg(target_os = "windows")]
type ImageRegistry = mkapk_win32::ImageRegistry;
#[cfg(target_os = "windows")]
type TextRegistry = mkapk_win32::TextRegistry;
#[cfg(target_os = "windows")]
type PlatformTextLayout = mkapk_win32::TextLayout;

struct Panel {
    id: WidgetId,
    theme: Theme,
    frame: Cell<Rectf>,
}

impl Panel {
    fn new(theme: Theme) -> Self {
        Self {
            id: WidgetId::new(),
            theme,
            frame: Cell::new(Rectf::default()),
        }
    }

    fn set_frame(&self, frame: Rectf) {
        self.frame.set(frame);
    }
}

impl Widget for Panel {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout(&self, _constraints: LayoutConstraints) -> Sizef {
        Sizef::zero()
    }

    fn paint(&self, commands: &mut CommandList) {
        commands.push(PaintCommand::FillRect {
            rect: self.frame.get(),
            color: self.theme.background,
        });
    }
}

pub struct GainEditor {
    view: *mut core::ffi::c_void,
    size: Sizef,
    tree: Tree,
    layout_engine: LayoutEngine,
    layout: LayoutResult,
    gateway: Arc<LockFreeParameterGateway>,
    meter: PeakMeter,
    meter_value: f32,
    last_frame: Option<Instant>,
    mouse_capture: Option<WidgetId>,
    commands: CommandList,
    logo_handle: ResourceHandle<PngImage>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    image_registry: ImageRegistry,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    text_registry: TextRegistry,
    last_meter_percent: i32,
    #[cfg(target_os = "macos")]
    accessibility_handle: Option<mkapk_mac::AccessibilityElementHandle>,
    _panel: WidgetId,
    _label: WidgetId,
    slider_id: WidgetId,
    knob_id: WidgetId,
}

impl GainEditor {
    /// `gateway` should be the same `Arc<LockFreeParameterGateway>` driving
    /// the paired `GainProcessor`'s real-time callback, and `meter` the same
    /// `PeakMeter` that callback writes the real output level into (see
    /// `mkapk-standalone` or the VST3/AU entry points for how each format
    /// wires this up).
    pub fn new(gateway: Arc<LockFreeParameterGateway>, meter: PeakMeter) -> Self {
        let theme = Theme::default();
        let mut tree = Tree::new();

        let panel = Panel::new(theme);
        let root = panel.id();
        tree.insert(Box::new(panel), None);

        let label = Label::new("Gain", theme);
        let label_id = label.id();
        tree.insert(Box::new(label), Some(root));

        // Both widgets control the same `GAIN_PARAM`; `sync_widget_values`
        // keeps them showing the same value regardless of which one the
        // user is dragging (see `on_mouse_down`/`on_mouse_move`).
        let initial_gain = NormalizedValue::new(1.0);

        let slider = Slider::new(GAIN_PARAM, initial_gain, theme);
        let slider_id = slider.id();
        let gateway_for_slider = gateway.clone();
        slider.on_changed(move |id, value| {
            gateway_for_slider.set_normalized(id, value);
        });
        tree.insert(Box::new(slider), Some(root));

        let knob = Knob::new(GAIN_PARAM, initial_gain, theme);
        let knob_id = knob.id();
        let gateway_for_knob = gateway.clone();
        knob.on_changed(move |id, value| {
            gateway_for_knob.set_normalized(id, value);
        });
        tree.insert(Box::new(knob), Some(root));

        let mut layout_engine = LayoutEngine::new();
        layout_engine.set_node(LayoutNode {
            id: root,
            direction: LayoutDirection::Column,
            padding: theme.padding,
            ..LayoutNode::default()
        });

        let margin = Insetsf::uniform(4.0);
        for &id in &[label_id, slider_id, knob_id] {
            layout_engine.set_node(LayoutNode {
                id,
                margin,
                ..LayoutNode::default()
            });
        }

        let size = Sizef::new(400.0, 300.0);
        let constraints = LayoutConstraints {
            min_width: Some(size.width),
            max_width: Some(size.width),
            min_height: Some(size.height),
            max_height: Some(size.height),
        };

        let layout = layout_engine.compute(&tree, constraints);

        let mut registry = ResourceRegistry::new();
        EMBEDDED.register_with(&mut registry);
        let logo_handle = registry
            .load::<PngImage>(ResourceId::from_bytes_le(b"logo.png"))
            .expect("logo.png must be embedded");

        // `ResourceHandle` only allows shared access to the cached decode, so
        // rasterize a standalone copy once here to populate the platform
        // image registry (CGImage / ID2D1Bitmap source bytes).
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        let mut image_registry = ImageRegistry::new();
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let logo_bytes = EMBEDDED
                .get(ResourceId::from_bytes_le(b"logo.png"))
                .expect("logo.png must be embedded");
            let logo_png = PngImage::decode(logo_bytes).expect("logo.png should decode");
            let premultiplied = logo_png.rgba_premultiplied();
            image_registry.register_rgba(
                ImageId(2),
                logo_png.width(),
                logo_png.height(),
                &premultiplied,
            );
        }
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        let mut text_registry = TextRegistry::new();
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        text_registry.insert(
            TextLayoutId(0),
            PlatformTextLayout::new("Gain", theme.font_size),
        );

        let mut editor = Self {
            view: core::ptr::null_mut(),
            size,
            tree,
            layout_engine,
            layout,
            gateway,
            meter,
            meter_value: 0.0,
            last_frame: None,
            mouse_capture: None,
            commands: CommandList::with_capacity(32),
            logo_handle,
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            image_registry,
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            text_registry,
            last_meter_percent: -1,
            #[cfg(target_os = "macos")]
            accessibility_handle: None,
            _panel: root,
            _label: label_id,
            slider_id,
            knob_id,
        };
        editor.apply_layout();
        editor
    }

    /// Pushes `value` into both the slider and the knob, so whichever one
    /// the user isn't currently dragging still reflects the current gain.
    fn sync_widget_values(&mut self, value: NormalizedValue) {
        if let Some(node) = self.tree.find(self.slider_id) {
            let widget = node.widget.borrow();
            if let Some(slider) = downcast_widget_ref::<Slider>(&**widget) {
                slider.set_value(value);
            }
        }
        if let Some(node) = self.tree.find(self.knob_id) {
            let widget = node.widget.borrow();
            if let Some(knob) = downcast_widget_ref::<Knob>(&**widget) {
                knob.set_value(value);
            }
        }
    }

    fn apply_layout(&mut self) {
        for (&id, layout_box) in self.layout.iter() {
            let Some(node) = self.tree.find(id) else {
                continue;
            };
            let widget = node.widget.borrow();
            let frame = Rectf::new(layout_box.origin, layout_box.size);
            if let Some(panel) = downcast_widget_ref::<Panel>(&**widget) {
                panel.set_frame(frame);
            } else if let Some(label) = downcast_widget_ref::<Label>(&**widget) {
                label.set_frame(frame);
            } else if let Some(slider) = downcast_widget_ref::<Slider>(&**widget) {
                slider.set_frame(frame);
            } else if let Some(knob) = downcast_widget_ref::<Knob>(&**widget) {
                knob.set_frame(frame);
            }
        }

        self.tree.set_layout_result(self.layout.clone());
        #[cfg(target_os = "macos")]
        self.refresh_accessibility();
    }

    /// Mirrors the current widget accessibility tree into a real
    /// `NSAccessibilityElement` tree and attaches it to the live view, so
    /// VoiceOver/Accessibility Inspector can query widget roles/labels/values.
    #[cfg(target_os = "macos")]
    fn refresh_accessibility(&mut self) {
        if self.view.is_null() {
            return;
        }
        let a11y_tree = self.tree.accessibility_tree();
        let handle = mkapk_mac::build_accessibility_tree(&a11y_tree);
        mkapk_mac::attach_to_view(self.view, &handle);
        self.accessibility_handle = Some(handle);
    }

    fn rebuild_commands(&mut self) {
        self.commands.clear();
        for id in self.tree.traverse(TraverseOrder::PreOrder) {
            if let Some(node) = self.tree.find(id) {
                node.widget.borrow().paint(&mut self.commands);
            }
        }
        self.draw_peak_meter();
        self.draw_assets();
    }

    /// Reads the real peak level `PeakMeter` last had written into it from
    /// the audio callback, and applies a simple decay so the bar doesn't
    /// snap straight back to whatever the next-read value is between UI
    /// ticks (standard peak-meter ballistics: instant attack, timed decay).
    fn update_meter(&mut self) {
        let now = Instant::now();
        let dt = self
            .last_frame
            .map(|t| now.duration_since(t))
            .unwrap_or(Duration::from_millis(16));
        self.last_frame = Some(now);

        let level = self.meter.read().clamp(0.0, 1.0);
        const DECAY_PER_SECOND: f32 = 2.5;
        let decayed = self.meter_value - DECAY_PER_SECOND * dt.as_secs_f32();
        self.meter_value = level.max(decayed).max(0.0);
    }

    fn draw_peak_meter(&mut self) {
        let meter_width = 24.0;
        let meter_height = 120.0;
        let x = self.size.width - meter_width - 20.0;
        let y = self.size.height - 20.0 - meter_height;

        self.commands.push(PaintCommand::FillRect {
            rect: Rectf::new(Pointf::new(x, y), Sizef::new(meter_width, meter_height)),
            color: Color::from_rgb(40, 40, 40),
        });

        let bar_height = meter_height * self.meter_value;
        self.commands.push(PaintCommand::FillRect {
            rect: Rectf::new(
                Pointf::new(x, y + meter_height - bar_height),
                Sizef::new(meter_width, bar_height),
            ),
            color: Color::from_rgb(0, 200, 100),
        });

        #[cfg(any(target_os = "macos", target_os = "windows"))]
        self.commands.push(PaintCommand::DrawText {
            position: Pointf::new(x - 12.0, y - 20.0),
            text: TextLayoutId(1),
        });
    }

    /// Rebuilds the peak-meter percentage label only when its rounded value
    /// changes, avoiding a fresh `TextLayout` every frame.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn update_meter_text(&mut self) {
        let percent = (self.meter_value * 100.0).round() as i32;
        if percent == self.last_meter_percent {
            return;
        }
        self.last_meter_percent = percent;
        self.text_registry.insert(
            TextLayoutId(1),
            PlatformTextLayout::new(&format!("{percent}%"), 14.0),
        );
    }

    fn draw_assets(&mut self) {
        let logo_size = Sizef::new(
            self.logo_handle.width() as f32,
            self.logo_handle.height() as f32,
        );

        self.commands.push(PaintCommand::DrawImage {
            rect: Rectf::new(Pointf::new(20.0, 20.0), logo_size),
            image: ImageId(2),
        });
    }
}

impl PluginEditor for GainEditor {
    fn open(&mut self, parent: ParentWindowHandle, _host: &dyn EditorHost) {
        match parent {
            ParentWindowHandle::Mac(view) => {
                // Attach our own child `NSView` (with a real `drawRect:`
                // override) instead of drawing directly into the
                // host-provided view via `lockFocus`; see
                // `mkapk_mac::paint_view` for why.
                #[cfg(target_os = "macos")]
                {
                    self.view = mkapk_mac::attach_paint_view(view, self.size).unwrap_or(view);

                    let editor_ptr: *mut GainEditor = self;
                    // SAFETY: `editor_ptr` is only ever dereferenced
                    // synchronously, from AppKit's main-thread dispatch of
                    // `mouseDown:`/`mouseDragged:`/`mouseUp:` on the
                    // `GuiPaintView` just attached above, and never again
                    // once `close()` clears the sink below.
                    mkapk_mac::set_input_sink(
                        self.view,
                        Box::new(move |event| {
                            let editor = unsafe { &mut *editor_ptr };
                            match event {
                                Event::MouseDown(e) => editor.on_mouse_down(&e),
                                Event::MouseUp(e) => editor.on_mouse_up(&e),
                                Event::MouseMove(e) => editor.on_mouse_move(&e),
                                _ => EventResponse::Bubble,
                            }
                        }),
                    );
                }
                #[cfg(not(target_os = "macos"))]
                {
                    self.view = view;
                }
            }
            ParentWindowHandle::Windows(view) => {
                self.view = view;
            }
        }
        #[cfg(target_os = "macos")]
        self.refresh_accessibility();
    }

    fn resize(&mut self, size: Sizef) {
        self.size = size;
        let constraints = LayoutConstraints {
            min_width: Some(size.width),
            max_width: Some(size.width),
            min_height: Some(size.height),
            max_height: Some(size.height),
        };
        self.layout = self.layout_engine.compute(&self.tree, constraints);
        self.apply_layout();

        #[cfg(target_os = "macos")]
        mkapk_mac::resize_paint_view(self.view, size);
    }

    fn idle(&mut self) {
        self.update_meter();
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        self.update_meter_text();
        self.rebuild_commands();

        let view = self.view;
        let size = self.size;
        let commands = &self.commands;

        #[cfg(target_os = "macos")]
        {
            let _ = mkapk_mac::update_paint_view(
                view,
                size,
                1.0,
                commands,
                Some(&self.image_registry),
                Some(&self.text_registry),
            );
        }

        #[cfg(target_os = "windows")]
        {
            let _ = mkapk_win32::render_to_hwnd_with_registries(
                view,
                size,
                1.0,
                commands,
                Some(&self.image_registry),
                Some(&self.text_registry),
            );
        }
    }

    fn close(&mut self) {
        #[cfg(target_os = "macos")]
        mkapk_mac::clear_input_sink(self.view);
    }

    fn on_parameter_changed(&mut self, id: ParameterId, value: NormalizedValue) {
        if id == GAIN_PARAM {
            self.sync_widget_values(value);
        }
    }

    fn size_constraints(&self) -> SizeConstraints {
        SizeConstraints::default()
    }

    fn on_mouse_down(&mut self, event: &MouseEvent) -> EventResponse {
        let mut dispatcher = EventDispatcher::new(&self.tree, &self.layout);
        let target = dispatcher.hit_test(event.position);
        let response = dispatcher.dispatch(Event::MouseDown(event.clone()));
        if response == EventResponse::Handled {
            self.mouse_capture = target;
            if let Some(value) = self.gateway.get_normalized(GAIN_PARAM) {
                self.sync_widget_values(value);
            }
        }
        response
    }

    fn on_mouse_move(&mut self, event: &PointerEvent) -> EventResponse {
        // Only dispatch while dragging a captured widget: `Slider`/`Knob`
        // apply their new value unconditionally in `on_mouse_move`, so
        // dispatching on mere hover (no capture) would change the gain
        // just by moving the cursor over the control.
        if self.mouse_capture.is_none() {
            return EventResponse::Bubble;
        }
        let mut dispatcher = EventDispatcher::new(&self.tree, &self.layout);
        dispatcher.set_capture(self.mouse_capture);
        let response = dispatcher.dispatch(Event::MouseMove(event.clone()));
        if response == EventResponse::Handled {
            if let Some(value) = self.gateway.get_normalized(GAIN_PARAM) {
                self.sync_widget_values(value);
            }
        }
        response
    }

    fn on_mouse_up(&mut self, event: &MouseEvent) -> EventResponse {
        let mut dispatcher = EventDispatcher::new(&self.tree, &self.layout);
        dispatcher.set_capture(self.mouse_capture);
        let response = dispatcher.dispatch(Event::MouseUp(event.clone()));
        self.mouse_capture = None;
        response
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;

    use mkapk_core::{
        Event, EventDispatcher, EventResponse, LayoutConstraints, LayoutEngine, LayoutNode,
        Modifiers, MouseButton, MouseEvent, Pointf, Rectf, Tree, Widget,
    };
    use mkapk_host::{
        LockFreeParameterGateway, NormalizedValue, ParameterId, PeakMeter, PluginEditor,
    };
    use mkapk_widgets::{Slider, Theme};

    use super::GainEditor;

    fn test_gateway() -> Arc<LockFreeParameterGateway> {
        Arc::new(LockFreeParameterGateway::default())
    }

    #[test]
    fn slider_mouse_down_invokes_parameter_callback() {
        let slider = Slider::new(ParameterId(1), NormalizedValue::new(0.0), Theme::default());
        let slider_id = slider.id();

        let values: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
        let values_for_cb = values.clone();
        slider.on_changed(move |_id, value| {
            values_for_cb.borrow_mut().push(value.get());
        });

        let mut tree = Tree::new();
        tree.insert(Box::new(slider), None);

        let mut engine = LayoutEngine::new();
        engine.set_node(LayoutNode {
            id: slider_id,
            ..LayoutNode::default()
        });
        let layout = engine.compute(
            &tree,
            LayoutConstraints {
                min_width: Some(100.0),
                max_width: Some(100.0),
                min_height: Some(24.0),
                max_height: Some(24.0),
            },
        );

        let layout_box = layout.get(slider_id).unwrap();
        let slider_node = tree.find(slider_id).unwrap();
        let widget = slider_node.widget.borrow();
        let slider_ref =
            mkapk_core::downcast_widget_ref::<mkapk_widgets::Slider>(&**widget).unwrap();
        slider_ref.set_frame(Rectf::new(layout_box.origin, layout_box.size));
        drop(widget);

        let mut dispatcher = EventDispatcher::new(&tree, &layout);
        let response = dispatcher.dispatch(Event::MouseDown(MouseEvent {
            button: MouseButton::Left,
            position: Pointf::new(50.0, 12.0),
            modifiers: Modifiers::default(),
            click_count: 1,
        }));

        assert_eq!(response, EventResponse::Handled);
        let captured = values.borrow();
        assert_eq!(captured.len(), 1);
        assert!(captured[0] > 0.4 && captured[0] < 0.6);
    }

    #[test]
    fn rebuild_commands_retains_capacity() {
        let mut editor = GainEditor::new(test_gateway(), PeakMeter::new());
        editor.rebuild_commands();
        let first_capacity = editor.commands.capacity();
        editor.rebuild_commands();
        let second_capacity = editor.commands.capacity();
        assert_eq!(
            first_capacity, second_capacity,
            "rebuild_commands should not reallocate across frames"
        );
    }

    #[test]
    fn slider_changes_flow_through_shared_gateway() {
        use mkapk_host::ParameterGateway;

        let gateway = test_gateway();
        let _editor = GainEditor::new(gateway.clone(), PeakMeter::new());

        gateway.set_normalized(ParameterId(1), NormalizedValue::new(0.25));
        assert_eq!(
            gateway.get_normalized(ParameterId(1)),
            Some(NormalizedValue::new(0.25))
        );
    }

    #[test]
    fn update_meter_reflects_real_peak_meter_value() {
        let meter = PeakMeter::new();
        let mut editor = GainEditor::new(test_gateway(), meter.clone());

        meter.write(0.8);
        editor.update_meter();
        assert!(
            editor.meter_value > 0.0,
            "meter should pick up the real level written from the audio thread"
        );
    }

    #[test]
    fn on_mouse_down_on_slider_updates_gain_and_syncs_knob() {
        use mkapk_host::ParameterGateway;

        let gateway = test_gateway();
        let mut editor = GainEditor::new(gateway.clone(), PeakMeter::new());
        editor.resize(mkapk_core::Sizef::new(400.0, 300.0));

        let slider_box = editor.layout.get(editor.slider_id).unwrap();
        let position = Pointf::new(
            slider_box.origin.x + slider_box.size.width * 0.25,
            slider_box.origin.y + slider_box.size.height / 2.0,
        );

        let response = editor.on_mouse_down(&MouseEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
        });

        assert_eq!(response, EventResponse::Handled);
        let value = gateway
            .get_normalized(crate::processor::GAIN_PARAM)
            .expect("slider drag should have written the gain parameter");

        let knob_node = editor.tree.find(editor.knob_id).unwrap();
        let widget = knob_node.widget.borrow();
        let knob = mkapk_core::downcast_widget_ref::<mkapk_widgets::Knob>(&**widget).unwrap();
        assert_eq!(knob.value(), value, "knob should mirror the slider's value");
    }
}
