use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::fmt::{self, Display, Formatter};

use crate::{Rectf, WidgetId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Role {
    #[default]
    None,
    Button,
    Slider,
    Label,
    Knob,
    Panel,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct State(u8);

impl State {
    pub const DISABLED: u8 = 1 << 0;
    pub const HIDDEN: u8 = 1 << 1;
    pub const FOCUSED: u8 = 1 << 2;
    pub const CHECKED: u8 = 1 << 3;

    pub const fn empty() -> Self {
        Self(0)
    }

    pub fn disabled(&self) -> bool {
        self.0 & Self::DISABLED != 0
    }

    pub fn hidden(&self) -> bool {
        self.0 & Self::HIDDEN != 0
    }

    pub fn focused(&self) -> bool {
        self.0 & Self::FOCUSED != 0
    }

    pub fn checked(&self) -> bool {
        self.0 & Self::CHECKED != 0
    }

    pub fn set_disabled(&mut self, value: bool) {
        self.set_flag(Self::DISABLED, value);
    }

    pub fn set_hidden(&mut self, value: bool) {
        self.set_flag(Self::HIDDEN, value);
    }

    pub fn set_focused(&mut self, value: bool) {
        self.set_flag(Self::FOCUSED, value);
    }

    pub fn set_checked(&mut self, value: bool) {
        self.set_flag(Self::CHECKED, value);
    }

    fn set_flag(&mut self, flag: u8, value: bool) {
        if value {
            self.0 |= flag;
        } else {
            self.0 &= !flag;
        }
    }
}

impl Display for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            return Ok(());
        }

        let mut first = true;
        let mut write_flag = |name: &str, flag: u8| -> fmt::Result {
            if self.0 & flag != 0 {
                if !first {
                    f.write_str("|")?;
                }
                first = false;
                f.write_str(name)?;
            }
            Ok(())
        };

        write_flag("disabled", Self::DISABLED)?;
        write_flag("hidden", Self::HIDDEN)?;
        write_flag("focused", Self::FOCUSED)?;
        write_flag("checked", Self::CHECKED)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AccessibilityNode {
    pub id: WidgetId,
    pub role: Role,
    pub label: Option<String>,
    pub value: Option<String>,
    pub state: State,
    pub bounds: Rectf,
    pub children: Vec<AccessibilityNode>,
}

impl AccessibilityNode {
    pub fn new(id: WidgetId) -> Self {
        Self {
            id,
            ..Self::default()
        }
    }

    pub fn with_role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn with_state(mut self, state: State) -> Self {
        self.state = state;
        self
    }

    pub fn with_bounds(mut self, bounds: Rectf) -> Self {
        self.bounds = bounds;
        self
    }

    pub fn with_child(mut self, child: AccessibilityNode) -> Self {
        self.children.push(child);
        self
    }
}

impl Display for AccessibilityNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.role)?;

        let mut first = true;
        let mut write_field = |name: &str, value: &str| -> fmt::Result {
            if !first {
                f.write_str(", ")?;
            } else {
                f.write_str("[")?;
            }
            first = false;
            write!(f, "{}={}", name, value)
        };

        if let Some(label) = &self.label {
            write_field("label", label)?;
        }
        if let Some(value) = &self.value {
            write_field("value", value)?;
        }
        if self.state.0 != 0 {
            write_field("state", &self.state.to_string())?;
        }
        if !self.children.is_empty() {
            write_field(
                "children",
                &self
                    .children
                    .iter()
                    .map(|child| child.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            )?;
        }

        if first { Ok(()) } else { f.write_str("]") }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AccessibilityTree {
    root: AccessibilityNode,
}

impl AccessibilityTree {
    pub fn new(root: AccessibilityNode) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &AccessibilityNode {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use alloc::format;
    use alloc::string::ToString;

    use crate::{
        AccessibilityNode, LayoutConstraints, LayoutEngine, LayoutNode, Pointf, Rectf, Role,
        Sizef, State, Tree, Widget, WidgetId,
    };

    struct Panel {
        id: WidgetId,
    }

    impl Panel {
        fn new() -> Self {
            Self {
                id: WidgetId::new(),
            }
        }
    }

    impl Widget for Panel {
        fn id(&self) -> WidgetId {
            self.id
        }

        fn accessibility(&self) -> AccessibilityNode {
            AccessibilityNode::new(self.id).with_role(Role::Panel)
        }
    }

    struct Slider {
        id: WidgetId,
        label: Option<&'static str>,
        value: f64,
    }

    impl Slider {
        fn new(value: f64) -> Self {
            Self {
                id: WidgetId::new(),
                label: None,
                value,
            }
        }

        fn with_label(mut self, label: &'static str) -> Self {
            self.label = Some(label);
            self
        }
    }

    impl Widget for Slider {
        fn id(&self) -> WidgetId {
            self.id
        }

        fn accessibility(&self) -> AccessibilityNode {
            let mut node = AccessibilityNode::new(self.id).with_role(Role::Slider);
            if let Some(label) = self.label {
                node.label = Some(label.into());
            }
            node.value = Some(format!("{:.0}%", self.value * 100.0));
            node
        }
    }

    struct Label {
        id: WidgetId,
        text: &'static str,
    }

    impl Label {
        fn new(text: &'static str) -> Self {
            Self {
                id: WidgetId::new(),
                text,
            }
        }
    }

    impl Widget for Label {
        fn id(&self) -> WidgetId {
            self.id
        }

        fn accessibility(&self) -> AccessibilityNode {
            AccessibilityNode::new(self.id)
                .with_role(Role::Label)
                .with_label(self.text)
        }
    }

    fn fixed_constraints(width: f32, height: f32) -> LayoutConstraints {
        LayoutConstraints {
            min_width: Some(width),
            max_width: Some(width),
            min_height: Some(height),
            max_height: Some(height),
        }
    }

    #[test]
    fn state_flags_can_be_set_and_queried() {
        let mut state = State::default();
        assert!(!state.disabled());
        state.set_disabled(true);
        assert!(state.disabled());
        state.set_disabled(false);
        assert!(!state.disabled());
    }

    #[test]
    fn node_display_includes_role_label_and_value() {
        let node = AccessibilityNode::new(WidgetId(1))
            .with_role(Role::Slider)
            .with_label("Gain")
            .with_value("50%");
        assert_eq!(node.to_string(), "Slider[label=Gain, value=50%]");
    }

    #[test]
    fn accessibility_tree_contains_roles_and_labels() {
        let mut tree = Tree::new();

        let panel = Panel::new();
        let root = panel.id();
        tree.insert(Box::new(panel), None);

        let slider = Slider::new(0.75).with_label("Gain");
        let slider_id = slider.id();
        tree.insert(Box::new(slider), Some(root));

        let label = Label::new("Master");
        let label_id = label.id();
        tree.insert(Box::new(label), Some(root));

        let mut engine = LayoutEngine::new();
        engine
            .set_node(LayoutNode {
                id: root,
                ..LayoutNode::default()
            })
            .set_node(LayoutNode {
                id: slider_id,
                preferred_size: Some(Sizef::new(100.0, 24.0)),
                ..LayoutNode::default()
            })
            .set_node(LayoutNode {
                id: label_id,
                preferred_size: Some(Sizef::new(60.0, 20.0)),
                ..LayoutNode::default()
            });
        let layout = engine.compute(&tree, fixed_constraints(200.0, 100.0));
        tree.set_layout_result(layout);

        let a11y = tree.accessibility_tree();
        let serialized = a11y.root().to_string();

        assert!(serialized.contains("Panel"));
        assert!(serialized.contains("Slider"));
        assert!(serialized.contains("Label"));
        assert!(serialized.contains("label=Gain"));
        assert!(serialized.contains("label=Master"));
        assert!(serialized.contains("value=75%"));

        let slider_bounds = a11y
            .root()
            .children
            .iter()
            .find(|child| child.role == Role::Slider)
            .map(|child| child.bounds)
            .unwrap();
        assert_eq!(
            slider_bounds,
            Rectf::new(Pointf::zero(), Sizef::new(100.0, 24.0))
        );
    }

    #[test]
    fn accessibility_tree_uses_zero_bounds_without_layout() {
        let mut tree = Tree::new();
        let root = tree.insert(Box::new(AccessibilityTestWidget::new(Role::Panel)), None);
        tree.insert(
            Box::new(AccessibilityTestWidget::new(Role::Button)),
            Some(root),
        );

        let a11y = tree.accessibility_tree();
        assert_eq!(a11y.root().bounds, Rectf::default());
        assert_eq!(a11y.root().children.len(), 1);
        assert_eq!(a11y.root().children[0].bounds, Rectf::default());
    }

    struct AccessibilityTestWidget {
        id: WidgetId,
        role: Role,
    }

    impl AccessibilityTestWidget {
        fn new(role: Role) -> Self {
            Self {
                id: WidgetId::new(),
                role,
            }
        }
    }

    impl Widget for AccessibilityTestWidget {
        fn id(&self) -> WidgetId {
            self.id
        }

        fn accessibility(&self) -> AccessibilityNode {
            AccessibilityNode::new(self.id).with_role(self.role)
        }
    }
}
