//! ScenePaint — 场景绘制抽象，解耦 compositor 与 ui 域。

use std::collections::HashSet;

use crate::core::{Point, Rect};

use crate::core::DirtyRegion;
use crate::draw::Transform;
use crate::draw::painting::PaintContext;
use crate::draw::scene::NodeId;

/// 调试检查器读取的单个场景节点快照，不持有 UI 对象借用。
#[derive(Debug, Clone, PartialEq)]
pub struct HoverInspectorNode {
    /// 稳定场景节点身份。
    pub node_id: NodeId,
    /// Rust 组件动态类型名。
    pub type_name: &'static str,
    /// automation id、key 或节点 id 形成的稳定可读身份。
    pub stable_id: String,
    /// 布局完成后的节点矩形。
    pub frame: Rect,
    /// 当前兄弟绘制层级。
    pub z_index: i32,
    /// 当前直接子节点数量。
    pub child_count: usize,
    /// 当前是否可见。
    pub visible: bool,
    /// 当前是否需要重绘。
    pub dirty: bool,
    /// 当前是否为悬停目标。
    pub hovered: bool,
    /// 当前是否为按压目标。
    pub pressed: bool,
    /// 当前是否持有键盘焦点。
    pub focused: bool,
    /// 当前是否禁止交互。
    pub disabled: bool,
    /// 当前是否已挂接到树。
    pub attached: bool,
    /// 当前是否已完成挂载。
    pub mounted: bool,
    /// 当前生命周期是否活动。
    pub active: bool,
    /// 当前是否等待离场移除。
    pub pending_removal: bool,
}

/// 从场景根到悬停叶节点的只读检查器快照。
#[derive(Debug, Clone, PartialEq)]
pub struct HoverInspectorSnapshot {
    /// 严格按根到叶顺序保存的节点路径。
    pub nodes: Vec<HoverInspectorNode>,
}

impl HoverInspectorSnapshot {
    /// 返回最深命中的叶节点。
    pub fn leaf(&self) -> Option<&HoverInspectorNode> {
        self.nodes.last()
    }

    /// 生成包含路径、状态、生命周期和布局事实的紧凑检查器文本。
    pub fn inspector_lines(&self, max_path_nodes: usize) -> Vec<String> {
        let max_path_nodes = max_path_nodes.max(1);
        let total = self.nodes.len();
        let first = total.saturating_sub(max_path_nodes);
        let mut lines = vec![format!("UIX Inspector depth={total} flags=HPFDV*|AMXR")];
        if first > 0 {
            lines.push(format!("... {first} ancestors omitted"));
        }
        for (index, node) in self.nodes.iter().enumerate().skip(first) {
            let type_name = node.type_name.rsplit("::").next().unwrap_or(node.type_name);
            let type_name = compact_inspector_text(type_name, 18);
            let stable_id = compact_inspector_text(&node.stable_id, 22);
            let flags = format!(
                "{}{}{}{}{}{}|{}{}{}{}",
                inspector_flag(node.hovered, 'H'),
                inspector_flag(node.pressed, 'P'),
                inspector_flag(node.focused, 'F'),
                inspector_flag(node.disabled, 'D'),
                inspector_flag(node.visible, 'V'),
                inspector_flag(node.dirty, '*'),
                inspector_flag(node.attached, 'A'),
                inspector_flag(node.mounted, 'M'),
                inspector_flag(node.active, 'X'),
                inspector_flag(node.pending_removal, 'R'),
            );
            lines.push(format!(
                "{index:02} {type_name} {stable_id} [{flags}] ({:.0},{:.0} {:.0}x{:.0}) z{} c{}",
                node.frame.x,
                node.frame.y,
                node.frame.w,
                node.frame.h,
                node.z_index,
                node.child_count,
            ));
        }
        lines
    }
}

// 把布尔状态编码为检查器固定位置字符。
fn inspector_flag(enabled: bool, marker: char) -> char {
    if enabled { marker } else { '-' }
}

// 限制动态类型名和稳定标识长度，保留行后部的状态与几何事实。
fn compact_inspector_text(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut text = value
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    text.push('…');
    text
}

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
    /// 尝试把当前帧的 Paint 失效身份一次写入调用方复用集合。
    ///
    /// `Some(true)` 表示全部节点都脏，`Some(false)` 表示集合包含完整的局部失效身份，
    /// `None` 表示实现不提供批量快照，调用方必须继续逐节点查询 [`Self::node_dirty`]。
    fn paint_invalidation_snapshot_into(&self, ids: &mut HashSet<NodeId>) -> Option<bool> {
        ids.clear();
        None
    }
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
    /// 在节点声明父级不连续裁剪片段时，以只读切片调用一次访问器。
    ///
    /// 返回值区分“没有片段元数据”和 `Some(empty)`：前者为 `false` 且不调用访问器，
    /// 后者为 `true` 且传入空切片。切片只在访问器调用期间有效，消费方不得保留。
    fn visit_node_clip_regions(&self, id: NodeId, visitor: &mut dyn FnMut(&[Rect])) -> bool {
        // 默认场景节点不提供额外的父级片段裁剪能力。
        let _ = (id, visitor);
        false
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
    /// 返回浮层脱离普通父子遍历后应在根画布应用的完整变换。
    ///
    /// 默认只保留节点自身变换；需要跟随原树祖先或滚动锚点的场景应显式覆盖。
    fn node_overlay_transform(&self, id: NodeId) -> Transform {
        self.node_transform(id)
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
    /// 返回从根到命中叶节点的调试检查器快照。
    fn hover_inspector(&self, leaf: NodeId) -> Option<HoverInspectorSnapshot> {
        // 非 UI 场景不必实现组件检查器。
        let _ = leaf;
        None
    }
    /// 在给定 frame 与绘制上下文中下发节点绘制回调。
    fn paint(&self, id: NodeId, frame: Rect, ctx: &mut PaintContext<'_>);
}
