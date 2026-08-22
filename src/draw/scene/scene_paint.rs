//! ScenePaint — 场景绘制抽象，解耦 compositor 与 ui 域。

use crate::core::{Point, Rect};

use crate::core::DirtyRegion;
use crate::draw::Transform;
use crate::draw::painting::PaintContext;
use crate::draw::scene::NodeId;

/// Picture cache eligibility declared by widget metadata and refined by runtime signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PicturePolicy {
    /// 节点及其子树不得进入离屏 Picture 缓存路径。
    Never,
    /// 节点满足元数据层面的 Picture 缓存候选条件。
    Eligible,
}

/// 已由 UI 解析完成、可直接交给场景管线执行的 overlay backdrop 效果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayBackdropEffect {
    /// 以逻辑像素表示的模糊区域。
    region: Rect,
    /// 与 Picture blur 一致的逻辑像素半径。
    radius: f32,
}

impl OverlayBackdropEffect {
    /// 从经过校验的逻辑区域与半径创建效果；无效或 no-op 请求返回 `None`。
    pub fn new(region: Rect, radius: f32) -> Option<Self> {
        // 拒绝不能稳定 lower 到设备像素的区域或小于半像素的 no-op 半径。
        if !region.x.is_finite()
            || !region.y.is_finite()
            || !region.w.is_finite()
            || !region.h.is_finite()
            || region.w <= 0.0
            || region.h <= 0.0
            || !radius.is_finite()
            || radius < 0.5
        {
            // 无效请求不进入 renderer effect 计划。
            return None;
        }
        // 保存唯一的区域与半径事实。
        Some(Self { region, radius })
    }

    /// 返回逻辑模糊区域。
    pub const fn region(self) -> Rect {
        // Rect 是小型 Copy 值，直接交给场景管线。
        self.region
    }

    /// 返回逻辑模糊半径。
    pub const fn radius(self) -> f32 {
        // 半径已经在构造边界验证。
        self.radius
    }

    /// 将多个 overlay 请求收敛为单个效果计划。
    pub fn union(self, other: Self) -> Self {
        // 区域采用保守并集，避免分离 overlay 触发重复快照与模糊事务。
        let region = self.region.union(&other.region);
        // 单次模糊采用请求中的最大半径。
        let radius = self.radius.max(other.radius);
        // 两个输入均已验证，并集仍为有效非空区域。
        Self { region, radius }
    }
}

/// 场景绘制契约：LayerTree 通过此 trait 读取节点元数据并下发绘制。
pub trait ScenePaint {
    /// 返回可供场景管线遍历的根节点；无可用场景时返回空值。
    fn root_id(&self) -> Option<NodeId>;
    /// 返回节点结构发生变化时递增的场景树版本。
    fn tree_version(&self) -> u64;
    /// 返回当前帧需要重绘的全局脏区域。
    fn dirty_region(&self) -> DirtyRegion;
    /// 返回节点当前是否参与布局后的场景绘制。
    fn node_visible(&self, id: NodeId) -> bool;
    /// 返回节点在布局坐标系中的边界矩形。
    fn node_frame(&self, id: NodeId) -> Rect;
    /// 返回节点是否因失效而需要重新绘制。
    fn node_dirty(&self, id: NodeId) -> bool;
    /// 返回节点在兄弟节点之间使用的绘制层级。
    fn node_z_index(&self, id: NodeId) -> i32;
    /// Visual transform for this node and its descendants, in layout coordinates.
    fn node_transform(&self, id: NodeId) -> Transform {
        let _ = id;
        Transform::identity()
    }
    /// Opacity multiplier for this node and its descendants.
    fn node_opacity(&self, id: NodeId) -> f32 {
        let _ = id;
        1.0
    }
    /// 返回节点按场景顺序排列的直接子节点切片。
    fn node_children(&self, id: NodeId) -> &[NodeId];
    /// 返回父布局为节点子树声明的不连续裁剪片段。
    fn node_clip_regions(&self, id: NodeId) -> Option<Vec<Rect>> {
        // 默认场景节点不需要额外的父级片段裁剪。
        let _ = id;
        // 用空能力保持现有场景实现兼容。
        None
    }
    /// 返回节点声明并经运行时信号收紧后的 Picture 缓存资格。
    fn node_picture_policy(&self, id: NodeId) -> PicturePolicy {
        let _ = id;
        PicturePolicy::Never
    }
    /// 返回节点是否登记了需要保持交互语义的事件处理器。
    fn node_has_semantic_handlers(&self, id: NodeId) -> bool {
        let _ = id;
        false
    }
    /// 返回节点内容是否会在树结构不变时动态更新。
    fn node_has_dynamic_content(&self, id: NodeId) -> bool {
        let _ = id;
        false
    }
    /// 返回节点是否持有会改变绘制结果的交互状态。
    fn node_has_interactive_state(&self, id: NodeId) -> bool {
        let _ = id;
        false
    }
    /// 返回节点是否要求连续接收指针移动更新。
    fn node_wants_continuous_pointer_move(&self, id: NodeId) -> bool {
        let _ = id;
        false
    }
    /// 返回节点是否属于需要独立合成顺序的浮层。
    fn node_is_overlay(&self, id: NodeId) -> bool {
        let _ = id;
        false
    }
    /// 返回该浮层是否需要保留一份不含浮层像素的干净背景。
    ///
    /// 未细分浮层能力的场景保持保守语义：所有浮层都需要背景快照。
    fn node_requires_overlay_backdrop(&self, id: NodeId) -> bool {
        self.node_is_overlay(id)
    }
    /// 返回当前帧全部 overlay 合并后的 backdrop 效果计划。
    fn overlay_backdrop_effect(&self) -> Option<OverlayBackdropEffect> {
        // 非 UI 场景默认不请求 backdrop 效果。
        None
    }
    /// 返回节点是否显式请求在子树完成后绘制覆盖视觉。
    fn node_paints_after_children(&self, id: NodeId) -> bool {
        // 非 UI 场景默认没有二阶段覆盖绘制。
        let _ = id;
        // 保持现有 ScenePaint 实现兼容。
        false
    }
    /// 返回节点子树在给定布局 frame 内使用的连续裁剪矩形。
    fn children_clip(&self, id: NodeId, frame: Rect) -> Option<Rect>;
    /// 返回节点绘制可能影响的矩形，可在 frame 外扩展。
    fn dirty_rect(&self, id: NodeId, frame: Rect) -> Rect;
    /// 滚动容器内容偏移（viewport → content），无滚动时返回 `None`。
    fn scroll_offset(&self, id: NodeId) -> Option<(f32, f32)>;
    /// 返回当前持有键盘焦点的节点。
    fn focused_node(&self) -> Option<NodeId>;
    /// 返回节点当前是否可以接收键盘焦点。
    fn node_focusable(&self, id: NodeId) -> bool;
    /// 返回布局坐标命中位置最上层的可交互节点。
    fn hit_test(&self, pos: Point) -> Option<NodeId>;
    /// 返回节点的直接父节点；根节点和未知节点返回空值。
    fn parent(&self, id: NodeId) -> Option<NodeId>;
    /// 在给定 frame 与绘制上下文中下发节点绘制回调。
    fn paint(&self, id: NodeId, frame: Rect, ctx: &mut PaintContext<'_>);
}
