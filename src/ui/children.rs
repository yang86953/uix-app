//! Children helper — reusable pattern for widgets that own child widgets.
//!
//! Many container widgets (Card, Space, ScrollView) need to store child
//! widgets during builder construction and release them when the widget
//! tree calls `Widget::build()`. This module provides a common helper
//! that eliminates the duplicated `RefCell<Option<Vec<Box<dyn Widget>>>>`
//! pattern.
//!
//! # Usage
//!
//! In your `define_widget!` struct, include a field:
//! ```ignore
//! children: WidgetChildren,
//! ```
//!
//! In `build()`:
//! ```ignore
//! build => (&self) -> Vec<Box<dyn Widget>> { self.children.take() }
//! ```
//!
//! In the manual `impl` block (for builder pattern):
//! ```ignore
//! pub fn child(self, w: impl Widget + 'static) -> Self { self.children.add(self, w) }
//! pub fn children(self, widgets: Vec<Box<dyn Widget>>) -> Self { self.children.set_all(self, widgets) }
//! ```

use std::cell::RefCell;
use crate::ui::widget::Widget;

/// Stores child widgets during builder construction and releases them
/// on demand (typically from `Widget::build()`).
///
/// Once children are taken by `take()`, subsequent calls return an empty
/// vec — safe because the widget tree already holds references.
#[derive(Default)]
pub struct WidgetChildren {
    inner: RefCell<Option<Vec<Box<dyn Widget>>>>,
}

impl WidgetChildren {
    /// Create empty children storage.
    pub fn new() -> Self {
        Self {
            inner: RefCell::new(None),
        }
    }

    /// Take the stored children (used in `build()`).
    ///
    /// Returns an empty vec if no children were set or if children
    /// were already taken (idempotent).
    pub fn take(&self) -> Vec<Box<dyn Widget>> {
        self.inner.borrow_mut().take().unwrap_or_default()
    }

    /// Add a single child widget. Lazily initializes the storage.
    pub fn add(&self, child: impl Widget + 'static) {
        self.inner
            .borrow_mut()
            .get_or_insert_with(Vec::new)
            .push(Box::new(child));
    }

    /// Add a boxed child widget. Lazily initializes the storage.
    pub fn add_boxed(&self, child: Box<dyn Widget>) {
        self.inner
            .borrow_mut()
            .get_or_insert_with(Vec::new)
            .push(child);
    }

    /// Set all children at once, replacing any existing.
    pub fn set_all(&self, children: Vec<Box<dyn Widget>>) {
        *self.inner.borrow_mut() = Some(children);
    }

    /// Check whether children have been set.
    pub fn is_set(&self) -> bool {
        self.inner.borrow().is_some()
    }

    /// Return the number of stored children (0 if not yet set).
    pub fn len(&self) -> usize {
        self.inner.borrow().as_ref().map_or(0, |v| v.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::{Rect, Size};
    use crate::ui::render_context::RenderContext;
    use crate::ui::widget::WidgetTree;

    struct Dummy;
    impl Widget for Dummy {
        fn render(&self, _: Rect, _: &mut RenderContext, _: &WidgetTree) {}
        fn preferred_size(&self, _: Option<&dyn crate::graphics::GraphicsEngine>) -> Size { Size::zero() }
    }

    #[test]
    fn children_new_is_empty() {
        let c = WidgetChildren::new();
        assert!(!c.is_set());
        assert_eq!(c.len(), 0);
        assert!(c.take().is_empty());
    }

    #[test]
    fn children_add_and_take() {
        let c = WidgetChildren::new();
        c.add(Dummy);
        assert!(c.is_set());
        assert_eq!(c.len(), 1);
        let taken = c.take();
        assert_eq!(taken.len(), 1);
        // Second take returns empty
        assert!(c.take().is_empty());
    }

    #[test]
    fn children_set_all_and_take() {
        let c = WidgetChildren::new();
        c.set_all(vec![Box::new(Dummy), Box::new(Dummy)]);
        assert_eq!(c.len(), 2);
        let taken = c.take();
        assert_eq!(taken.len(), 2);
    }

    #[test]
    fn children_add_boxed() {
        let c = WidgetChildren::new();
        c.add_boxed(Box::new(Dummy));
        assert!(c.is_set());
        assert_eq!(c.len(), 1);
    }
}
