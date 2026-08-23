//! LayerTree — 合成树（Phase 3 迁入 draw）。
//!
//! 通过 ScenePaint trait 读取场景结构并下发绘制，不依赖 ui 域。

use crate::core::Rect;
use crate::draw::geometry::types::{ImageHandle, Transform};
use crate::draw::painting::DisplayList;
use crate::draw::scene::NodeId;

mod layout;
mod render;

/// 场景节点在合成阶段使用的缓存、裁剪或直接绘制表示。
pub enum LayerNode {
    /// 图片图层：缓存被 ScenePaint 边界选中的子树栅格结果。
    /// 包含 children 以支持嵌套 Picture 缓存层（#82、#86、#87）。
    Picture {
        /// 此图层对应的场景节点身份。
        node_id: NodeId,
        /// 离屏缓存覆盖的场景边界。
        bounds: Rect,
        /// 指示缓存内容是否需要重新绘制。
        is_dirty: bool,
        /// 已分配的离屏图片资源句柄。
        offscreen_handle: Option<ImageHandle>,
        /// widget 自身 Content 阶段的 DisplayList（Phase 7 离屏回放缓存）。
        display_list: Option<DisplayList>,
        // 保存父布局限制当前节点子树的可见片段。
        /// 父布局限制此子树可见区域的裁剪片段。
        clip_regions: Option<Vec<Rect>>,
        /// 参与此图片缓存层的嵌套合成子节点。
        children: Vec<LayerNode>,
        /// 离屏创建连续失败计数，rebuild 时重置为 0。
        retry_count: u8,
    },
    /// 裁剪图层：将子图层内容限制在矩形区域内。
    ClipRect {
        /// 此图层对应的场景节点身份。
        node_id: NodeId,
        /// 此图层施加的矩形裁剪区域。
        rect: Rect,
        /// 绘制此节点及其子树时应用的变换。
        transform: Transform,
        /// 绘制此节点及其子树时应用的不透明度。
        opacity: f32,
        // 保存父布局限制当前节点子树的可见片段。
        /// 父布局限制此子树可见区域的裁剪片段。
        clip_regions: Option<Vec<Rect>>,
        /// 受此矩形裁剪约束的合成子节点。
        children: Vec<LayerNode>,
    },
    /// 普通节点：直接渲染 widget 及其子树（无特殊图层语义，无离屏缓存）。
    Direct {
        /// 此图层对应的场景节点身份。
        node_id: NodeId,
        /// 绘制此节点及其子树时应用的变换。
        transform: Transform,
        /// 绘制此节点及其子树时应用的不透明度。
        opacity: f32,
        // 保存父布局限制当前节点子树的可见片段。
        /// 父布局限制此子树可见区域的裁剪片段。
        clip_regions: Option<Vec<Rect>>,
        /// 无需独立缓存或裁剪层的合成子节点。
        children: Vec<LayerNode>,
    },
}

impl std::fmt::Debug for LayerNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayerNode::Picture {
                node_id,
                bounds,
                is_dirty,
                offscreen_handle,
                display_list,
                clip_regions,
                children,
                retry_count,
            } => f
                .debug_struct("PictureLayer")
                .field("node_id", node_id)
                .field("bounds", bounds)
                .field("is_dirty", is_dirty)
                .field("has_offscreen", &offscreen_handle.is_some())
                .field("has_display_list", &display_list.is_some())
                // 记录调试输出中的父级裁剪片段数量。
                .field(
                    // 使用稳定字段名暴露片段规模。
                    "clip_regions_count",
                    // 未声明片段时按零处理。
                    &clip_regions.as_ref().map_or(0, Vec::len),
                )
                .field("children_count", &children.len())
                .field("retry_count", retry_count)
                .finish(),
            LayerNode::ClipRect {
                node_id,
                rect,
                transform,
                opacity,
                clip_regions,
                children,
            } => f
                .debug_struct("ClipRectLayer")
                .field("node_id", node_id)
                .field("rect", rect)
                .field("transform", transform)
                .field("opacity", opacity)
                // 记录调试输出中的父级裁剪片段数量。
                .field(
                    // 使用稳定字段名暴露片段规模。
                    "clip_regions_count",
                    // 未声明片段时按零处理。
                    &clip_regions.as_ref().map_or(0, Vec::len),
                )
                .field("children_count", &children.len())
                .finish(),
            LayerNode::Direct {
                node_id,
                transform,
                opacity,
                clip_regions,
                children,
            } => f
                .debug_struct("DirectLayer")
                .field("node_id", node_id)
                .field("transform", transform)
                .field("opacity", opacity)
                // 记录调试输出中的父级裁剪片段数量。
                .field(
                    // 使用稳定字段名暴露片段规模。
                    "clip_regions_count",
                    // 未声明片段时按零处理。
                    &clip_regions.as_ref().map_or(0, Vec::len),
                )
                .field("children_count", &children.len())
                .finish(),
        }
    }
}

impl LayerNode {
    fn mark_cache_dirty(&mut self) {
        match self {
            LayerNode::Picture {
                is_dirty, children, ..
            } => {
                *is_dirty = true;
                for child in children.iter_mut() {
                    child.mark_cache_dirty();
                }
            }
            LayerNode::ClipRect { children, .. } | LayerNode::Direct { children, .. } => {
                for child in children.iter_mut() {
                    child.mark_cache_dirty();
                }
            }
        }
    }

    fn mark_clean(&mut self) {
        match self {
            LayerNode::Picture {
                is_dirty, children, ..
            } => {
                *is_dirty = false;
                for child in children.iter_mut() {
                    child.mark_clean();
                }
            }
            LayerNode::ClipRect { children, .. } | LayerNode::Direct { children, .. } => {
                for child in children.iter_mut() {
                    child.mark_clean();
                }
            }
        }
    }

    /// 获取该节点对应的 node_id。
    pub(crate) fn node_id(&self) -> NodeId {
        match self {
            LayerNode::Picture { node_id, .. }
            | LayerNode::ClipRect { node_id, .. }
            | LayerNode::Direct { node_id, .. } => *node_id,
        }
    }

    fn transform(&self) -> Transform {
        match self {
            LayerNode::Picture { .. } => Transform::identity(),
            LayerNode::ClipRect { transform, .. } | LayerNode::Direct { transform, .. } => {
                *transform
            }
        }
    }

    fn opacity(&self) -> f32 {
        match self {
            LayerNode::Picture { .. } => 1.0,
            LayerNode::ClipRect { opacity, .. } | LayerNode::Direct { opacity, .. } => *opacity,
        }
    }

    // 读取父布局为当前节点子树声明的裁剪片段。
    fn clip_regions(&self) -> Option<&[Rect]> {
        // 三种节点都携带相同的父级片段元数据。
        match self {
            // 图片节点只会在没有片段时进入缓存，但仍保留统一字段。
            LayerNode::Picture { clip_regions, .. }
            // 普通裁剪节点继承父级片段限制。
            | LayerNode::ClipRect { clip_regions, .. }
            // 直接节点同样在自身变换之前消费片段。
            | LayerNode::Direct { clip_regions, .. } => clip_regions.as_deref(),
        }
    }
}

/// 管理场景根节点、覆盖层与离屏图片资源生命周期的合成树。
pub struct LayerTree {
    root: Option<LayerNode>,
    overlays: Vec<LayerNode>,
    /// build 后未复用的旧离屏句柄（等待 sweep 释放）。
    orphaned_handles: Vec<ImageHandle>,
}

const PICTURE_CACHE_MIN_NODES: usize = 8;
const PICTURE_CACHE_MIN_PIXELS: f32 = 65_536.0;
/// Hard ceiling for Picture render-target residency owned by one LayerTree.
/// 32 MiB retains one 4K BGRA target or four 1080p targets while preventing
/// an unbounded number of otherwise eligible static subtrees from accumulating.
const PICTURE_CACHE_BUDGET_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
struct PictureSubtreeStats {
    node_count: usize,
    estimated_pixels: f32,
    cacheable: bool,
}

#[derive(Debug, Clone, Copy)]
struct PictureCandidate {
    node_id: NodeId,
    retained_bytes: usize,
    estimated_repaint_work: u64,
    reusable: bool,
}

impl PictureSubtreeStats {
    fn eligible(self) -> bool {
        self.cacheable
            && self.node_count >= PICTURE_CACHE_MIN_NODES
            && self.estimated_pixels >= PICTURE_CACHE_MIN_PIXELS
    }
}

impl std::fmt::Debug for LayerTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayerTree")
            .field("has_root", &self.root.is_some())
            .field("overlay_count", &self.overlays.len())
            .finish()
    }
}

impl Default for LayerTree {
    fn default() -> Self {
        Self::new()
    }
}
