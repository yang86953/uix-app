//! State, derived values and subscriptions without UI, graphics or platform code.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use uix_app::core::{Rect, WidgetId};
use uix_app::ui::state::InvalidationTarget;
use uix_app::ui::{Computed, Effect, State};

struct RefreshCount(AtomicUsize);
impl InvalidationTarget for RefreshCount {
    fn invalidate_paint(&self, _: WidgetId, _: Option<Rect>) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
    fn invalidate_layout(&self, _: WidgetId) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

fn main() {
    let source = State::new(2);
    let refresh = Arc::new(RefreshCount(AtomicUsize::new(0)));
    source.bind_paint_invalidation(WidgetId::new(1), refresh.clone(), None);
    let input = source.clone();
    let doubled = Computed::new(move || input.get() * 2);
    let values = Arc::new(Mutex::new(Vec::new()));
    let output = values.clone();
    let effect = Effect::new(move || output.lock().unwrap().push(doubled.get()));
    source.set(7);
    assert!(effect.has_pending());
    assert!(effect.tick());
    assert_eq!(*values.lock().unwrap(), [4, 14]);
    drop(effect);
    source.set(9);
    assert_eq!(*values.lock().unwrap(), [4, 14]);
    assert_eq!(refresh.0.load(Ordering::Relaxed), 2);
    println!("standalone state, computed value and effect passed");
}
