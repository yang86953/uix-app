//! Connect state notifications to the UI tree's renderer queue.
use super::reactive::state::InvalidationTarget;
use crate::core::{Rect, WidgetId};
use crate::draw::renderer::{Invalidation, InvalidationQueue};
impl InvalidationTarget for std::sync::Mutex<InvalidationQueue> {
    fn invalidate_paint(&self, widget: WidgetId, rect: Option<Rect>) {
        if let Ok(mut queue) = self.lock() {
            queue.push(Invalidation::Paint { id: widget, rect });
        }
    }
    fn invalidate_layout(&self, widget: WidgetId) {
        if let Ok(mut queue) = self.lock() {
            queue.push(Invalidation::Layout(widget));
        }
    }
}
