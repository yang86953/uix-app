//! 图层合成。

#![allow(clippy::too_many_arguments)]

pub mod layer_tree;
pub mod node;
pub(crate) mod picture;
pub(crate) mod render_object;
pub mod scene_paint;
pub(crate) mod viewport_transform;

pub use layer_tree::{LayerNode, LayerTree};
pub use picture::blur_picture_region;
pub use render_object::{RenderObjectEntry, RenderObjectTree};
// 导出 UI 与 renderer 之间的 typed overlay effect 契约。
pub use scene_paint::{
    HoverInspectorNode, HoverInspectorSnapshot, OverlayBackdropEffect, PicturePolicy, ScenePaint,
};
pub use viewport_transform::{
    content_to_viewport, cumulative_scroll, needs_paint, needs_paint_rect, node_viewport_frame,
    node_visual_rect, node_visual_transform, visible_viewport_rect,
};

pub use node::NodeId;
