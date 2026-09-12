//! Extension contracts used by independently packaged widget libraries.
//! Tree ownership and reconciliation remain in the framework.
pub use crate::ui::widget_runtime::widget::{WidgetNode, WidgetTree};
pub use crate::ui::{
    ComponentContext, DynamicChildInput, DynamicChildrenCoordinator, DynamicRefresh,
};
pub mod adapter {
    pub use crate::ui::adapter::DynamicViewCaptureContext;
    /// Expand a declaration without access to a live runtime tree.
    pub fn expand(view: crate::ui::ViewNode) -> super::WidgetNode {
        crate::ui::adapter::ViewAdapter::expand(view)
    }
}
pub mod animation {
    pub use crate::ui::animation::{
        Animated, Animation, AnimationConfig, AnimationGroup, AnimationGroupError,
        AnimationGroupItem, Easing, Keyframe, KeyframeAnimation, KeyframeDirection, KeyframeError,
        KeyframeFillMode, KeyframePlayback, Spring, SpringAnimation, Transition, TransitionPlayer,
        presets, reduced_motion, set_reduced_motion,
    };
    pub mod traits {
        pub use crate::ui::Animatable;
    }
}
pub mod event {
    pub use crate::ui::event::{
        ClickEvent, EventResult, HandlerId, HandlerOptions, HandlerRegistration, SemanticEvent,
        SemanticKind, SemanticPayload, SystemEvent, SystemEventKind, WindowAction,
    };
}
pub mod reactive {
    pub use crate::ui::state;
}
pub mod layout {
    pub use super::super::layout::*;
    pub mod flex {
        pub use crate::ui::layout::flex::*;
    }
    pub mod engine {
        pub use crate::ui::layout::engine::{
            BoxModel, FlexLayout, GridLayout, LayoutChild, LayoutEngine, LayoutEngineScratch,
            LayoutOutput, content_size_from_children, finite_non_negative, finite_or_zero,
            normalize_layout_size, normalize_margin,
        };
    }
}
pub mod theme {
    pub use crate::ui::theme::{ModeTokens, Theme, TokenPatch, TokenValue, style};
    pub use crate::ui::{ThemeTokens, TokenProvider};
    pub mod traits {
        pub use crate::ui::{ThemeTokens, TokenProvider};
    }
}
pub mod view {
    pub use crate::ui::{AccessibilityExt, EventExt, StyleExt, TransitionExt, View, ViewNode};
}
pub mod widget_snapshot {
    pub use crate::ui::{
        AccessibilityRole, AccessibilitySnapshot, AccessibilityState, AriaAttribute,
        SelectionSnapshot, SnapshotField, SnapshotModel, SnapshotSource, SnapshotValue,
        WidgetConfigSnapshot, WidgetSnapshotFields,
    };
}
pub mod render_handler {
    pub use super::super::render_handler::RenderHandlerRegistration;
}
pub mod children {
    pub use crate::ui::WidgetChildren;
}
pub mod text_decoration {
    pub use super::super::text_decoration::{DecorationSegment, paint, segments};
}
pub mod text_family {
    pub use super::super::text_family::resolve;
}
pub mod text_weight {
    pub use super::super::text_weight::paint;
}
pub mod text_selection {
    pub mod per_node {
        pub use crate::ui::text_selection::per_node::PerNodeTextSelection;
    }
}
pub mod widget_runtime {
    pub use crate::ui::integration::children;
    pub use crate::ui::{clipboard, config};
    pub mod traits {
        pub use crate::ui::widget_runtime::traits::{
            EventHandler, IntoWidgetNode, TextEditSnapshot, Widget, WidgetAnimation,
            WidgetCapabilities, WidgetLayout, WidgetLifecycle, WidgetRender, WidgetTextInput,
            WidgetTextSelection,
        };
    }
    pub mod paint_context {
        pub use crate::ui::PaintContext;
    }
    pub mod paint_scope {
        pub use crate::ui::widget_runtime::paint_scope::current_paint_widget;
    }
    pub mod provider_context {
        pub use crate::ui::widget_runtime::provider_context::{
            ProviderContext, with_provider_context,
        };
    }
    pub mod measurement {
        pub use crate::ui::widget_runtime::measurement::{text_metrics, with_measurement_tokens};
    }
    pub mod tree_measure {
        pub use crate::ui::widget_runtime::tree_measure::{
            child_from_tree_with_constraints, child_from_tree_with_constraints_in,
            child_from_tree_with_flex_constraints, child_from_tree_with_natural_constraints,
            container_height_is_flex_cross_content, percent_reference_for_children,
        };
    }
    pub mod dynamic_label {
        pub use crate::ui::widget_runtime::dynamic_label::DynamicLabel;
    }
    pub mod focus_trap {
        pub use crate::ui::FocusTrap;
    }
    pub mod widget {
        pub use crate::ui::widget_runtime::widget::{WidgetNode, WidgetTree};
    }
}

pub mod binding {
    pub use crate::ui::primitives::binding::{capture_dependency, write_if_changed};
}
