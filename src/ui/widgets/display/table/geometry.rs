use crate::core::{Point, Rect};

// 复用表模块唯一位宽敏感收敛实现的有限非负纯函数。
use super::types::{Fixed, TableColumn, finite_nonnegative};

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

// 保存列几何的最近输入与可跨帧复用的列数组。
#[derive(Debug, Default)]
pub(crate) struct TableColumnGeometryCache {
    key: Option<TableColumnGeometryKey>,
    geometry: TableColumnGeometry,
}

// 只记录几何标量；列宽与固定区直接和已求解列逐项比较，避免散列碰撞。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TableColumnGeometryKey {
    origin_x_bits: u32,
    viewport_width_bits: u32,
    selection_width_bits: u32,
    scroll_x_bits: u32,
}

impl Default for TableColumnGeometry {
    fn default() -> Self {
        Self {
            columns: Vec::new(),
            middle_clip: Rect::zero(),
            left_width: 0.0,
            right_width: 0.0,
            max_scroll_x: 0.0,
            origin_x: 0.0,
            viewport_width: 0.0,
            selection_width: 0.0,
        }
    }
}

impl TableColumnGeometryCache {
    // 返回与当前列定义和视口标量匹配的共享几何。
    pub(crate) fn resolve(
        &mut self,
        columns: &[TableColumn],
        origin_x: f32,
        viewport_width: f32,
        selection_width: f32,
        scroll_x: f32,
    ) -> &TableColumnGeometry {
        let key = TableColumnGeometryKey {
            origin_x_bits: origin_x.to_bits(),
            viewport_width_bits: viewport_width.to_bits(),
            selection_width_bits: selection_width.to_bits(),
            scroll_x_bits: scroll_x.to_bits(),
        };
        let columns_match = self.geometry.columns.len() == columns.len()
            && self
                .geometry
                .columns
                .iter()
                .zip(columns)
                .all(|(resolved, column)| {
                    resolved.width.to_bits() == finite_nonnegative(column.width).to_bits()
                        && resolved.zone
                            == match column.fixed {
                                Some(Fixed::Left) => ColumnZone::Left,
                                Some(Fixed::Right) => ColumnZone::Right,
                                None => ColumnZone::Middle,
                            }
                });
        if self.key != Some(key) || !columns_match {
            self.geometry
                .resolve(columns, origin_x, viewport_width, selection_width, scroll_x);
            self.key = Some(key);
        }
        &self.geometry
    }

    // 以只读借用暴露已经求解的共享几何。
    pub(crate) fn geometry(&self) -> &TableColumnGeometry {
        &self.geometry
    }
}

impl TableColumnGeometry {
    #[cfg(test)]
    pub(crate) fn new(
        columns: &[TableColumn],
        origin_x: f32,
        viewport_width: f32,
        selection_width: f32,
        scroll_x: f32,
    ) -> Self {
        let mut geometry = Self::default();
        geometry.resolve(columns, origin_x, viewport_width, selection_width, scroll_x);
        geometry
    }

    // 在保留列数组容量的前提下重新求解全部列区几何。
    fn resolve(
        &mut self,
        columns: &[TableColumn],
        origin_x: f32,
        viewport_width: f32,
        selection_width: f32,
        scroll_x: f32,
    ) {
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

        self.columns.clear();
        self.columns.reserve(columns.len());
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
            self.columns.push(LaidOutColumn {
                index,
                x,
                width,
                zone,
            });
        }

        self.middle_clip = Rect::new(middle_x, 0.0, middle_viewport_width, 0.0);
        self.left_width = left_width;
        self.right_width = right_width;
        self.max_scroll_x = max_scroll_x;
        self.origin_x = origin_x;
        self.viewport_width = viewport_width;
        self.selection_width = selection_width;
    }

    pub(crate) fn clip_for(&self, zone: ColumnZone, y: f32, height: f32) -> Option<Rect> {
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
    pub(crate) fn span_bounds(
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
    pub(crate) fn span_clip_for(
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
    pub(crate) fn merged_span_repaint_clip_for(
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
    pub(crate) fn merged_repaint_clip_for(
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

    pub(crate) fn column_at(&self, x: f32) -> Option<usize> {
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
