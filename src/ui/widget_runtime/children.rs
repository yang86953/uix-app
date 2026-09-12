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
//! In your `widget!` struct, include a field:
//!
//! In `build()`:
//!
//! In the manual `impl` block (for builder pattern):

use crate::ui::Widget;
use std::cell::RefCell;

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
    /// 返回当前是否没有存储任何子组件。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
