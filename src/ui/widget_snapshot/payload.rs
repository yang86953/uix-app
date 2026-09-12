//! Type-erased component state with framework-owned semantic projections.
use std::{any::Any, fmt::Debug, sync::Arc};
use super::{AccessibilityRole, AccessibilitySnapshot, SelectionSnapshot, SnapshotField};
use crate::ui::event::WindowAction;

/// A component library owns this model and its comparison semantics.
pub trait SnapshotModel: Any + Debug + PartialEq + Send + Sync {
    fn accessibility(&self) -> AccessibilitySnapshot {
        AccessibilitySnapshot::new(AccessibilityRole::Generic)
    }
    fn selection(&self) -> Option<SelectionSnapshot> { None }
    fn text(&self) -> Option<String> { None }
    fn placeholder(&self) -> Option<String> { None }
    fn disabled(&self) -> Option<bool> { None }
    fn checked(&self) -> Option<bool> { None }
    fn numeric_value(&self) -> Option<f64> { None }
    fn invoke_window_action(&self) -> Option<WindowAction> { None }
    fn config_changed(&self, next: &Self) -> bool { self != next }
    fn layout_changed(&self, _next: &Self) -> bool { true }
}

trait ErasedSnapshot: Debug + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn equals(&self, next: &dyn ErasedSnapshot) -> bool;
    fn config_changed(&self, next: &dyn ErasedSnapshot) -> bool;
    fn layout_changed(&self, next: &dyn ErasedSnapshot) -> bool;
    fn accessibility(&self) -> AccessibilitySnapshot;
    fn selection(&self) -> Option<SelectionSnapshot>;
    fn text(&self) -> Option<String>;
    fn placeholder(&self) -> Option<String>;
    fn disabled(&self) -> Option<bool>;
    fn checked(&self) -> Option<bool>;
    fn numeric_value(&self) -> Option<f64>;
    fn invoke_window_action(&self) -> Option<WindowAction>;
}
impl<T: SnapshotModel> ErasedSnapshot for T {
    fn as_any(&self) -> &dyn Any { self }
    fn equals(&self, next: &dyn ErasedSnapshot) -> bool {
        next.as_any().downcast_ref::<Self>().is_some_and(|next| self == next)
    }
    fn config_changed(&self, next: &dyn ErasedSnapshot) -> bool {
        next.as_any().downcast_ref::<Self>().is_none_or(|next| SnapshotModel::config_changed(self, next))
    }
    fn layout_changed(&self, next: &dyn ErasedSnapshot) -> bool {
        next.as_any().downcast_ref::<Self>().is_none_or(|next| SnapshotModel::layout_changed(self, next))
    }
    fn accessibility(&self) -> AccessibilitySnapshot { SnapshotModel::accessibility(self) }
    fn selection(&self) -> Option<SelectionSnapshot> { SnapshotModel::selection(self) }
    fn text(&self) -> Option<String> { SnapshotModel::text(self) }
    fn placeholder(&self) -> Option<String> { SnapshotModel::placeholder(self) }
    fn disabled(&self) -> Option<bool> { SnapshotModel::disabled(self) }
    fn checked(&self) -> Option<bool> { SnapshotModel::checked(self) }
    fn numeric_value(&self) -> Option<f64> { SnapshotModel::numeric_value(self) }
    fn invoke_window_action(&self) -> Option<WindowAction> { SnapshotModel::invoke_window_action(self) }
}

/// A snapshot can cross the framework boundary without importing a concrete control.
#[derive(Clone, Debug)]
pub struct WidgetSnapshotFields {
    data: Option<Arc<dyn ErasedSnapshot>>,
}
impl PartialEq for WidgetSnapshotFields {
    fn eq(&self, other: &Self) -> bool {
        match (&self.data, &other.data) {
            (Some(a), Some(b)) => a.equals(b.as_ref()),
            (None, None) => true,
            _ => false,
        }
    }
}
impl WidgetSnapshotFields {
    pub const UNKNOWN: Self = Self { data: None };
    pub fn new<T: SnapshotModel>(value: T) -> Self { Self { data: Some(Arc::new(value)) } }
    pub fn custom(widget: &'static str, fields: Vec<SnapshotField>) -> Self {
        Self::new(CustomSnapshot { widget, fields })
    }
    pub fn downcast_ref<T: SnapshotModel>(&self) -> Option<&T> {
        self.data.as_ref()?.as_any().downcast_ref()
    }
    pub fn is_unknown(&self) -> bool { self.data.is_none() }
    pub fn accessibility(&self) -> AccessibilitySnapshot {
        self.data.as_ref().map(|data| data.accessibility())
            .unwrap_or_else(|| AccessibilitySnapshot::new(AccessibilityRole::Generic))
    }
    pub fn selection(&self) -> Option<SelectionSnapshot> { self.data.as_ref()?.selection() }
    pub fn text(&self) -> Option<String> { self.data.as_ref()?.text() }
    pub fn placeholder(&self) -> Option<String> { self.data.as_ref()?.placeholder() }
    pub fn disabled(&self) -> Option<bool> { self.data.as_ref()?.disabled() }
    pub fn checked(&self) -> Option<bool> { self.data.as_ref()?.checked() }
    pub fn numeric_value(&self) -> Option<f64> { self.data.as_ref()?.numeric_value() }
    pub fn invoke_window_action(&self) -> Option<WindowAction> { self.data.as_ref()?.invoke_window_action() }
    pub fn config_changed(&self, next: &Self) -> bool {
        match (&self.data, &next.data) {
            (Some(a), Some(b)) => a.config_changed(b.as_ref()),
            _ => true,
        }
    }
    pub fn layout_changed(&self, next: &Self) -> bool {
        match (&self.data, &next.data) {
            (Some(a), Some(b)) => a.layout_changed(b.as_ref()),
            _ => true,
        }
    }
}

#[derive(Debug, PartialEq)]
struct CustomSnapshot { widget: &'static str, fields: Vec<SnapshotField> }
impl SnapshotModel for CustomSnapshot {}
