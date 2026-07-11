#![allow(unexpected_cfgs, deprecated)]

use std::cell::Cell;
use std::ptr::null_mut;

use mkapk_core::{
    CommandList, LayoutConstraints, LayoutDirection, LayoutEngine, LayoutNode, LayoutResult,
    PaintCommand, Rectf, Sizef, TraverseOrder, Tree, Widget, WidgetId, downcast_widget_ref,
};
use mkapk_host::{
    EditorHost, NormalizedValue, ParameterId, ParentWindowHandle, PluginEditor, SizeConstraints,
};
use mkapk_widgets::{Button, Knob, Label, Slider, Theme};

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

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

struct ControlsEditor {
    view: *mut core::ffi::c_void,
    size: Sizef,
    scale: f32,
    tree: Tree,
    layout_engine: LayoutEngine,
    layout: LayoutResult,
    _label: WidgetId,
    _slider: WidgetId,
    _knob: WidgetId,
    _button: WidgetId,
}

impl ControlsEditor {
    fn new() -> Self {
        let theme = Theme::default();
        let mut tree = Tree::new();

        let panel = Panel::new(theme);
        let root = panel.id();
        tree.insert(Box::new(panel), None);

        let label = Label::new("Controls Demo", theme);
        let label_id = label.id();
        tree.insert(Box::new(label), Some(root));

        let slider = Slider::new(ParameterId(1), NormalizedValue::new(0.5), theme);
        let slider_id = slider.id();
        tree.insert(Box::new(slider), Some(root));

        let knob = Knob::new(ParameterId(2), NormalizedValue::new(0.25), theme);
        let knob_id = knob.id();
        tree.insert(Box::new(knob), Some(root));

        let button = Button::new("Click Me", theme);
        let button_id = button.id();
        tree.insert(Box::new(button), Some(root));

        let mut layout_engine = LayoutEngine::new();
        layout_engine.set_node(LayoutNode {
            id: root,
            direction: LayoutDirection::Column,
            padding: theme.padding,
            ..LayoutNode::default()
        });

        let margin = mkapk_core::Insetsf::uniform(4.0);
        for &id in &[label_id, slider_id, knob_id, button_id] {
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
            view: null_mut(),
            size,
            scale: 1.0,
            tree,
            layout_engine,
            layout,
            _label: label_id,
            _slider: slider_id,
            _knob: knob_id,
            _button: button_id,
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
            } else if let Some(knob) = downcast_widget_ref::<Knob>(&**widget) {
                knob.set_frame(frame);
            } else if let Some(button) = downcast_widget_ref::<Button>(&**widget) {
                button.set_frame(frame);
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

#[cfg(target_os = "macos")]
fn backing_scale_for_view(view: *mut core::ffi::c_void) -> f32 {
    use objc::runtime::{BOOL, Object, YES};

    if view.is_null() {
        return 1.0;
    }
    unsafe {
        let view = view as *mut Object;
        let can_draw: BOOL = msg_send![view, canDraw];
        if can_draw != YES {
            return 1.0;
        }
        let window: *mut Object = msg_send![view, window];
        if window.is_null() {
            return 1.0;
        }
        let scale: f64 = msg_send![window, backingScaleFactor];
        scale as f32
    }
}

#[cfg(not(target_os = "macos"))]
fn backing_scale_for_view(_view: *mut core::ffi::c_void) -> f32 {
    1.0
}

impl PluginEditor for ControlsEditor {
    fn open(&mut self, parent: ParentWindowHandle, _host: &dyn EditorHost) {
        if let ParentWindowHandle::Mac(ptr) = parent {
            self.view = ptr;
            self.scale = backing_scale_for_view(ptr);
        }
    }

    fn close(&mut self) {}

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
        mkapk_mac::render_to_view(self.view, self.size, self.scale, &commands);
    }

    fn on_parameter_changed(&mut self, _id: ParameterId, _value: NormalizedValue) {}

    fn size_constraints(&self) -> SizeConstraints {
        SizeConstraints::default()
    }
}

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
            other => eprintln!("warning: unknown argument {other}"),
        }
    }

    mkapk_test_host::run_test_host_with_editor(duration_ms, width, height, ControlsEditor::new());
}
