// 引入组件树核心类型。
use super::super::super::*;
// 引入组件树本体。
use super::super::WidgetTree;
// 引入定位测量与几何类型。
use crate::core::{Constraints, Point, Rect, Size};
// 引入定位产生的视觉平移矩阵。
use crate::draw::Transform;
// 引入父组件布局契约和统一子项描述。
use crate::ui::widget_runtime::traits::WidgetLayout;
// 引入完整定位元数据和模式。
use crate::ui::position::{PositionMode, PositionedLayout};
// 引入共享布局子项类型。
use crate::ui::LayoutChild;

impl WidgetTree {
    /// 自动尺寸脱流父级由内容决定大小，不能用上轮自身尺寸反向钳住内容扩展。
    pub(crate) fn out_of_flow_auto_axes(&self, id: WidgetId) -> (bool, bool) {
        let Some(node) = self.get(id) else {
            return (false, false);
        };
        let position = node.position();
        if !position.mode.is_out_of_flow() {
            return (false, false);
        }
        let (width_locked, height_locked) = self.phase2_explicit_size_locks(id);
        (
            !width_locked
                && !(position.insets.left_value().is_some()
                    && position.insets.right_value().is_some()),
            !height_locked
                && !(position.insets.top_value().is_some()
                    && position.insets.bottom_value().is_some()),
        )
    }

    /// 把定位分类和父布局结果写入调用方工作区。
    pub(crate) fn arrange_positioned_children_into(
        &self,
        layout: &dyn WidgetLayout,
        parent_frame: Rect,
        measured: &mut Vec<LayoutChild>,
        in_flow: &mut Vec<LayoutChild>,
        out_of_flow: &mut Vec<WidgetId>,
        engine: &mut crate::ui::LayoutEngineScratch,
        positions: &mut Vec<(WidgetId, Rect)>,
    ) {
        in_flow.clear();
        out_of_flow.clear();
        for child in measured.drain(..) {
            let mode = self
                .get(child.id)
                .map(|node| node.position().mode)
                .unwrap_or(PositionMode::Static);
            if mode.is_out_of_flow() {
                out_of_flow.push(child.id);
            } else {
                in_flow.push(child);
            }
        }
        layout.layout_children_into(parent_frame, in_flow, self, engine, positions);
        for child_id in out_of_flow.iter().copied() {
            if let Some(rect) = self.out_of_flow_rect(child_id) {
                positions.push((child_id, rect));
            }
        }
    }

    // 让父组件只排列正常流子项，再追加 absolute/fixed 子项结果。
    pub(crate) fn arrange_positioned_children(
        // 读取当前树中的节点元数据与包含块。
        &self,
        // 接收父组件自己的布局实现。
        layout: &dyn WidgetLayout,
        // 接收父节点当前 frame。
        parent_frame: Rect,
        // 接收父组件测量完成的全部直接子项。
        measured: Vec<LayoutChild>,
    ) -> Vec<(WidgetId, Rect)> {
        // 保留继续参与 flex/grid 或定制布局的子项。
        let mut in_flow = Vec::with_capacity(measured.len());
        // 保留需要树级求解的 out-of-flow 子项标识。
        let mut out_of_flow = Vec::new();
        // 按声明顺序分类每个测量结果。
        for child in measured {
            // 读取节点定位模式，缺失节点按 static 保守处理。
            let mode = self
                // 查找仍存活的实际子节点。
                .get(child.id)
                // 读取完整定位元数据。
                .map(|node| node.position().mode)
                // 协调中节点消失时不把它错误提升为 out-of-flow。
                .unwrap_or(PositionMode::Static);
            // absolute 与 fixed 不占用父级正常流槽位。
            if mode.is_out_of_flow() {
                // 保存标识供包含块求解器独立测量与排列。
                out_of_flow.push(child.id);
            } else {
                // static、relative 与 sticky 继续参与父布局。
                in_flow.push(child);
            }
        }
        // 让父组件仅依据正常流子项计算槽位。
        let mut positions = layout.layout_children(parent_frame, &in_flow, self);
        // 按原始声明顺序追加每个脱流子项的确定 frame。
        for child_id in out_of_flow {
            // 节点在测量后被移除时跳过失效结果。
            if let Some(rect) = self.out_of_flow_rect(child_id) {
                // 保留节点标识供统一 frame 提交阶段写入。
                positions.push((child_id, rect));
            }
        }
        // 返回同时覆盖正常流和定位子项的完整结果。
        positions
    }

    // 动态替换节点定位元数据并报告是否发生变化。
    pub(crate) fn set_node_position(
        // 可变访问所属组件树。
        &mut self,
        // 接收需要更新的实际节点标识。
        id: WidgetId,
        // 接收新声明的完整定位值。
        position: PositionedLayout,
    ) -> bool {
        // 不存在或值相同时无需重排。
        if self
            // 读取当前节点。
            .get(id)
            // 比较完整模式与四边值。
            .is_none_or(|node| node.position() == position)
        {
            // 返回无变化。
            return false;
        }
        // 在改写前捕获旧视觉子树边界供脏区清理。
        let old_bounds = self.visual_subtree_bounds(id);
        // 节点仍存在时原子替换小型复制值。
        if let Some(node) = self.get_mut(id) {
            // 保存新定位声明。
            node.set_position(position);
        }
        // 定位影响合成树拓扑、裁剪与命中路径。
        self.tree_version = self.tree_version.wrapping_add(1);
        // 使用新声明计算当前可观察边界。
        let new_bounds = self.visual_subtree_bounds(id);
        // 旧位置和新位置都需要重绘以避免残留。
        for rect in [old_bounds, new_bounds].into_iter().flatten() {
            // 空矩形不产生有效脏区。
            if rect.w > 0.0 && rect.h > 0.0 {
                // 将精确视觉范围加入绘制失效队列。
                self.push_paint_invalidation(id, Some(rect));
            }
        }
        // 报告调用方继续触发布局传播。
        true
    }

    // 判断节点是否是相对根视口合成的 fixed 根。
    pub(crate) fn node_is_fixed(&self, id: WidgetId) -> bool {
        // 只有显式 Fixed 模式截断祖先滚动与裁剪路径。
        self.get(id)
            .is_some_and(|node| node.position().mode == PositionMode::Fixed)
    }

    // 返回节点定位偏移与作者视觉变换合成后的最终矩阵。
    pub(crate) fn positioned_visual_transform(&self, id: WidgetId) -> Transform {
        // 读取节点自己的作者变换，缺失时使用单位矩阵。
        let authored = self
            // 查找当前实际节点。
            .get(id)
            // 读取围绕布局 frame 解析后的视觉矩阵。
            .map(|node| node.visual_transform_matrix())
            // 失效节点不引入变换。
            .unwrap_or_else(Transform::identity);
        // 计算 relative 或 sticky 的纯布局视觉偏移。
        let offset = self.position_visual_offset(id);
        // 先移动布局盒，再应用节点作者变换。
        Transform::translate(offset.x, offset.y).concat(authored)
    }

    // 根据包含块和四边值求解脱流节点 frame。
    fn out_of_flow_rect(&self, id: WidgetId) -> Option<Rect> {
        // 读取当前节点定位声明。
        let position = self.get(id)?.position();
        // 仅处理 absolute 与 fixed。
        if !position.mode.is_out_of_flow() {
            // 其他模式由正常流和视觉偏移负责。
            return None;
        }
        // 根据模式选择最近定位祖先或根视口。
        let block = self.position_containing_block(id, position.mode)?;
        // 脱流子项不参与 Flex 空间分配，不能沿用 grow 的零 basis。
        // 统一自然测量入口也保留透明包装节点的子树代理契约。
        let measured =
            crate::ui::widget_runtime::tree_measure::child_from_tree_with_natural_constraints(
                id,
                self,
                Constraints::loose(Size::new(block.w.max(0.0), block.h.max(0.0))),
            )
            .measured_size;
        // 提取四边显式值。
        let top = position.insets.top_value();
        // 提取右边显式值。
        let right = position.insets.right_value();
        // 提取底边显式值。
        let bottom = position.insets.bottom_value();
        // 提取左边显式值。
        let left = position.insets.left_value();
        // 双侧都声明时由包含块剩余宽度决定 border-box 宽度。
        let width = match (left, right) {
            // 左右同时锚定时拉伸，并把负结果收敛为空宽度。
            (Some(left), Some(right)) => (block.w - left - right).max(0.0),
            // 只有单侧或均为 auto 时保留自然宽度。
            _ => measured.w.max(0.0),
        };
        // 双侧都声明时由包含块剩余高度决定 border-box 高度。
        let height = match (top, bottom) {
            // 上下同时锚定时拉伸，并把负结果收敛为空高度。
            (Some(top), Some(bottom)) => (block.h - top - bottom).max(0.0),
            // 只有单侧或均为 auto 时保留自然高度。
            _ => measured.h.max(0.0),
        };
        // 水平优先使用 left，其次使用 right，均 auto 时贴包含块起点。
        let x = if let Some(left) = left {
            // 从包含块左边向内偏移。
            block.x + left
        } else if let Some(right) = right {
            // 从包含块右边反向放置完整宽度。
            block.x + block.w - right - width
        } else {
            // UIX auto 轴采用包含块起点，保持确定行为。
            block.x
        };
        // 垂直优先使用 top，其次使用 bottom，均 auto 时贴包含块起点。
        let y = if let Some(top) = top {
            // 从包含块顶部向内偏移。
            block.y + top
        } else if let Some(bottom) = bottom {
            // 从包含块底部反向放置完整高度。
            block.y + block.h - bottom - height
        } else {
            // UIX auto 轴采用包含块起点，保持确定行为。
            block.y
        };
        // 返回有限、非负尺寸的最终 border-box。
        Some(crate::ui::layout::engine::normalize_layout_rect(Rect::new(
            // 保存水平坐标。
            x,     // 保存垂直坐标。
            y,     // 保存求解宽度。
            width, // 保存求解高度。
            height,
        )))
    }

    // 选择 absolute 最近定位祖先或 fixed 根视口 frame。
    fn position_containing_block(&self, id: WidgetId, mode: PositionMode) -> Option<Rect> {
        // fixed 始终使用当前根客户区。
        if mode == PositionMode::Fixed {
            // 返回根节点实际 frame。
            return self.root().map(|root| root.frame());
        }
        // 从直接父节点开始寻找最近非 static 祖先。
        let mut current = self.get(id)?.parent();
        // 祖先链有限，直到根节点结束。
        while let Some(parent_id) = current {
            // 读取当前祖先。
            let parent = self.get(parent_id)?;
            // 任意非 static 祖先建立 absolute 包含块。
            if parent.position().mode != PositionMode::Static {
                // 使用该祖先 border-box 作为 UIX 定位坐标系。
                return Some(parent.frame());
            }
            // 继续向根方向查找。
            current = parent.parent();
        }
        // 没有定位祖先时回退根视口。
        self.root().map(|root| root.frame())
    }

    // 计算 relative/sticky 不改变正常流 frame 的视觉偏移。
    fn position_visual_offset(&self, id: WidgetId) -> Point {
        // 读取节点定位声明。
        let Some(position) = self.get(id).map(|node| node.position()) else {
            // 失效节点没有偏移。
            return Point::new(0.0, 0.0);
        };
        // static、absolute 与 fixed 的 frame 已经包含全部布局位置。
        match position.mode {
            // relative 直接使用四边优先级计算视觉平移。
            PositionMode::Relative => Self::relative_offset(position),
            // sticky 结合最近 viewport 和父级边界计算平移。
            PositionMode::Sticky => self.sticky_offset(id, position),
            // 其余模式没有额外视觉平移。
            _ => Point::new(0.0, 0.0),
        }
    }

    // 计算 relative 的左右、上下互斥优先级。
    fn relative_offset(position: PositionedLayout) -> Point {
        // left 优先；未声明 left 时 right 产生反向平移。
        let x = position
            // 读取左边值。
            .insets
            // 复制显式左边。
            .left_value()
            // 没有 left 时把 right 转为负方向。
            .or_else(|| position.insets.right_value().map(|right| -right))
            // 两边均 auto 时不平移。
            .unwrap_or(0.0);
        // top 优先；未声明 top 时 bottom 产生反向平移。
        let y = position
            // 读取顶边值。
            .insets
            // 复制显式顶边。
            .top_value()
            // 没有 top 时把 bottom 转为负方向。
            .or_else(|| position.insets.bottom_value().map(|bottom| -bottom))
            // 两边均 auto 时不平移。
            .unwrap_or(0.0);
        // 返回不改变正常流占位的视觉偏移。
        Point::new(x, y)
    }

    // 计算 sticky 相对最近 viewport 的滚动补偿并限制在父边界内。
    fn sticky_offset(&self, id: WidgetId, position: PositionedLayout) -> Point {
        // 读取 sticky 节点正常流 frame。
        let Some(frame) = self.get(id).map(|node| node.frame()) else {
            // 失效节点没有补偿。
            return Point::new(0.0, 0.0);
        };
        // 读取直接父边界，根节点回退自身 frame。
        let parent_frame = self
            // 读取父节点标识。
            .get(id)
            // 取得父链。
            .and_then(|node| node.parent())
            // 读取父节点实际 frame。
            .and_then(|parent| self.get(parent).map(|node| node.frame()))
            // 根 sticky 使用自身边界。
            .unwrap_or(frame);
        // 查找最近 viewport，同时累计全部祖先滚动偏移。
        let (viewport, scroll) = self.nearest_position_viewport(id);
        // 当前正常流 frame 投影到屏幕后的起点。
        let visual_x = frame.x - scroll.x;
        // 当前正常流 frame 投影到屏幕后的起点。
        let visual_y = frame.y - scroll.y;
        // 没有左右 inset 时保持水平正常流位置。
        let mut dx: f32 = 0.0;
        // left 建立 viewport 左侧下限。
        if let Some(left) = position.insets.left_value() {
            // 只在内容滚过下限时向右补偿。
            dx = dx.max(viewport.x + left - visual_x);
        }
        // right 建立 viewport 右侧上限。
        if let Some(right) = position.insets.right_value() {
            // 把当前补偿限制到右侧可见边界。
            dx = dx.min(viewport.x + viewport.w - right - frame.w - visual_x);
        }
        // sticky 不能越过直接父节点右边界。
        dx = dx.min(parent_frame.x + parent_frame.w - frame.x - frame.w);
        // sticky 不能越过直接父节点左边界。
        dx = dx.max(parent_frame.x - frame.x);
        // 没有上下 inset 时保持垂直正常流位置。
        let mut dy: f32 = 0.0;
        // top 建立 viewport 顶部下限。
        if let Some(top) = position.insets.top_value() {
            // 只在内容滚过下限时向下补偿。
            dy = dy.max(viewport.y + top - visual_y);
        }
        // bottom 建立 viewport 底部上限。
        if let Some(bottom) = position.insets.bottom_value() {
            // 把当前补偿限制到底部可见边界。
            dy = dy.min(viewport.y + viewport.h - bottom - frame.h - visual_y);
        }
        // sticky 不能越过直接父节点底边界。
        dy = dy.min(parent_frame.y + parent_frame.h - frame.y - frame.h);
        // sticky 不能越过直接父节点顶边界。
        dy = dy.max(parent_frame.y - frame.y);
        // 返回只影响视觉、命中和子树的最终补偿。
        Point::new(dx, dy)
    }

    // 查找最近 viewport frame 并累计祖先滚动偏移。
    fn nearest_position_viewport(&self, id: WidgetId) -> (Rect, Point) {
        // 根视口是没有滚动容器时的确定回退。
        let root_frame = self
            // 读取当前根。
            .root()
            // 复制根客户区。
            .map(|root| root.frame())
            // 空树回退空矩形。
            .unwrap_or_default();
        // 保存最近 viewport，首次命中后不再覆盖。
        let mut nearest = None;
        // 累计路径上所有 viewport 滚动量以还原屏幕位置。
        let mut scroll = Point::new(0.0, 0.0);
        // 从直接父节点开始向根遍历。
        let mut current = self.get(id).and_then(|node| node.parent());
        // 祖先链有限。
        while let Some(parent_id) = current {
            // 节点失效时停止并使用已取得信息。
            let Some(parent) = self.get(parent_id) else {
                // 中断不完整父链。
                break;
            };
            // viewport 通过滚动偏移能力明确自己的内容坐标系。
            if let Some((scroll_x, scroll_y)) = parent.viewport_scroll_offset() {
                // 首个命中的 viewport 决定 sticky 可见边界。
                nearest.get_or_insert_with(|| {
                    // 优先使用组件显式子裁剪，否则使用自身 frame。
                    parent
                        .children_clip(parent.frame())
                        .unwrap_or(parent.frame())
                });
                // 累加水平滚动量。
                scroll.x += scroll_x;
                // 累加垂直滚动量。
                scroll.y += scroll_y;
            }
            // 继续向根遍历以累计外层滚动。
            current = parent.parent();
        }
        // 返回最近 viewport 或根客户区，以及完整滚动补偿。
        (nearest.unwrap_or(root_frame), scroll)
    }
}
