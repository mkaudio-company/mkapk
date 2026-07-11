use alloc::vec::Vec;

use crate::{LayoutResult, Pointf, Rectf, Tree, WidgetId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyCode(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct MouseEvent {
    pub button: MouseButton,
    pub position: Pointf,
    pub modifiers: Modifiers,
    pub click_count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyEvent {
    pub key_code: KeyCode,
    pub modifiers: Modifiers,
    pub repeat: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PointerEvent {
    pub position: Pointf,
    pub modifiers: Modifiers,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    MouseDown(MouseEvent),
    MouseUp(MouseEvent),
    MouseMove(PointerEvent),
    KeyDown(KeyEvent),
    KeyUp(KeyEvent),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResponse {
    Handled,
    Bubble,
}

pub struct EventDispatcher<'a> {
    tree: &'a Tree,
    layout: &'a LayoutResult,
    capture: Option<WidgetId>,
}

impl<'a> EventDispatcher<'a> {
    pub fn new(tree: &'a Tree, layout: &'a LayoutResult) -> Self {
        Self {
            tree,
            layout,
            capture: None,
        }
    }

    pub fn dispatch(&mut self, event: Event) -> EventResponse {
        match &event {
            Event::MouseDown(_) | Event::MouseUp(_) | Event::MouseMove(_) => {
                self.dispatch_pointer(&event)
            }
            Event::KeyDown(_) | Event::KeyUp(_) => self.dispatch_keyboard(&event),
        }
    }

    pub fn set_capture(&mut self, id: Option<WidgetId>) {
        self.capture = id;
    }

    pub fn capture(&self) -> Option<WidgetId> {
        self.capture
    }

    fn dispatch_pointer(&mut self, event: &Event) -> EventResponse {
        let position = match event {
            Event::MouseDown(e) => e.position,
            Event::MouseUp(e) => e.position,
            Event::MouseMove(e) => e.position,
            _ => return EventResponse::Bubble,
        };

        let target = self.capture.or_else(|| self.hit_test(position));
        let Some(target) = target else {
            return EventResponse::Bubble;
        };

        self.dispatch_along_chain(target, event)
    }

    fn dispatch_keyboard(&mut self, event: &Event) -> EventResponse {
        let Some(target) = self.tree.root() else {
            return EventResponse::Bubble;
        };
        self.dispatch_along_chain(target, event)
    }

    fn dispatch_along_chain(&mut self, target: WidgetId, event: &Event) -> EventResponse {
        let mut chain = Vec::new();
        let mut current = Some(target);
        while let Some(id) = current {
            chain.push(id);
            current = self.tree.find(id).and_then(|n| n.parent);
        }

        for id in chain {
            if let Some(node) = self.tree.find(id) {
                let response = match event {
                    Event::MouseDown(e) => node.widget.borrow_mut().on_mouse_down(e),
                    Event::MouseUp(e) => node.widget.borrow_mut().on_mouse_up(e),
                    Event::MouseMove(e) => node.widget.borrow_mut().on_mouse_move(e),
                    Event::KeyDown(e) => node.widget.borrow_mut().on_key_down(e),
                    Event::KeyUp(e) => node.widget.borrow_mut().on_key_up(e),
                };
                if response == EventResponse::Handled {
                    return EventResponse::Handled;
                }
            }
        }

        EventResponse::Bubble
    }

    /// Finds the deepest widget whose layout box contains `point`, in the
    /// same top-left-origin space `LayoutResult` uses. Exposed so callers
    /// that need to track their own mouse-capture target across separate
    /// `EventDispatcher` instances (one per event, since it borrows the
    /// tree/layout) can find the widget a `MouseDown` landed on without
    /// duplicating hit-testing logic.
    pub fn hit_test(&self, point: Pointf) -> Option<WidgetId> {
        let root = self.tree.root()?;
        self.hit_test_node(root, point)
    }

    fn hit_test_node(&self, id: WidgetId, point: Pointf) -> Option<WidgetId> {
        let layout_box = self.layout.get(id)?;
        let rect = Rectf::new(layout_box.origin, layout_box.size);
        if !rect.contains(point) {
            return None;
        }

        for &child in self.tree.children_of(id) {
            if let Some(hit) = self.hit_test_node(child, point) {
                return Some(hit);
            }
        }

        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use alloc::rc::Rc;
    use core::cell::Cell;

    use crate::{
        Insetsf, LayoutConstraints, LayoutEngine, LayoutNode, LayoutResult, Pointf, Sizef, Tree,
        Widget, WidgetId,
    };

    use super::*;

    struct Counters {
        mouse_down: Cell<u32>,
        mouse_up: Cell<u32>,
        mouse_move: Cell<u32>,
        key_down: Cell<u32>,
        key_up: Cell<u32>,
    }

    impl Counters {
        fn new() -> Rc<Self> {
            Rc::new(Self {
                mouse_down: Cell::new(0),
                mouse_up: Cell::new(0),
                mouse_move: Cell::new(0),
                key_down: Cell::new(0),
                key_up: Cell::new(0),
            })
        }
    }

    struct RecordingWidget {
        id: WidgetId,
        counters: Rc<Counters>,
        mouse_down_response: EventResponse,
    }

    impl RecordingWidget {
        fn new(counters: Rc<Counters>) -> Self {
            Self {
                id: WidgetId::new(),
                counters,
                mouse_down_response: EventResponse::Bubble,
            }
        }

        fn with_mouse_down_response(mut self, response: EventResponse) -> Self {
            self.mouse_down_response = response;
            self
        }
    }

    impl Widget for RecordingWidget {
        fn id(&self) -> WidgetId {
            self.id
        }

        fn on_mouse_down(&mut self, _event: &MouseEvent) -> EventResponse {
            self.counters
                .mouse_down
                .set(self.counters.mouse_down.get() + 1);
            self.mouse_down_response
        }

        fn on_mouse_up(&mut self, _event: &MouseEvent) -> EventResponse {
            self.counters.mouse_up.set(self.counters.mouse_up.get() + 1);
            EventResponse::Bubble
        }

        fn on_mouse_move(&mut self, _event: &PointerEvent) -> EventResponse {
            self.counters
                .mouse_move
                .set(self.counters.mouse_move.get() + 1);
            EventResponse::Bubble
        }

        fn on_key_down(&mut self, _event: &KeyEvent) -> EventResponse {
            self.counters.key_down.set(self.counters.key_down.get() + 1);
            EventResponse::Bubble
        }

        fn on_key_up(&mut self, _event: &KeyEvent) -> EventResponse {
            self.counters.key_up.set(self.counters.key_up.get() + 1);
            EventResponse::Bubble
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

    fn make_mouse_event(x: f32, y: f32) -> Event {
        Event::MouseDown(MouseEvent {
            button: MouseButton::Left,
            position: Pointf::new(x, y),
            modifiers: Modifiers::default(),
            click_count: 1,
        })
    }

    fn build_tree() -> (
        Tree,
        LayoutResult,
        WidgetId,
        WidgetId,
        WidgetId,
        Rc<Counters>,
        Rc<Counters>,
        Rc<Counters>,
    ) {
        let mut tree = Tree::new();

        let root_counters = Counters::new();
        let root_widget = RecordingWidget::new(root_counters.clone());
        let root_id = root_widget.id;
        tree.insert(Box::new(root_widget), None);

        let child_counters = Counters::new();
        let child_widget = RecordingWidget::new(child_counters.clone());
        let child_id = child_widget.id;
        tree.insert(Box::new(child_widget), Some(root_id));

        let leaf_counters = Counters::new();
        let leaf_widget = RecordingWidget::new(leaf_counters.clone());
        let leaf_id = leaf_widget.id;
        tree.insert(Box::new(leaf_widget), Some(child_id));

        let mut engine = LayoutEngine::new();
        engine
            .set_node(LayoutNode {
                id: root_id,
                ..LayoutNode::default()
            })
            .set_node(LayoutNode {
                id: child_id,
                margin: Insetsf::uniform(10.0),
                preferred_size: Some(Sizef::new(80.0, 80.0)),
                ..LayoutNode::default()
            })
            .set_node(LayoutNode {
                id: leaf_id,
                margin: Insetsf::uniform(10.0),
                preferred_size: Some(Sizef::new(30.0, 30.0)),
                ..LayoutNode::default()
            });

        let layout = engine.compute(&tree, fixed_constraints(100.0, 100.0));

        (
            tree,
            layout,
            root_id,
            child_id,
            leaf_id,
            root_counters,
            child_counters,
            leaf_counters,
        )
    }

    #[test]
    fn mouse_down_hits_deepest_leaf() {
        let (
            tree,
            layout,
            _root_id,
            _child_id,
            leaf_id,
            root_counters,
            child_counters,
            leaf_counters,
        ) = build_tree();
        let mut dispatcher = EventDispatcher::new(&tree, &layout);
        let response = dispatcher.dispatch(make_mouse_event(25.0, 25.0));

        assert_eq!(response, EventResponse::Bubble);
        assert_eq!(leaf_counters.mouse_down.get(), 1);
        assert_eq!(child_counters.mouse_down.get(), 1);
        assert_eq!(root_counters.mouse_down.get(), 1);

        let leaf_box = layout.get(leaf_id).unwrap();
        assert_eq!(leaf_box.origin, Pointf::new(20.0, 20.0));
        assert_eq!(leaf_box.size, Sizef::new(30.0, 30.0));
    }

    #[test]
    fn mouse_down_outside_bubbles() {
        let (
            tree,
            layout,
            _root_id,
            _child_id,
            _leaf_id,
            root_counters,
            child_counters,
            leaf_counters,
        ) = build_tree();
        let mut dispatcher = EventDispatcher::new(&tree, &layout);
        let response = dispatcher.dispatch(make_mouse_event(200.0, 200.0));

        assert_eq!(response, EventResponse::Bubble);
        assert_eq!(root_counters.mouse_down.get(), 0);
        assert_eq!(child_counters.mouse_down.get(), 0);
        assert_eq!(leaf_counters.mouse_down.get(), 0);
    }

    #[test]
    fn handled_event_stops_bubbling() {
        let mut tree = Tree::new();

        let root_counters = Counters::new();
        let root_widget = RecordingWidget::new(root_counters.clone());
        let root_id = root_widget.id;
        tree.insert(Box::new(root_widget), None);

        let child_counters = Counters::new();
        let child_widget = RecordingWidget::new(child_counters.clone())
            .with_mouse_down_response(EventResponse::Handled);
        let child_id = child_widget.id;
        tree.insert(Box::new(child_widget), Some(root_id));

        let mut engine = LayoutEngine::new();
        engine
            .set_node(LayoutNode {
                id: root_id,
                ..LayoutNode::default()
            })
            .set_node(LayoutNode {
                id: child_id,
                preferred_size: Some(Sizef::new(50.0, 50.0)),
                ..LayoutNode::default()
            });

        let layout = engine.compute(&tree, fixed_constraints(100.0, 100.0));
        let mut dispatcher = EventDispatcher::new(&tree, &layout);
        let response = dispatcher.dispatch(make_mouse_event(10.0, 10.0));

        assert_eq!(response, EventResponse::Handled);
        assert_eq!(child_counters.mouse_down.get(), 1);
        assert_eq!(root_counters.mouse_down.get(), 0);
    }

    #[test]
    fn keyboard_dispatch_reaches_root() {
        let (
            tree,
            layout,
            _root_id,
            _child_id,
            _leaf_id,
            root_counters,
            _child_counters,
            _leaf_counters,
        ) = build_tree();
        let mut dispatcher = EventDispatcher::new(&tree, &layout);
        let event = Event::KeyDown(KeyEvent {
            key_code: KeyCode(42),
            modifiers: Modifiers::default(),
            repeat: false,
        });
        let response = dispatcher.dispatch(event);

        assert_eq!(response, EventResponse::Bubble);
        assert_eq!(root_counters.key_down.get(), 1);
    }

    #[test]
    fn mouse_capture_redirects_events() {
        let (
            tree,
            layout,
            _root_id,
            child_id,
            leaf_id,
            root_counters,
            child_counters,
            leaf_counters,
        ) = build_tree();
        let mut dispatcher = EventDispatcher::new(&tree, &layout);
        dispatcher.set_capture(Some(child_id));

        let response = dispatcher.dispatch(make_mouse_event(200.0, 200.0));

        assert_eq!(response, EventResponse::Bubble);
        assert_eq!(child_counters.mouse_down.get(), 1);
        assert_eq!(root_counters.mouse_down.get(), 1);
        assert_eq!(leaf_counters.mouse_down.get(), 0);

        dispatcher.set_capture(Some(leaf_id));
        let response = dispatcher.dispatch(make_mouse_event(200.0, 200.0));
        assert_eq!(response, EventResponse::Bubble);
        assert_eq!(leaf_counters.mouse_down.get(), 1);
    }
}
