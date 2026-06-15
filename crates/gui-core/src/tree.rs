use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::{Widget, WidgetId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraverseOrder {
    PreOrder,
    PostOrder,
    BreadthFirst,
}

pub struct Node {
    pub widget: Box<dyn Widget>,
    pub parent: Option<WidgetId>,
    pub children: Vec<WidgetId>,
}

pub struct Tree {
    nodes: BTreeMap<WidgetId, Node>,
    root: Option<WidgetId>,
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

impl Tree {
    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            root: None,
        }
    }

    pub fn insert(&mut self, widget: Box<dyn Widget>, parent: Option<WidgetId>) -> WidgetId {
        let id = widget.id();
        if self.root.is_none() {
            self.root = Some(id);
        }
        if let Some(parent_id) = parent {
            if let Some(parent_node) = self.nodes.get_mut(&parent_id) {
                parent_node.children.push(id);
            }
        }
        self.nodes.insert(
            id,
            Node {
                widget,
                parent,
                children: Vec::new(),
            },
        );
        if let Some(node) = self.nodes.get_mut(&id) {
            node.widget.mount();
        }
        id
    }

    pub fn remove(&mut self, id: WidgetId) -> Option<Box<dyn Widget>> {
        let children: Vec<WidgetId> = self.nodes.get(&id)?.children.clone();
        for child in children {
            self.remove(child);
        }
        if let Some(node) = self.nodes.get(&id) {
            if let Some(parent_id) = node.parent {
                if let Some(parent_node) = self.nodes.get_mut(&parent_id) {
                    parent_node.children.retain(|&child_id| child_id != id);
                }
            }
        }
        if self.root == Some(id) {
            self.root = None;
        }
        let mut node = self.nodes.remove(&id)?;
        node.widget.unmount();
        Some(node.widget)
    }

    pub fn find(&self, id: WidgetId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn find_mut(&mut self, id: WidgetId) -> Option<&mut Node> {
        self.nodes.get_mut(&id)
    }

    pub fn root(&self) -> Option<WidgetId> {
        self.root
    }

    pub fn children_of(&self, id: WidgetId) -> &[WidgetId] {
        match self.nodes.get(&id) {
            Some(node) => &node.children,
            None => &[],
        }
    }

    pub fn traverse(&self, order: TraverseOrder) -> Vec<WidgetId> {
        let mut result = Vec::new();
        match order {
            TraverseOrder::PreOrder => {
                if let Some(root) = self.root {
                    self.traverse_pre(root, &mut result);
                }
            }
            TraverseOrder::PostOrder => {
                if let Some(root) = self.root {
                    self.traverse_post(root, &mut result);
                }
            }
            TraverseOrder::BreadthFirst => {
                if let Some(root) = self.root {
                    let mut queue = Vec::new();
                    queue.push(root);
                    let mut i = 0;
                    while i < queue.len() {
                        let id = queue[i];
                        i += 1;
                        result.push(id);
                        if let Some(node) = self.nodes.get(&id) {
                            for &child in &node.children {
                                queue.push(child);
                            }
                        }
                    }
                }
            }
        }
        result
    }

    pub fn unique_id(&self, id: WidgetId) -> bool {
        let node_count = self.nodes.keys().filter(|&&key| key == id).count();
        let child_ref_count = self
            .nodes
            .values()
            .flat_map(|node| node.children.iter())
            .filter(|&&child_id| child_id == id)
            .count();
        node_count == 1 && child_ref_count <= 1
    }

    fn traverse_pre(&self, id: WidgetId, result: &mut Vec<WidgetId>) {
        result.push(id);
        if let Some(node) = self.nodes.get(&id) {
            for &child in &node.children {
                self.traverse_pre(child, result);
            }
        }
    }

    fn traverse_post(&self, id: WidgetId, result: &mut Vec<WidgetId>) {
        if let Some(node) = self.nodes.get(&id) {
            for &child in &node.children {
                self.traverse_post(child, result);
            }
        }
        result.push(id);
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use alloc::rc::Rc;
    use alloc::vec;
    use core::cell::Cell;

    use super::*;

    struct TestWidget {
        id: WidgetId,
        mounted: Rc<Cell<bool>>,
        unmounted: Rc<Cell<bool>>,
    }

    impl TestWidget {
        fn new() -> Self {
            Self {
                id: WidgetId::new(),
                mounted: Rc::new(Cell::new(false)),
                unmounted: Rc::new(Cell::new(false)),
            }
        }
    }

    impl Widget for TestWidget {
        fn id(&self) -> WidgetId {
            self.id
        }

        fn mount(&mut self) {
            self.mounted.set(true);
        }

        fn unmount(&mut self) {
            self.unmounted.set(true);
        }
    }

    #[test]
    fn insert_root_and_children() {
        let mut tree = Tree::new();
        let root_widget = TestWidget::new();
        let root_mounted = root_widget.mounted.clone();
        let root = tree.insert(Box::new(root_widget), None);

        let child1_widget = TestWidget::new();
        let child1_mounted = child1_widget.mounted.clone();
        let child1 = tree.insert(Box::new(child1_widget), Some(root));

        let child2_widget = TestWidget::new();
        let child2_mounted = child2_widget.mounted.clone();
        let child2 = tree.insert(Box::new(child2_widget), Some(root));

        assert_eq!(tree.root(), Some(root));
        assert_eq!(tree.children_of(root), &[child1, child2]);
        assert_eq!(tree.find(child1).unwrap().parent, Some(root));
        assert_eq!(tree.find(child2).unwrap().parent, Some(root));

        assert!(root_mounted.get());
        assert!(child1_mounted.get());
        assert!(child2_mounted.get());
    }

    #[test]
    fn remove_child_calls_unmount_and_removes_descendants() {
        let mut tree = Tree::new();
        let root = tree.insert(Box::new(TestWidget::new()), None);

        let child_widget = TestWidget::new();
        let child_unmounted = child_widget.unmounted.clone();
        let child = tree.insert(Box::new(child_widget), Some(root));

        let grandchild_widget = TestWidget::new();
        let grandchild_unmounted = grandchild_widget.unmounted.clone();
        let grandchild = tree.insert(Box::new(grandchild_widget), Some(child));

        let removed = tree.remove(child);
        assert!(removed.is_some());
        assert!(child_unmounted.get());
        assert!(grandchild_unmounted.get());
        assert!(tree.find(child).is_none());
        assert!(tree.find(grandchild).is_none());
        assert!(!tree.children_of(root).contains(&child));
    }

    #[test]
    fn traverse_orders() {
        let mut tree = Tree::new();
        let root = tree.insert(Box::new(TestWidget::new()), None);
        let a = tree.insert(Box::new(TestWidget::new()), Some(root));
        let b = tree.insert(Box::new(TestWidget::new()), Some(root));
        let a1 = tree.insert(Box::new(TestWidget::new()), Some(a));
        let a2 = tree.insert(Box::new(TestWidget::new()), Some(a));

        assert_eq!(
            tree.traverse(TraverseOrder::PreOrder),
            vec![root, a, a1, a2, b]
        );
        assert_eq!(
            tree.traverse(TraverseOrder::BreadthFirst),
            vec![root, a, b, a1, a2]
        );
        assert_eq!(
            tree.traverse(TraverseOrder::PostOrder),
            vec![a1, a2, a, b, root]
        );
    }

    #[test]
    fn unique_ids() {
        let mut tree = Tree::new();
        let root = tree.insert(Box::new(TestWidget::new()), None);
        let child = tree.insert(Box::new(TestWidget::new()), Some(root));

        assert!(tree.unique_id(root));
        assert!(tree.unique_id(child));
        assert!(!tree.unique_id(WidgetId::default()));
    }
}
