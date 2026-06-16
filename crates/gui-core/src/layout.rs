use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::{Insetsf, LayoutConstraints, Pointf, Sizef, Tree, WidgetId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutDirection {
    #[default]
    Column,
    Row,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayoutBox {
    pub id: WidgetId,
    pub origin: Pointf,
    pub size: Sizef,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayoutNode {
    pub id: WidgetId,
    pub direction: LayoutDirection,
    pub constraints: LayoutConstraints,
    pub margin: Insetsf,
    pub padding: Insetsf,
    pub flex_grow: f32,
    pub alignment: Alignment,
    pub preferred_size: Option<Sizef>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayoutEngine {
    nodes: BTreeMap<WidgetId, LayoutNode>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayoutResult {
    boxes: BTreeMap<WidgetId, LayoutBox>,
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
        }
    }

    pub fn set_node(&mut self, node: LayoutNode) -> &mut Self {
        self.nodes.insert(node.id, node);
        self
    }

    pub fn node(&self, id: WidgetId) -> Option<&LayoutNode> {
        self.nodes.get(&id)
    }

    pub fn compute(&mut self, tree: &Tree, root_constraints: LayoutConstraints) -> LayoutResult {
        let mut measured = BTreeMap::new();
        let mut result = LayoutResult::default();

        if let Some(root) = tree.root() {
            self.measure(root, tree, root_constraints, &mut measured);
            let root_measured = *measured.get(&root).unwrap_or(&Sizef::default());
            let root_node = self.nodes.get(&root).copied().unwrap_or_default();
            let root_size = clamp_size(
                root_measured,
                intersect_constraints(root_constraints, root_node.constraints),
            );
            self.arrange_node(
                root,
                tree,
                &measured,
                Pointf::zero(),
                root_size,
                &mut result,
            );
        }

        result
    }

    fn measure(
        &self,
        id: WidgetId,
        tree: &Tree,
        constraints: LayoutConstraints,
        measured: &mut BTreeMap<WidgetId, Sizef>,
    ) {
        let node = self.nodes.get(&id).copied().unwrap_or_default();
        let effective = intersect_constraints(constraints, node.constraints);

        let widget_size = tree
            .find(id)
            .map(|n| n.widget.layout(effective))
            .unwrap_or_default();
        let size = clamp_size(node.preferred_size.unwrap_or(widget_size), effective);
        measured.insert(id, size);

        for &child in tree.children_of(id) {
            let child_node = self.nodes.get(&child).copied().unwrap_or_default();
            let child_constraints =
                derive_child_constraints(node.direction, size, node.padding, child_node.margin);
            self.measure(child, tree, child_constraints, measured);
        }
    }

    fn arrange_node(
        &self,
        id: WidgetId,
        tree: &Tree,
        measured: &BTreeMap<WidgetId, Sizef>,
        origin: Pointf,
        allocated_size: Sizef,
        result: &mut LayoutResult,
    ) {
        let node = self.nodes.get(&id).copied().unwrap_or_default();
        let final_size = clamp_size(allocated_size, node.constraints);
        result.boxes.insert(
            id,
            LayoutBox {
                id,
                origin,
                size: final_size,
            },
        );

        let content_origin = Pointf::new(origin.x + node.padding.left, origin.y + node.padding.top);
        let content_size = Sizef::new(
            (final_size.width - node.padding.horizontal()).max(0.0),
            (final_size.height - node.padding.vertical()).max(0.0),
        );

        let children: Vec<WidgetId> = tree.children_of(id).to_vec();
        match node.direction {
            LayoutDirection::Row => {
                self.arrange_row(
                    &children,
                    tree,
                    measured,
                    content_origin,
                    content_size,
                    result,
                );
            }
            LayoutDirection::Column => {
                self.arrange_column(
                    &children,
                    tree,
                    measured,
                    content_origin,
                    content_size,
                    result,
                );
            }
        }
    }

    fn arrange_row(
        &self,
        children: &[WidgetId],
        tree: &Tree,
        measured: &BTreeMap<WidgetId, Sizef>,
        content_origin: Pointf,
        content_size: Sizef,
        result: &mut LayoutResult,
    ) {
        let available_main = content_size.width;
        let mut total_main = 0.0;
        let mut total_flex = 0.0;
        let mut infos = Vec::with_capacity(children.len());

        for &child in children {
            let node = self.nodes.get(&child).copied().unwrap_or_default();
            let measured_size = *measured.get(&child).unwrap_or(&Sizef::default());
            total_main += measured_size.width + node.margin.horizontal();
            total_flex += node.flex_grow;
            infos.push((child, node, measured_size));
        }

        let remaining = (available_main - total_main).max(0.0);
        let mut x = content_origin.x;

        for (child, node, measured_size) in infos {
            let allocated_width = if total_flex > 0.0 && node.flex_grow > 0.0 {
                measured_size.width + remaining * (node.flex_grow / total_flex)
            } else {
                measured_size.width
            };

            let available_cross = (content_size.height - node.margin.vertical()).max(0.0);
            let allocated_height = match node.alignment {
                Alignment::Stretch => available_cross,
                _ => measured_size.height.min(available_cross),
            };

            let y_offset = match node.alignment {
                Alignment::Start => 0.0,
                Alignment::Center => (available_cross - allocated_height) / 2.0,
                Alignment::End => available_cross - allocated_height,
                Alignment::Stretch => 0.0,
            };

            let child_origin = Pointf::new(
                x + node.margin.left,
                content_origin.y + node.margin.top + y_offset,
            );
            let child_size = Sizef::new(allocated_width, allocated_height);

            self.arrange_node(child, tree, measured, child_origin, child_size, result);

            x += allocated_width + node.margin.horizontal();
        }
    }

    fn arrange_column(
        &self,
        children: &[WidgetId],
        tree: &Tree,
        measured: &BTreeMap<WidgetId, Sizef>,
        content_origin: Pointf,
        content_size: Sizef,
        result: &mut LayoutResult,
    ) {
        let available_main = content_size.height;
        let mut total_main = 0.0;
        let mut total_flex = 0.0;
        let mut infos = Vec::with_capacity(children.len());

        for &child in children {
            let node = self.nodes.get(&child).copied().unwrap_or_default();
            let measured_size = *measured.get(&child).unwrap_or(&Sizef::default());
            total_main += measured_size.height + node.margin.vertical();
            total_flex += node.flex_grow;
            infos.push((child, node, measured_size));
        }

        let remaining = (available_main - total_main).max(0.0);
        let mut y = content_origin.y;

        for (child, node, measured_size) in infos {
            let allocated_height = if total_flex > 0.0 && node.flex_grow > 0.0 {
                measured_size.height + remaining * (node.flex_grow / total_flex)
            } else {
                measured_size.height
            };

            let available_cross = (content_size.width - node.margin.horizontal()).max(0.0);
            let allocated_width = match node.alignment {
                Alignment::Stretch => available_cross,
                _ => measured_size.width.min(available_cross),
            };

            let x_offset = match node.alignment {
                Alignment::Start => 0.0,
                Alignment::Center => (available_cross - allocated_width) / 2.0,
                Alignment::End => available_cross - allocated_width,
                Alignment::Stretch => 0.0,
            };

            let child_origin = Pointf::new(
                content_origin.x + node.margin.left + x_offset,
                y + node.margin.top,
            );
            let child_size = Sizef::new(allocated_width, allocated_height);

            self.arrange_node(child, tree, measured, child_origin, child_size, result);

            y += allocated_height + node.margin.vertical();
        }
    }
}

impl LayoutResult {
    pub fn get(&self, id: WidgetId) -> Option<&LayoutBox> {
        self.boxes.get(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&WidgetId, &LayoutBox)> {
        self.boxes.iter()
    }
}

fn intersect_constraints(a: LayoutConstraints, b: LayoutConstraints) -> LayoutConstraints {
    LayoutConstraints {
        min_width: max_option(a.min_width, b.min_width),
        max_width: min_option(a.max_width, b.max_width),
        min_height: max_option(a.min_height, b.min_height),
        max_height: min_option(a.max_height, b.max_height),
    }
}

fn derive_child_constraints(
    direction: LayoutDirection,
    parent_size: Sizef,
    padding: Insetsf,
    margin: Insetsf,
) -> LayoutConstraints {
    let content_width = (parent_size.width - padding.horizontal()).max(0.0);
    let content_height = (parent_size.height - padding.vertical()).max(0.0);

    match direction {
        LayoutDirection::Row => LayoutConstraints {
            min_width: None,
            max_width: Some((content_width - margin.horizontal()).max(0.0)),
            min_height: None,
            max_height: Some((content_height - margin.vertical()).max(0.0)),
        },
        LayoutDirection::Column => LayoutConstraints {
            min_width: None,
            max_width: Some((content_width - margin.horizontal()).max(0.0)),
            min_height: None,
            max_height: Some((content_height - margin.vertical()).max(0.0)),
        },
    }
}

fn clamp_size(size: Sizef, constraints: LayoutConstraints) -> Sizef {
    Sizef::new(
        clamp_value(size.width, constraints.min_width, constraints.max_width),
        clamp_value(size.height, constraints.min_height, constraints.max_height),
    )
}

fn clamp_value(value: f32, min: Option<f32>, max: Option<f32>) -> f32 {
    let mut result = value;
    if let Some(min) = min {
        if result < min {
            result = min;
        }
    }
    if let Some(max) = max {
        if result > max {
            result = max;
        }
    }
    result
}

fn max_option(a: Option<f32>, b: Option<f32>) -> Option<f32> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn min_option(a: Option<f32>, b: Option<f32>) -> Option<f32> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;

    use crate::{Tree, Widget, WidgetId};

    use super::*;

    struct FixedWidget {
        id: WidgetId,
        size: Sizef,
    }

    impl FixedWidget {
        fn new(size: Sizef) -> Self {
            Self {
                id: WidgetId::new(),
                size,
            }
        }
    }

    impl Widget for FixedWidget {
        fn id(&self) -> WidgetId {
            self.id
        }

        fn layout(&self, constraints: LayoutConstraints) -> Sizef {
            clamp_size(self.size, constraints)
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
    fn single_widget_matches_constraints() {
        let mut tree = Tree::new();
        let widget = FixedWidget::new(Sizef::new(50.0, 30.0));
        let id = widget.id();
        tree.insert(Box::new(widget), None);

        let result = LayoutEngine::new().compute(&tree, fixed_constraints(100.0, 80.0));
        let box_ = result.get(id).unwrap();

        assert_eq!(box_.origin, Pointf::zero());
        assert_eq!(box_.size, Sizef::new(100.0, 80.0));
    }

    #[test]
    fn row_lays_out_children_left_to_right() {
        let mut tree = Tree::new();
        let root_widget = FixedWidget::new(Sizef::new(300.0, 100.0));
        let root = root_widget.id();
        tree.insert(Box::new(root_widget), None);

        let sizes = [
            Sizef::new(50.0, 40.0),
            Sizef::new(75.0, 60.0),
            Sizef::new(100.0, 30.0),
        ];
        let mut child_ids = Vec::with_capacity(sizes.len());
        for size in sizes {
            let widget = FixedWidget::new(size);
            child_ids.push(widget.id());
            tree.insert(Box::new(widget), Some(root));
        }

        let mut engine = LayoutEngine::new();
        engine.set_node(LayoutNode {
            id: root,
            direction: LayoutDirection::Row,
            ..LayoutNode::default()
        });

        let result = engine.compute(&tree, fixed_constraints(300.0, 100.0));

        assert_eq!(result.get(root).unwrap().size, Sizef::new(300.0, 100.0));

        let mut x = 0.0;
        for (expected, &id) in sizes.iter().zip(child_ids.iter()) {
            let box_ = result.get(id).unwrap();
            assert_eq!(box_.origin.x, x);
            assert_eq!(box_.origin.y, 0.0);
            assert_eq!(box_.size, *expected);
            x += expected.width;
        }
    }

    #[test]
    fn row_flex_grow_distributes_remaining_space() {
        let mut tree = Tree::new();
        let root_widget = FixedWidget::new(Sizef::new(300.0, 100.0));
        let root = root_widget.id();
        tree.insert(Box::new(root_widget), None);

        let a = FixedWidget::new(Sizef::new(50.0, 40.0));
        let b = FixedWidget::new(Sizef::new(50.0, 40.0));
        let c = FixedWidget::new(Sizef::new(50.0, 40.0));
        let a_id = a.id();
        let b_id = b.id();
        let c_id = c.id();
        tree.insert(Box::new(a), Some(root));
        tree.insert(Box::new(b), Some(root));
        tree.insert(Box::new(c), Some(root));

        let mut engine = LayoutEngine::new();
        engine
            .set_node(LayoutNode {
                id: root,
                direction: LayoutDirection::Row,
                ..LayoutNode::default()
            })
            .set_node(LayoutNode {
                id: a_id,
                flex_grow: 1.0,
                ..LayoutNode::default()
            })
            .set_node(LayoutNode {
                id: b_id,
                flex_grow: 2.0,
                ..LayoutNode::default()
            });

        let result = engine.compute(&tree, fixed_constraints(300.0, 100.0));

        let a_box = result.get(a_id).unwrap();
        let b_box = result.get(b_id).unwrap();
        let c_box = result.get(c_id).unwrap();

        assert_eq!(c_box.size.width, 50.0);
        assert_eq!(a_box.size.width, 100.0);
        assert_eq!(b_box.size.width, 150.0);
        assert_eq!(a_box.origin.x, 0.0);
        assert_eq!(b_box.origin.x, 100.0);
        assert_eq!(c_box.origin.x, 250.0);
    }

    #[test]
    fn column_respects_padding_and_margin() {
        let mut tree = Tree::new();
        let root_widget = FixedWidget::new(Sizef::new(200.0, 200.0));
        let root = root_widget.id();
        tree.insert(Box::new(root_widget), None);

        let child = FixedWidget::new(Sizef::new(50.0, 50.0));
        let child_id = child.id();
        tree.insert(Box::new(child), Some(root));

        let mut engine = LayoutEngine::new();
        engine
            .set_node(LayoutNode {
                id: root,
                direction: LayoutDirection::Column,
                padding: Insetsf::uniform(10.0),
                ..LayoutNode::default()
            })
            .set_node(LayoutNode {
                id: child_id,
                margin: Insetsf::uniform(5.0),
                ..LayoutNode::default()
            });

        let result = engine.compute(&tree, fixed_constraints(200.0, 200.0));

        let child_box = result.get(child_id).unwrap();
        assert_eq!(child_box.origin, Pointf::new(15.0, 15.0));
        assert_eq!(child_box.size, Sizef::new(50.0, 50.0));
    }

    #[test]
    fn row_center_alignment_positions_children_middle() {
        let mut tree = Tree::new();
        let root_widget = FixedWidget::new(Sizef::new(300.0, 100.0));
        let root = root_widget.id();
        tree.insert(Box::new(root_widget), None);

        let child = FixedWidget::new(Sizef::new(50.0, 40.0));
        let child_id = child.id();
        tree.insert(Box::new(child), Some(root));

        let mut engine = LayoutEngine::new();
        engine
            .set_node(LayoutNode {
                id: root,
                direction: LayoutDirection::Row,
                ..LayoutNode::default()
            })
            .set_node(LayoutNode {
                id: child_id,
                alignment: Alignment::Center,
                ..LayoutNode::default()
            });

        let result = engine.compute(&tree, fixed_constraints(300.0, 100.0));

        let child_box = result.get(child_id).unwrap();
        assert_eq!(child_box.origin.x, 0.0);
        assert_eq!(child_box.origin.y, 30.0);
        assert_eq!(child_box.size, Sizef::new(50.0, 40.0));
    }
}
