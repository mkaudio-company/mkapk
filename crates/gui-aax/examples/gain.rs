#![deny(unsafe_code)]

use std::cell::Cell;
use std::sync::Arc;

use gui_core::{
    CommandList, LayoutConstraints, LayoutDirection, LayoutEngine, LayoutNode, LayoutResult,
    PaintCommand, Rectf, Sizef, TraverseOrder, Tree, Widget, WidgetId, downcast_widget_ref,
};
use gui_host::{
    EditorHost, LockFreeParameterGateway, NormalizedValue, ParameterGateway, ParameterId,
    ParentWindowHandle, PluginEditor, SizeConstraints,
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

        let margin = gui_core::Insetsf::uniform(4.0);
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

        let mut editor = Self {
            view: core::ptr::null_mut(),
            size,
            tree,
            layout_engine,
            layout,
            gateway,
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

    fn rebuild_commands(&self) -> CommandList {
        let mut commands = CommandList::new();
        for id in self.tree.traverse(TraverseOrder::PreOrder) {
            if let Some(node) = self.tree.find(id) {
                node.widget.borrow().paint(&mut commands);
            }
        }
        commands
    }
}

impl PluginEditor for GainEditor {
    fn open(&mut self, parent: ParentWindowHandle, _host: &dyn EditorHost) {
        if let ParentWindowHandle::Mac(view) = parent {
            self.view = view;
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
        let commands = self.rebuild_commands();
        let _ = gui_mac::render_to_view(self.view, self.size, 1.0, &commands);
    }

    fn close(&mut self) {}

    fn on_parameter_changed(&mut self, _id: ParameterId, _value: NormalizedValue) {}

    fn size_constraints(&self) -> SizeConstraints {
        SizeConstraints::default()
    }
}

#[cfg(target_os = "windows")]
struct AaxTestHost;

#[cfg(target_os = "windows")]
impl EditorHost for AaxTestHost {
    fn request_resize(&self, _size: Sizef) {}
    fn start_parameter_gesture(&self, _id: ParameterId) {}
    fn end_parameter_gesture(&self, _id: ParameterId) {}
    fn set_parameter_normalized(&self, _id: ParameterId, _value: NormalizedValue) {}
}

#[cfg(target_os = "macos")]
fn main() {
    gui_test_host::run_test_host_with_editor(1000, 400, 300, GainEditor::new());
}

#[cfg(target_os = "windows")]
fn main() {
    use std::time::{Duration, Instant};

    let (window, parent) = gui_test_host::create_host_window(400, 300);
    let host = AaxTestHost;
    let editor = Box::new(GainEditor::new());

    match gui_win32::Win32Window::create(parent, 400, 300, editor, Box::new(host)) {
        Some(_child) => {
            let start = Instant::now();
            let duration = Duration::from_millis(1000);
            while start.elapsed() < duration {
                if !window.pump_events() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(16));
            }
            window.destroy();
        }
        None => {
            println!("AAX stub on Windows");
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn main() {
    println!("This example is only supported on macOS and Windows.");
}
