//! The 10 manager systems that compose widget behavior.
//! Managers follow composition over inheritance — each is injected into the widget tree.

use std::collections::HashMap;

mod animation_manager;
mod drag_manager;
mod event_manager;
mod focus_manager;
mod image_manager;
mod interaction_manager;
mod state_manager;
mod style_manager;
mod text_manager;

pub use animation_manager::AnimationManager;
pub use drag_manager::*;
pub use event_manager::*;
pub use focus_manager::*;
pub use image_manager::*;
pub use interaction_manager::*;
pub use state_manager::*;
pub use style_manager::*;
pub use text_manager::*;

/// Aggregated container for all 10 manager systems.
/// Composed into `WidgetTree` to provide cross-cutting services to widgets
/// during layout, rendering, and event processing.
///
/// Supports per-widget overrides: individual widgets can have their own
/// manager instance, falling back to the tree-level default.
#[derive(Default)]
pub struct WidgetManagers {
    pub state: StateManager,
    pub style: StyleManager,
    pub text: TextManager,
    pub image: ImageManager,
    pub interaction: InteractionManager,
    pub animation: AnimationManager,
    pub focus: FocusManager,
    pub drag: DragManager,
    pub event: EventManager,
    /// Per-widget manager overrides keyed by WidgetId.
    overrides: HashMap<usize, Box<WidgetManagersOverrides>>,
}

/// Subset of managers that can be overridden per-widget.
#[derive(Default)]
struct WidgetManagersOverrides {
    state: Option<StateManager>,
    text: Option<TextManager>,
    image: Option<ImageManager>,
}

impl WidgetManagers {
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the state manager for a specific widget.
    pub fn override_state(&mut self, widget_id: usize, mgr: StateManager) {
        self.overrides.entry(widget_id).or_default().state = Some(mgr);
    }

    /// Override the text manager for a specific widget.
    pub fn override_text(&mut self, widget_id: usize, mgr: TextManager) {
        self.overrides.entry(widget_id).or_default().text = Some(mgr);
    }

    /// Override the image manager for a specific widget.
    pub fn override_image(&mut self, widget_id: usize, mgr: ImageManager) {
        self.overrides.entry(widget_id).or_default().image = Some(mgr);
    }

    /// Remove all overrides for a widget.
    pub fn remove_overrides(&mut self, widget_id: usize) {
        self.overrides.remove(&widget_id);
    }

    /// Get the effective state manager for a widget (override or default).
    pub fn state_for(&self, widget_id: usize) -> &StateManager {
        self.overrides
            .get(&widget_id)
            .and_then(|o| o.state.as_ref())
            .unwrap_or(&self.state)
    }

    /// Get the effective text manager for a widget (override or default).
    pub fn text_for(&self, widget_id: usize) -> &TextManager {
        self.overrides
            .get(&widget_id)
            .and_then(|o| o.text.as_ref())
            .unwrap_or(&self.text)
    }

    /// Get the effective image manager for a widget (override or default).
    pub fn image_for(&self, widget_id: usize) -> &ImageManager {
        self.overrides
            .get(&widget_id)
            .and_then(|o| o.image.as_ref())
            .unwrap_or(&self.image)
    }
}
