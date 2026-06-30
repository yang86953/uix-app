//! Children helper — reusable pattern for widgets that own child widgets.
//!
//! Many container widgets (Card, Space, ScrollView) need to store child
//! widgets during builder construction and release them when the widget
//! tree calls `WidgetComponent::build()`. This module provides a common helper
//! that eliminates the duplicated `RefCell<Option<Vec<Box<dyn WidgetComponent>>>>`
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
//! build => (&self) -> Vec<Box<dyn WidgetComponent>> { self.children.take() }
//! ```
//!
//! In the manual `impl` block (for builder pattern):
//! ```ignore
//! pub fn child(self, w: impl WidgetComponent + 'static) -> Self { self.children.add(self, w) }
//! pub fn children(self, widgets: Vec<Box<dyn WidgetComponent>>) -> Self { self.children.set_all(self, widgets) }
//! ```

use std::cell::RefCell;
use crate::widget::{WidgetCore, WidgetId, WidgetTree, WidgetNode};
use crate::widget::WidgetComponent;

/// Stores child widgets during builder construction and releases them
/// on demand (typically from `WidgetComponent::build()`).
///
/// Once children are taken by `take()`, subsequent calls return an empty
/// vec — safe because the widget tree already holds references.
#[derive(Default)]
pub struct WidgetChildren {
    inner: RefCell<Option<Vec<Box<dyn WidgetComponent>>>>,
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
    pub fn take(&self) -> Vec<Box<dyn WidgetComponent>> {
        self.inner.borrow_mut().take().unwrap_or_default()
    }

    /// Add a single child widget. Lazily initializes the storage.
    pub fn add(&self, child: impl WidgetComponent + 'static) {
        self.inner
            .borrow_mut()
            .get_or_insert_with(Vec::new)
            .push(Box::new(child));
    }

    /// Add a boxed child widget. Lazily initializes the storage.
    pub fn add_boxed(&self, child: Box<dyn WidgetComponent>) {
        self.inner
            .borrow_mut()
            .get_or_insert_with(Vec::new)
            .push(child);
    }

    /// Set all children at once, replacing any existing.
    pub fn set_all(&self, children: Vec<Box<dyn WidgetComponent>>) {
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
