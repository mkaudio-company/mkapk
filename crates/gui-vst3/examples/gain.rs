#![deny(unsafe_code)]

use std::cell::Cell;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gui_core::{
    Animation, AnimationController, AnimationCurve, AnimationEvent, AnimationId, Color,
    CommandList, ImageId, Insetsf, LayoutConstraints, LayoutDirection, LayoutEngine, LayoutNode,
    LayoutResult, PaintCommand, Pointf, Rectf, Sizef, TraverseOrder, Tree, Widget, WidgetId,
    downcast_widget_ref,
};
use gui_host::{
    EditorHost, LockFreeParameterGateway, NormalizedValue, ParameterGateway, ParameterId,
    ParentWindowHandle, PluginEditor, SizeConstraints,
};
use gui_res::{
    PngImage, ResourceHandle, ResourceId, ResourceRegistry, SvgImage, generated::EMBEDDED,
};
use gui_widgets::{Label, Slider, Theme};

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

struct GainEditor {
    view: *mut core::ffi::c_void,
    size: Sizef,
    tree: Tree,
    layout_engine: LayoutEngine,
    layout: LayoutResult,
    #[allow(dead_code)]
    gateway: Arc<LockFreeParameterGateway>,
    commands: CommandList,
    animation_controller: AnimationController<f32>,
    peak_meter_id: AnimationId,
    peak_meter_value: f32,
    peak_meter_direction: bool,
    last_frame: Option<Instant>,
    knob_handle: ResourceHandle<SvgImage>,
    logo_handle: ResourceHandle<PngImage>,
    _panel: WidgetId,
    _label: WidgetId,
    _slider: WidgetId,
}

impl GainEditor {
    fn new() -> Self {
        let theme = Theme::default();
        let mut tree = Tree::new();

        let panel = Panel::new(theme);
        let root = panel.id();
        tree.insert(Box::new(panel), None);

        let label = Label::new("Gain", theme);
        let label_id = label.id();
        tree.insert(Box::new(label), Some(root));

        let parameter_id = ParameterId(1);
        let slider = Slider::new(parameter_id, NormalizedValue::new(0.5), theme);
        let slider_id = slider.id();

        let gateway = Arc::new(LockFreeParameterGateway::new(256));
        let gateway_for_slider = gateway.clone();
        slider.on_changed(move |id, value| {
            gateway_for_slider.set_normalized(id, value);
            let db = if value.get() > 0.0 {
                20.0 * value.get().log10()
            } else {
                f64::NEG_INFINITY
            };
            println!(
                "parameter {} changed to {:.4} ({:.1} dB)",
                id.0,
                value.get(),
                db
            );
        });
        tree.insert(Box::new(slider), Some(root));

        let mut layout_engine = LayoutEngine::new();
        layout_engine.set_node(LayoutNode {
            id: root,
            direction: LayoutDirection::Column,
            padding: theme.padding,
            ..LayoutNode::default()
        });

        let margin = Insetsf::uniform(4.0);
        for &id in &[label_id, slider_id] {
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

        let mut animation_controller = AnimationController::<f32>::new();
        let peak_meter_id = animation_controller.start(
            Animation::new(0.0_f32, 1.0_f32, Duration::from_millis(750))
                .with_curve(AnimationCurve::EaseInOut),
        );

        let mut registry = ResourceRegistry::new();
        EMBEDDED.register_with(&mut registry);
        let knob_handle = registry
            .load::<SvgImage>(ResourceId::from_bytes_le(b"knob.svg"))
            .expect("knob.svg must be embedded");
        let logo_handle = registry
            .load::<PngImage>(ResourceId::from_bytes_le(b"logo.png"))
            .expect("logo.png must be embedded");

        let mut editor = Self {
            view: core::ptr::null_mut(),
            size,
            tree,
            layout_engine,
            layout,
            gateway,
            commands: CommandList::with_capacity(32),
            animation_controller,
            peak_meter_id,
            peak_meter_value: 0.0,
            peak_meter_direction: true,
            last_frame: None,
            knob_handle,
            logo_handle,
            _panel: root,
            _label: label_id,
            _slider: slider_id,
        };
        editor.apply_layout();
        editor
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
            }
        }
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

    fn update_animation(&mut self) {
        let now = Instant::now();
        let dt = self
            .last_frame
            .map(|t| now.duration_since(t))
            .unwrap_or(Duration::from_millis(16));
        self.last_frame = Some(now);

        let mut peak_value = self.peak_meter_value;
        let mut completed = false;
        self.animation_controller.tick(dt, |event| match event {
            AnimationEvent::Value { value, .. } => {
                peak_value = value;
            }
            AnimationEvent::Completed { value, .. } => {
                peak_value = value;
                completed = true;
            }
        });
        self.peak_meter_value = peak_value;

        if completed {
            let (from, to) = if self.peak_meter_direction {
                (1.0_f32, 0.0_f32)
            } else {
                (0.0_f32, 1.0_f32)
            };
            self.peak_meter_id = self.animation_controller.start(
                Animation::new(from, to, Duration::from_millis(750))
                    .with_curve(AnimationCurve::EaseInOut),
            );
            self.peak_meter_direction = !self.peak_meter_direction;
        }
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

        let bar_height = meter_height * self.peak_meter_value;
        self.commands.push(PaintCommand::FillRect {
            rect: Rectf::new(
                Pointf::new(x, y + meter_height - bar_height),
                Sizef::new(meter_width, bar_height),
            ),
            color: Color::from_rgb(0, 200, 100),
        });
    }

    fn draw_assets(&mut self) {
        let knob_size = Sizef::new(
            self.knob_handle.width() as f32,
            self.knob_handle.height() as f32,
        );
        let logo_size = Sizef::new(
            self.logo_handle.width() as f32,
            self.logo_handle.height() as f32,
        );

        self.commands.push(PaintCommand::DrawImage {
            rect: Rectf::new(Pointf::new(20.0, 80.0), knob_size),
            image: ImageId(1),
        });
        self.commands.push(PaintCommand::DrawImage {
            rect: Rectf::new(Pointf::new(20.0, 20.0), logo_size),
            image: ImageId(2),
        });
    }
}

impl PluginEditor for GainEditor {
    fn open(&mut self, parent: ParentWindowHandle, _host: &dyn EditorHost) {
        match parent {
            ParentWindowHandle::Mac(view) | ParentWindowHandle::Windows(view) => {
                self.view = view;
            }
        }
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
    }

    fn idle(&mut self) {
        self.update_animation();
        self.rebuild_commands();

        let view = self.view;
        let size = self.size;
        let commands = &self.commands;

        #[cfg(target_os = "macos")]
        {
            let _ = gui_mac::render_to_view(view, size, 1.0, commands);
        }

        #[cfg(target_os = "windows")]
        {
            let _ = gui_win32::render_to_hwnd(view, size, 1.0, commands);
        }
    }

    fn close(&mut self) {}

    fn on_parameter_changed(&mut self, _id: ParameterId, _value: NormalizedValue) {}

    fn size_constraints(&self) -> SizeConstraints {
        SizeConstraints::default()
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn main() {
    let mut duration_ms = 1000;
    let mut width = 400;
    let mut height = 300;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--duration-ms" => {
                duration_ms = args
                    .next()
                    .expect("--duration-ms requires a value")
                    .parse()
                    .expect("duration must be a number");
            }
            "--width" => {
                width = args
                    .next()
                    .expect("--width requires a value")
                    .parse()
                    .expect("width must be a number");
            }
            "--height" => {
                height = args
                    .next()
                    .expect("--height requires a value")
                    .parse()
                    .expect("height must be a number");
            }
            "--test-host" => {}
            other => eprintln!("warning: unknown argument {other}"),
        }
    }

    gui_test_host::run_test_host_with_editor(duration_ms, width, height, GainEditor::new());
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn main() {
    println!("This example is only supported on macOS and Windows.");
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Duration;

    use gui_core::{
        Animation, AnimationController, AnimationCurve, AnimationEvent, Event, EventDispatcher,
        EventResponse, LayoutConstraints, LayoutEngine, LayoutNode, Modifiers, MouseButton,
        MouseEvent, Pointf, Rectf, Tree, Widget,
    };
    use gui_host::{NormalizedValue, ParameterId};
    use gui_widgets::{Slider, Theme};

    use super::GainEditor;

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
        let slider_ref = gui_core::downcast_widget_ref::<gui_widgets::Slider>(&**widget).unwrap();
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
    fn peak_meter_animation_updates_across_two_frames() {
        let mut controller = AnimationController::<f32>::new();
        let _id = controller.start(
            Animation::new(0.0_f32, 1.0_f32, Duration::from_millis(1500))
                .with_curve(AnimationCurve::EaseInOut),
        );

        let mut first_value = 0.0_f32;
        controller.tick(Duration::from_millis(16), |event| {
            if let AnimationEvent::Value { value, .. } = event {
                first_value = value;
            }
        });

        let mut second_value = first_value;
        controller.tick(Duration::from_millis(16), |event| {
            if let AnimationEvent::Value { value, .. } = event {
                second_value = value;
            }
        });

        assert!(
            second_value > first_value,
            "animation should advance between two frames"
        );
    }

    #[test]
    fn rebuild_commands_retains_capacity() {
        let mut editor = GainEditor::new();
        editor.rebuild_commands();
        let first_capacity = editor.commands.capacity();
        editor.rebuild_commands();
        let second_capacity = editor.commands.capacity();
        assert_eq!(
            first_capacity, second_capacity,
            "rebuild_commands should not reallocate across frames"
        );
    }
}
