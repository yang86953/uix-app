use crate::core::{Point, Rect};

use super::{Fixed, TableColumn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColumnZone {
    Left,
    Middle,
    Right,
}

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
        let viewport_width = viewport_width.max(0.0);
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
            let width = column.width.max(0.0);
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

    pub fn column_at(&self, x: f32) -> Option<usize> {
        for zone in [ColumnZone::Left, ColumnZone::Right, ColumnZone::Middle] {
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
        .map(|column| column.width.max(0.0))
        .sum()
}
