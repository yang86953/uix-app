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
    // 结束表格列几何测试模块。
}
