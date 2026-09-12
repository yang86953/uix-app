//! Extension contracts used by independently packaged widget libraries.
//! Tree ownership and reconciliation remain in the framework.
pub use super::__private::{WidgetNode, WidgetTree};
pub mod adapter { pub use super::super::adapter::{DynamicViewCaptureContext, ViewAdapter}; }
pub mod animation { pub use super::super::animation::*; pub mod traits { pub use crate::ui::Animatable; } }
pub mod event { pub use super::super::event::*; }
pub mod reactive { pub use crate::ui::state; }
pub mod layout { pub use super::super::layout::*; pub mod flex { pub use crate::ui::layout::flex::*; } pub mod engine { pub use crate::ui::layout::engine::{BoxModel, FlexLayout, GridLayout, LayoutChild, normalize_margin, LayoutEngine, LayoutEngineScratch, LayoutOutput, content_size_from_children, finite_non_negative, finite_or_zero, normalize_layout_size}; } }
pub mod theme { pub use super::super::theme::*; pub use crate::ui::{IBoxShadowTokens, ISpacingTokens, ITypographyTokens, ThemeTokens, TokenProvider}; pub mod color_tokens { pub use crate::ui::{IColorTokens, NeutralRole, ShadowToken}; } pub mod traits { pub use crate::ui::{IBoxShadowTokens, ISpacingTokens, ITypographyTokens, ThemeTokens, TokenProvider}; } }
pub mod view { pub use super::super::view::*; }
pub mod widget_snapshot { pub use super::super::widget_snapshot::*; }
pub mod render_handler { pub use super::super::render_handler::RenderHandlerRegistration; }
pub mod children { pub use crate::ui::WidgetChildren; }
pub mod text_decoration { pub use super::super::text_decoration::{DecorationSegment, segments, paint}; }
pub mod text_family { pub use super::super::text_family::resolve; }
pub mod text_weight { pub use super::super::text_weight::paint; }
pub mod text_selection { pub mod per_node { pub use crate::ui::text_selection::per_node::PerNodeTextSelection; } }
pub mod widget_runtime {
    pub use crate::ui::integration::children;
    pub use crate::ui::{clipboard, config, locale};
    pub mod traits { pub use crate::ui::__private::traits::*; }
    pub mod paint_context { pub use crate::ui::PaintContext; }
    pub mod paint_scope { pub use crate::ui::widget_runtime::paint_scope::current_paint_widget; }
    pub mod provider_context { pub use crate::ui::widget_runtime::provider_context::{ProviderContext, with_provider_context}; }
    pub mod measurement { pub use crate::ui::widget_runtime::measurement::{text_metrics, with_measurement_tokens}; }
    pub mod tree_measure { pub use crate::ui::widget_runtime::tree_measure::{child_from_tree_with_constraints, child_from_tree_with_constraints_in, child_from_tree_with_flex_constraints, child_from_tree_with_natural_constraints, container_height_is_flex_cross_content, percent_reference_for_children}; }
    pub mod dynamic_label { pub use crate::ui::widget_runtime::dynamic_label::DynamicLabel; }
    pub mod focus_trap { pub use crate::ui::FocusTrap; }
    pub mod widget { pub use crate::ui::__private::{WidgetNode, WidgetTree}; }
}
