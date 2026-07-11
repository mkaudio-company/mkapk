#![deny(unsafe_code)]

use std::cell::Cell;
use std::sync::Arc;

use mkapk_core::{
    CommandList, LayoutConstraints, LayoutDirection, LayoutEngine, LayoutNode, LayoutResult,
    PaintCommand, Rectf, Sizef, TraverseOrder, Tree, Widget, WidgetId, downcast_widget_ref,
};
use mkapk_host::{
    EditorHost, LockFreeParameterGateway, NormalizedValue, ParameterGateway, ParameterId,
    ParentWindowHandle, PluginEditor, SizeConstraints,
};
use mkapk_widgets::{Label, Slider, Theme};

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

        let margin = mkapk_core::Insetsf::uniform(4.0);
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
        let _commands = self.rebuild_commands();
    }

    fn close(&mut self) {}

    fn on_parameter_changed(&mut self, _id: ParameterId, _value: NormalizedValue) {}

    fn size_constraints(&self) -> SizeConstraints {
        SizeConstraints::default()
    }
}

#[cfg(target_os = "macos")]
fn main() {
    run(ParentWindowHandle::Mac(core::ptr::null_mut()));
}

#[cfg(target_os = "windows")]
fn main() {
    run(ParentWindowHandle::Windows(core::ptr::null_mut()));
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn main() {
    println!("This example is only supported on macOS and Windows.");
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn run(parent: ParentWindowHandle) {
    let editor = GainEditor::new();
    let mut aax_editor = mkapk_aax::AaxEditor::new(Box::new(editor));
    aax_editor.create_view(parent).unwrap();
    let _ = aax_editor.view_size();
    aax_editor.timer_wakeup();
    aax_editor.set_parameter(1, 0.5);
    aax_editor.destroy_view();
    println!("AAX example built successfully");
}
