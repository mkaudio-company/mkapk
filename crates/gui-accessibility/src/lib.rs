//! Accessibility backend trait and platform-specific stubs for exposing the
//! accessibility tree to assistive technologies.
#![no_std]
#![deny(unsafe_code)]

extern crate alloc;

#[cfg(feature = "backend")]
extern crate std;

use alloc::boxed::Box;

use gui_core::AccessibilityTree;

pub trait AccessibilityBackend {
    fn update_tree(&mut self, tree: &AccessibilityTree);
    fn post_notification(&mut self, notification: &str);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopBackend;

impl AccessibilityBackend for NoopBackend {
    fn update_tree(&mut self, _tree: &AccessibilityTree) {}

    fn post_notification(&mut self, _notification: &str) {}
}

#[cfg(all(feature = "backend", target_os = "macos"))]
#[derive(Debug, Clone, Copy, Default)]
pub struct MacBackend;

#[cfg(all(feature = "backend", target_os = "macos"))]
impl AccessibilityBackend for MacBackend {
    fn update_tree(&mut self, tree: &AccessibilityTree) {
        std::println!("{}", tree.root());
    }

    fn post_notification(&mut self, notification: &str) {
        std::println!("{notification}");
    }
}

#[cfg(all(feature = "backend", target_os = "windows"))]
#[derive(Debug, Clone, Copy, Default)]
pub struct WinBackend;

#[cfg(all(feature = "backend", target_os = "windows"))]
impl AccessibilityBackend for WinBackend {
    fn update_tree(&mut self, tree: &AccessibilityTree) {
        std::println!("{}", tree.root());
    }

    fn post_notification(&mut self, notification: &str) {
        std::println!("{notification}");
    }
}

pub fn default_backend() -> Box<dyn AccessibilityBackend> {
    #[cfg(all(feature = "backend", target_os = "macos"))]
    {
        Box::new(MacBackend)
    }

    #[cfg(all(feature = "backend", target_os = "windows"))]
    {
        Box::new(WinBackend)
    }

    #[cfg(not(feature = "backend"))]
    {
        Box::new(NoopBackend)
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;

    use gui_core::{AccessibilityNode, AccessibilityTree, Role, WidgetId};

    use super::{AccessibilityBackend, NoopBackend};

    #[test]
    fn noop_backend_accepts_tree() {
        let tree = AccessibilityTree::new(
            AccessibilityNode::new(WidgetId::new())
                .with_role(Role::Panel)
                .with_child(AccessibilityNode::new(WidgetId::new()).with_role(Role::Slider)),
        );

        let mut backend: Box<dyn AccessibilityBackend> = Box::new(NoopBackend);
        backend.update_tree(&tree);
    }
}
