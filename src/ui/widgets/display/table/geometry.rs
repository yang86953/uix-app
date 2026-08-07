use crate::core::{Point, Rect};

use super::types::{Fixed, TableColumn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColumnZone {
    Left,
    Middle,
    Right,
}

// 统一表头、表体与命中测试使用的列区绘制层级。
pub(crate) const COLUMN_PAINT_ORDER: [ColumnZone; 3] = [
    // 中间滚动区最先绘制。
    ColumnZone::Middle,
    // 左固定区覆盖中间滚动区。
    ColumnZone::Left,
    // 右固定区最后绘制并位于最上层。
    ColumnZone::Right,
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LaidOutColumn {
    pub index: usize,
    pub x: f32,
    pub width: f32,
    pub zone: ColumnZone,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TableColumnGeometry {
    pub columns: Vec<LaidOutColumn>,
    pub middle_clip: Rect,
    pub left_width: f32,
    pub right_width: f32,
    pub max_scroll_x: f32,
    origin_x: f32,
    viewport_width: f32,
    selection_width: f32,
}

impl TableColumnGeometry {
    pub fn new(
        columns: &[TableColumn],
        origin_x: f32,
        viewport_width: f32,
        selection_width: f32,
        scroll_x: f32,
    ) -> Self {
        let viewport_width = finite_nonnegative(viewport_width);
        let selection_width = finite_nonnegative(selection_width).min(viewport_width);
        let left_width = zone_width(columns, Some(Fixed::Left));
        let right_width = zone_width(columns, Some(Fixed::Right));
        let middle_width = zone_width(columns, None);
        let middle_x = origin_x + selection_width + left_width;
        let middle_viewport_width =
            (viewport_width - selection_width - left_width - right_width).max(0.0);
        let max_scroll_x = (middle_width - middle_viewport_width).max(0.0);
        let scroll_x = scroll_x.clamp(0.0, max_scroll_x);

        let mut columns_out = Vec::with_capacity(columns.len());
        let mut left_x = origin_x + selection_width;
        let mut center_x = middle_x - scroll_x;
        let mut right_x = origin_x + viewport_width - right_width;

        for (index, column) in columns.iter().enumerate() {
            let width = finite_nonnegative(column.width);
            let (x, zone) = match column.fixed {
                Some(Fixed::Left) => {
                    let x = left_x;
                    left_x += width;
                    (x, ColumnZone::Left)
                }
                Some(Fixed::Right) => {
                    let x = right_x;
                    right_x += width;
                    (x, ColumnZone::Right)
                }
                None => {
                    let x = center_x;
                    center_x += width;
                    (x, ColumnZone::Middle)
                }
            };
            columns_out.push(LaidOutColumn {
                index,
                x,
                width,
                zone,
            });
        }

        Self {
            columns: columns_out,
            middle_clip: Rect::new(middle_x, 0.0, middle_viewport_width, 0.0),
            left_width,
            right_width,
            max_scroll_x,
            origin_x,
            viewport_width,
            selection_width,
        }
    }

    pub fn clip_for(&self, zone: ColumnZone, y: f32, height: f32) -> Option<Rect> {
        let viewport = Rect::new(self.origin_x, y, self.viewport_width, height);
        let zone_rect = match zone {
            ColumnZone::Left => Rect::new(
                self.origin_x + self.selection_width,
                y,
                self.left_width,
                height,
            ),
            ColumnZone::Middle => Rect::new(self.middle_clip.x, y, self.middle_clip.w, height),
            ColumnZone::Right => Rect::new(
                self.origin_x + self.viewport_width - self.right_width,
                y,
                self.right_width,
                height,
            ),
        };
        viewport.intersect(&zone_rect)
    }

    // 解析逻辑连续列合并锚点当前使用的完整绘制矩形。
    pub fn span_bounds(
        // 接收列几何快照。
        &self,
        // 接收合并锚点的逻辑列索引。
        column: usize,
        // 接收合并占用的逻辑列数。
        span: usize,
        // 接收单元格纵坐标。
        y: f32,
        // 接收单元格高度。
        height: f32,
    ) -> Option<Rect> {
        // 按覆盖列的真实物理位置解析完整水平联合范围。
        let (left, right) = self.span_horizontal_bounds(column, span, None)?;
        // 返回能够包含全部覆盖列的规范化矩形。
        Some(Rect::new(left, y, (right - left).max(0.0), height))
    }

    // 解析列合并锚点在指定列区中的普通可见片段。
    pub fn span_clip_for(
        // 接收列几何快照。
        &self,
        // 接收合并锚点的逻辑列索引。
        column: usize,
        // 接收合并占用的逻辑列数。
        span: usize,
        // 接收当前绘制列区。
        zone: ColumnZone,
        // 接收单元格纵坐标。
        y: f32,
        // 接收单元格高度。
        height: f32,
    ) -> Option<Rect> {
        // 解析跨度在当前列区实际覆盖的水平范围。
        let (left, right) = self.span_horizontal_bounds(column, span, Some(zone))?;
        // 构造当前列区中的跨度矩形。
        let span_rect = Rect::new(left, y, (right - left).max(0.0), height);
        // 同普通列区可见裁剪求交并排除视口外部分。
        self.clip_for(zone, y, height)?.intersect(&span_rect)
    }

    // 解析列合并锚点在末尾重绘时指定列区的最终可见片段。
    pub fn merged_span_repaint_clip_for(
        // 接收列几何快照。
        &self,
        // 接收合并锚点的逻辑列索引。
        column: usize,
        // 接收合并占用的逻辑列数。
        span: usize,
        // 接收当前重绘列区。
        zone: ColumnZone,
        // 接收单元格纵坐标。
        y: f32,
        // 接收单元格高度。
        height: f32,
    ) -> Option<Rect> {
        // 先取得跨度在当前列区的普通可见片段。
        let span_clip = self.span_clip_for(column, span, zone, y, height)?;
        // 再扣除当前列区之上已经绘制的固定区覆盖。
        self.merged_repaint_clip_for(zone, y, height)?
            // 只保留两种裁剪共同可见的部分。
            .intersect(&span_clip)
    }

    // 按逻辑跨度与可选列区解析覆盖列的物理水平联合范围。
    fn span_horizontal_bounds(
        // 接收列几何快照。
        &self,
        // 接收合并锚点的逻辑列索引。
        column: usize,
        // 接收合并占用的逻辑列数。
        span: usize,
        // 可选地把范围限制在单一列区。
        zone: Option<ColumnZone>,
    ) -> Option<(f32, f32)> {
        // 计算逻辑跨度的排他末端并避免索引加法溢出。
        let end = column.saturating_add(span.max(1));
        // 建立仅包含逻辑跨度与目标列区的稳定迭代器。
        let mut covered = self.columns.iter().filter(|laid_out| {
            // 同时约束逻辑索引范围与可选列区。
            laid_out.index >= column
                // 排除逻辑跨度末端之后的列。
                && laid_out.index < end
                // 未指定列区时接受全部覆盖列，否则只接受匹配列区。
                && zone.is_none_or(|zone| laid_out.zone == zone)
        });
        // 读取首个覆盖列作为联合范围初值。
        let first = covered.next()?;
        // 用首列左边界初始化联合范围起点。
        let mut left = first.x;
        // 用首列右边界初始化联合范围终点。
        let mut right = first.x + first.width;
        // 合并跨度内其余覆盖列的实际物理范围。
        for laid_out in covered {
            // 向左扩展到当前覆盖列起点。
            left = left.min(laid_out.x);
            // 向右扩展到当前覆盖列终点。
            right = right.max(laid_out.x + laid_out.width);
        }
        // 返回规范化后的水平联合范围。
        Some((left, right))
    }

    // 解析行合并锚点在末尾重绘时可使用的最终可见裁剪。
    pub fn merged_repaint_clip_for(
        // 接收需要重绘的锚点所属列区。
        &self,
        // 接收重绘列区。
        zone: ColumnZone,
        // 接收重绘矩形纵坐标。
        y: f32,
        // 接收重绘矩形高度。
        height: f32,
    ) -> Option<Rect> {
        // 先解析普通按层绘制使用的完整列区裁剪。
        let clip = self.clip_for(zone, y, height)?;
        // 中间区不会进入固定区重叠，最高层右区也无需再扣除覆盖。
        if zone != ColumnZone::Left {
            // 保留当前列区的完整可见裁剪。
            return Some(clip);
        }
        // 没有可见右固定区时，左区末尾重绘不会越过其他层级。
        let Some(right_clip) = self.clip_for(ColumnZone::Right, y, height) else {
            // 返回未收缩的左区裁剪。
            return Some(clip);
        };
        // 左区最终可见右边界不能越过更高层右固定区起点。
        let visible_right = (clip.x + clip.w).min(right_clip.x);
        // 右固定区覆盖整个左区时不再重绘左合并锚点。
        if visible_right <= clip.x {
            // 用空结果表示左区没有最终可见片段。
            return None;
        }
        // 返回扣除右固定覆盖后的单一左侧可见片段。
        Some(Rect::new(
            // 保留左区可见起点。
            clip.x,
            // 保留规范化后的纵坐标。
            clip.y,
            // 使用收缩后的最终可见宽度。
            visible_right - clip.x,
            // 保留规范化后的可见高度。
            clip.h,
        ))
    }

    pub fn column_at(&self, x: f32) -> Option<usize> {
        // 按绘制顺序逆序命中，使重叠区选择视觉上最上层的列。
        for zone in COLUMN_PAINT_ORDER.into_iter().rev() {
            if zone == ColumnZone::Middle
                && !self.middle_clip.contains(Point::new(x, self.middle_clip.y))
            {
                continue;
            }
            if let Some(column) = self
                .columns
                .iter()
                .find(|column| column.zone == zone && x >= column.x && x < column.x + column.width)
            {
                return Some(column.index);
            }
        }
        None
    }
}

fn zone_width(columns: &[TableColumn], fixed: Option<Fixed>) -> f32 {
    columns
        .iter()
        .filter(|column| column.fixed == fixed)
        .map(|column| finite_nonnegative(column.width))
        .sum()
}

fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

// 仅在单元测试中编译表格列几何契约。
#[cfg(test)]
// 将固定列重叠命中回归收拢在纯几何模块。
mod tests {
    // 复用列布局类型与父模块导入的列声明。
    use super::*;

    // 标记窄视口固定列绘制层级契约。
    #[test]
    // 验证重叠区命中最后绘制的右固定列。
    fn overlapping_fixed_columns_hit_topmost_painted_zone() {
        // 构造宽于视口的左固定列。
        let left = TableColumn::new("左列", 80.0).fixed(Fixed::Left);
        // 构造同样宽于剩余区域的右固定列。
        let right = TableColumn::new("右列", 80.0).fixed(Fixed::Right);
        // 在一百像素视口中形成二十到八十像素的重叠区。
        let geometry = TableColumnGeometry::new(&[left, right], 0.0, 100.0, 0.0, 0.0);
        // 左侧非重叠区仍由左固定列命中。
        assert_eq!(geometry.column_at(10.0), Some(0));
        // 重叠区必须命中绘制顺序中位于最上层的右固定列。
        assert_eq!(geometry.column_at(50.0), Some(1));
        // 右侧非重叠区继续由右固定列命中。
        assert_eq!(geometry.column_at(90.0), Some(1));
        // 结束固定列重叠命中契约。
    }

    // 标记行合并锚点末尾重绘不得覆盖更高固定区的契约。
    #[test]
    // 验证左固定合并重绘只保留未被右固定区覆盖的可见片段。
    fn merged_repaint_clip_preserves_topmost_fixed_zone() {
        // 构造宽于视口剩余区域的左固定列。
        let left = TableColumn::new("左合并列", 80.0).fixed(Fixed::Left);
        // 构造同样宽且视觉层级更高的右固定列。
        let right = TableColumn::new("右普通列", 80.0).fixed(Fixed::Right);
        // 在一百像素视口中形成二十到八十像素的固定区重叠。
        let geometry = TableColumnGeometry::new(&[left, right], 0.0, 100.0, 0.0, 0.0);
        // 初始按层绘制时左区仍可使用完整八十像素裁剪并等待右区覆盖。
        assert_eq!(
            // 查询普通绘制使用的左区裁剪。
            geometry.clip_for(ColumnZone::Left, 32.0, 64.0),
            // 左区从零到八十像素完整参与底层绘制。
            Some(Rect::new(0.0, 32.0, 80.0, 64.0))
        );
        // 末尾重绘发生在右区之后，只能保留右区起点之前的二十像素。
        assert_eq!(
            // 查询行合并锚点使用的最终可见裁剪。
            geometry.merged_repaint_clip_for(ColumnZone::Left, 32.0, 64.0),
            // 左区最终仅有零到二十像素未被右固定区覆盖。
            Some(Rect::new(0.0, 32.0, 20.0, 64.0))
        );
        // 右固定合并锚点仍可使用其完整八十像素顶层裁剪。
        assert_eq!(
            // 查询最高层右区的合并重绘裁剪。
            geometry.merged_repaint_clip_for(ColumnZone::Right, 32.0, 64.0),
            // 右区从二十到一百像素全部可见。
            Some(Rect::new(20.0, 32.0, 80.0, 64.0))
        );
        // 重叠点命中继续返回最终可见的右固定列。
        assert_eq!(geometry.column_at(50.0), Some(1));
        // 结束行合并锚点重绘层级契约。
    }

    // 标记跨固定区列合并必须覆盖全部逻辑列的契约。
    #[test]
    // 验证右固定锚点能够向左覆盖后声明的左固定列。
    fn cross_zone_span_bounds_cover_reordered_fixed_columns() {
        // 先声明位于视觉右侧的合并锚点列。
        let right_anchor = TableColumn::new("右侧锚点", 60.0).fixed(Fixed::Right);
        // 后声明位于视觉左侧的被覆盖列。
        let left_covered = TableColumn::new("左侧覆盖列", 40.0).fixed(Fixed::Left);
        // 在一百像素视口中让两列分别占据左右连续区域。
        let geometry =
            // 保留与公开列声明相同的右前左后逻辑顺序。
            TableColumnGeometry::new(&[right_anchor, left_covered], 0.0, 100.0, 0.0, 0.0);
        // 跨两列的单一合并单元格必须覆盖零到一百像素的视觉联合范围。
        assert_eq!(
            // 查询从右固定锚点开始的两列合并矩形。
            geometry.span_bounds(0, 2, 32.0, 28.0),
            // 期望矩形同时包含视觉左侧与右侧两列。
            Some(Rect::new(0.0, 32.0, 100.0, 28.0))
        );
        // 末尾重绘必须为跨度中的左固定片段保留零到四十像素。
        assert_eq!(
            // 查询逻辑跨度在左固定区中的最终可见片段。
            geometry.merged_span_repaint_clip_for(
                // 传入右固定锚点索引。
                0,
                // 传入覆盖左右两列的跨度。
                2,
                // 查询视觉左侧固定区。
                ColumnZone::Left,
                // 保留测试纵坐标。
                32.0,
                // 保留测试高度。
                28.0,
            ),
            // 左固定覆盖列完整占据零到四十像素。
            Some(Rect::new(0.0, 32.0, 40.0, 28.0))
        );
        // 末尾重绘必须为同一跨度保留四十到一百像素的右固定片段。
        assert_eq!(
            // 查询逻辑跨度在右固定区中的最终可见片段。
            geometry.merged_span_repaint_clip_for(
                // 传入右固定锚点索引。
                0,
                // 传入覆盖左右两列的跨度。
                2,
                // 查询视觉右侧固定区。
                ColumnZone::Right,
                // 保留测试纵坐标。
                32.0,
                // 保留测试高度。
                28.0,
            ),
            // 右固定锚点完整占据四十到一百像素。
            Some(Rect::new(40.0, 32.0, 60.0, 28.0))
        );
        // 结束跨固定区列合并矩形契约。
    }
    // 结束表格列几何测试模块。
}
