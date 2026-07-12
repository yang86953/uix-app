//! 图层合成。

#![allow(clippy::too_many_arguments)]

pub mod layer_tree;
pub(crate) mod picture;
pub mod scene_paint;
mod viewport_transform;

pub use crate::draw::render_object::{RenderObjectEntry, RenderObjectTree};
pub use layer_tree::{LayerNode, LayerTree};
pub use scene_paint::{PicturePolicy, ScenePaint};
pub use viewport_transform::{
    content_to_viewport, cumulative_scroll, needs_paint, needs_paint_rect, node_viewport_frame,
    visible_viewport_rect,
};
