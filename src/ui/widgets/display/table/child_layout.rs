//! Table 自定义单元格子树布局与父级片段裁剪。

// 引入通用矩形几何。
use crate::core::Rect;
// 引入布局子项、组件标识与只读组件树。
use crate::ui::{LayoutChild, WidgetId, WidgetTree};

// 引入固定列共享绘制顺序。
use super::geometry::COLUMN_PAINT_ORDER;
// 引入父模块定义的表格组件。
use super::Table;

// 为表格实现自定义 View 子树布局辅助。
impl Table {
    // 布局自定义单元格或展开行子树。
    pub(super) fn layout_table_children(
        // 接收当前表格组件。
        &self,
        // 接收表格最终布局矩形。
        frame: Rect,
        // 接收本轮已经测量的直接子项。
        children: &[LayoutChild],
        // 接收用于写入父级片段元数据的组件树。
        tree: &WidgetTree,
    ) -> Vec<(WidgetId, Rect)> {
        let mut output = Vec::with_capacity(children.len());
        self.layout_table_children_into(frame, children, tree, &mut output);
        output
    }

    // 把自定义单元格或展开行位置写入布局树提供的跨帧缓冲。
    pub(super) fn layout_table_children_into(
        &self,
        frame: Rect,
        children: &[LayoutChild],
        tree: &WidgetTree,
        output: &mut Vec<(WidgetId, Rect)>,
    ) {
        // 保存最新 frame 供可见行与滚动范围解析复用。
        self.last_frame.set(Some(frame));
        output.clear();
        output.reserve(children.len());
        // 泛型 View 列存在时优先布局物化单元格子树。
        if !self.view_columns.is_empty() {
            // 逐单元格生成完整 frame 与片段元数据。
            self.layout_cell_view_children_into(frame, children, tree, output);
            return;
        }
        // 展开行子树不使用表格单元格片段裁剪。
        for child in children {
            // 清除可能由动态子树协调保留下来的旧片段元数据。
            Self::clear_child_clip_regions(tree, child.id);
        }
        // 没有展开行时不排列任何普通子项。
        let Some(expanded_row) = self.expanded_row.get() else {
            // 保持原有空布局结果。
            return;
        };
        // 展开行只消费首个直接子项。
        let Some(child) = children.first() else {
            // 缺少 renderer 子树时保持空布局。
            return;
        };
        // 展开内容位于目标数据行之后。
        let y = frame.y
            // 跳过完整表头区域。
            + self.total_header_height()
            // 跳过表头与表体分隔线。
            + self.visual.geometry.body_separator
            // 跳过展开行本身及其之前的普通行。
            + (expanded_row + 1) as f32 * self.row_h;
        // 写入占满表格宽度的单一展开内容矩形。
        output.push((
            // 保留展开子树组件标识。
            child.id,
            // 展开子树不参与固定列片段拆分。
            Rect::new(frame.x, y, frame.w, self.expand_height),
        ));
    }

    // 布局当前物化窗口中的自定义单元格子树。
    fn layout_cell_view_children_into(
        // 接收当前表格组件。
        &self,
        // 接收表格最终布局矩形。
        frame: Rect,
        // 接收按行与 View 列排序的物化子项。
        children: &[LayoutChild],
        // 接收用于更新节点片段元数据的组件树。
        tree: &WidgetTree,
        // 接收布局树跨帧复用的位置数组。
        positions: &mut Vec<(WidgetId, Rect)>,
    ) {
        // 解析动态子树当前实际物化的行窗口。
        let (start, end) = self
            // 优先使用刷新阶段已经记录的物化范围。
            .materialized_cell_range
            // 读取无锁单线程状态。
            .get()
            // 初次布局时根据真实表体高度计算范围。
            .unwrap_or_else(|| self.visible_row_range(self.body_viewport_height()));
        // 为当前横向滚动与固定列状态建立共享列几何。
        let column_geometry = self.column_geometry_ref(frame.x, frame.w);
        // 表体内容坐标从完整表头与分隔线之后开始。
        let body_top = frame.y + self.total_header_height() + self.visual.geometry.body_separator;
        // View 列非空分支保证除数至少为一。
        let column_count = self.view_columns.len();
        // 按 renderer 生成时的稳定行主序遍历全部子项。
        for (local_index, child) in children.iter().enumerate() {
            // 将局部子项索引还原为绝对数据行。
            let row = start + local_index / column_count;
            // 多余旧子项不得保留上一次布局的可见片段。
            if row >= end {
                // 用空片段集合明确隐藏过期子树。
                Self::set_child_clip_regions_reusing(tree, child.id, std::iter::empty());
                // 同时把过期子树收敛到表体起点的零尺寸矩形。
                positions.push((child.id, Rect::new(frame.x, body_top, 0.0, 0.0)));
                // 继续清理其余可能存在的过期子项。
                continue;
            }
            // 将局部列槽还原为真实逻辑列索引。
            let column_index = self.view_columns[local_index % column_count];
            // 查找该逻辑列当前真实物理位置与固定区。
            let Some(column) = column_geometry
                // 遍历规范化后的列几何。
                .columns
                // 建立只读迭代器。
                .iter()
                // 按稳定逻辑索引匹配目标列。
                .find(|column| column.index == column_index)
            else {
                // 缺失列几何时隐藏对应动态子树。
                Self::set_child_clip_regions_reusing(tree, child.id, std::iter::empty());
                // 用零尺寸位置保持结果与子项标识对应。
                positions.push((child.id, Rect::new(frame.x, body_top, 0.0, 0.0)));
                // 继续处理下一物化子项。
                continue;
            };
            // 展开行之后的内容坐标需要包含展开区域高度。
            let expanded_offset = if self
                // 读取当前展开行。
                .expanded_row
                // 检查目标行是否位于展开内容之后。
                .get()
                // 只在展开行存在且目标行更靠后时增加偏移。
                .is_some_and(|expanded| row > expanded)
            {
                // 使用规范化后的展开内容高度。
                self.expand_height
            } else {
                // 其余行不增加额外偏移。
                0.0
            };
            // View 子树保持在表体内容坐标，由合成器统一应用纵向滚动。
            let row_y = body_top + row as f32 * self.row_h + expanded_offset;
            // 被其他合并锚点覆盖的 View 列不应重复显示子树。
            if self.cell_anchor(row, column_index) != Some((row, column_index)) {
                // 空片段集合同时阻止绘制与命中。
                Self::set_child_clip_regions_reusing(tree, child.id, std::iter::empty());
                // 保持覆盖列物理起点但把 frame 收敛为零。
                positions.push((child.id, Rect::new(column.x, row_y, 0.0, 0.0)));
                // 继续处理下一单元格子树。
                continue;
            }
            // 读取锚点当前声明的行跨度。
            let row_span = self.row_span(row, column_index);
            // 读取锚点当前声明的逻辑列跨度。
            let col_span = self.col_span(row, column_index);
            // 合并跨行高度并裁剪到现有数据行范围。
            let cell_height = self.span_height(row, row_span);
            // 按覆盖列真实物理位置解析唯一逻辑子树的完整 frame。
            let Some(cell_frame) = column_geometry.span_bounds(
                // 传入当前 View 锚点列。
                column_index,
                // 传入锚点声明的逻辑列跨度。
                col_span,
                // 保留表体内容坐标中的纵坐标。
                row_y,
                // 保留完整跨行高度。
                cell_height,
            ) else {
                // 缺失跨度几何时明确隐藏当前子树。
                Self::set_child_clip_regions_reusing(tree, child.id, std::iter::empty());
                // 回退为锚点处零尺寸位置。
                positions.push((child.id, Rect::new(column.x, row_y, 0.0, 0.0)));
                // 继续处理下一单元格子树。
                continue;
            };
            // 按共享固定区层级收集该逻辑跨度最终可见的不连续片段。
            let clip_regions = COLUMN_PAINT_ORDER
                // 按 Middle、Left、Right 稳定顺序遍历。
                .into_iter()
                // 只保留跨度实际覆盖且未被更高固定区遮挡的片段。
                .filter_map(|zone| {
                    // 复用普通文本合并重绘的最终可见裁剪契约。
                    column_geometry.merged_span_repaint_clip_for(
                        // 传入当前 View 锚点列。
                        column_index,
                        // 传入锚点声明的逻辑列跨度。
                        col_span,
                        // 传入当前固定列区。
                        zone,
                        // 片段与完整子树共享内容纵坐标。
                        cell_frame.y,
                        // 片段与完整子树共享跨行高度。
                        cell_frame.h,
                    )
                });
            // 把片段写入同一个有状态 View 根节点并复用节点自有数组。
            Self::set_child_clip_regions_reusing(tree, child.id, clip_regions);
            // 子树始终按完整逻辑合并矩形布局一次。
            positions.push((child.id, cell_frame));
        }
    }

    // 清除现有子节点的父布局片段约束。
    fn clear_child_clip_regions(tree: &WidgetTree, child_id: WidgetId) {
        if let Some(child) = tree.get(child_id) {
            child.set_parent_clip_regions(None);
        }
    }

    // 把父布局片段写入现有子节点并保留节点自有数组容量。
    fn set_child_clip_regions_reusing(
        // 接收只读树；节点内部用 RefCell 保存布局元数据。
        tree: &WidgetTree,
        // 接收目标子节点标识。
        child_id: WidgetId,
        // 接收空裁剪或多个实际片段。
        regions: impl Iterator<Item = Rect>,
    ) {
        // 动态协调期间节点可能尚未进入树，缺失时安全跳过元数据写入。
        if let Some(child) = tree.get(child_id) {
            // 更新节点供合成、可见性与命中路径共同读取。
            child.set_parent_clip_regions_reusing(regions);
        }
    }
}
