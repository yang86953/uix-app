use crate::core::{Point, Rect};
use crate::draw::painting::PaintContext;
use crate::draw::Radius;

use super::geometry::{ColumnZone, TableColumnGeometry};
use super::{SortDirection, Table};

pub(super) fn paint(
    table: &Table,
    frame: Rect,
    ctx: &mut PaintContext,
    geometry: &TableColumnGeometry,
) {
    let header_height = table.total_header_height();
    let header_rect = Rect::new(frame.x, frame.y, frame.w, header_height);
    let header_bg = ctx.tokens().color_fill_tertiary();
    let border = ctx.tokens().color_border();
    let text_color = ctx.tokens().color_text();
    let text_secondary = ctx.tokens().color_text_secondary();
    let primary = ctx.tokens().color_primary();
    let radius = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

    ctx.fill_rect(header_rect, header_bg, radius);
    if table.selection {
        let all_checked = table.checked_rows.len() == table.rows.len();
        crate::ui::widgets::icon::paint_icon_in_frame(
            ctx,
            if all_checked {
                "check-square"
            } else {
                "square"
            },
            Rect::new(frame.x + 6.0, header_rect.y, 18.0, header_rect.h),
            text_secondary,
            14.0,
        );
        if table.bordered {
            ctx.fill_rect(
                Rect::new(
                    frame.x + table.selection_width() - 1.0,
                    header_rect.y,
                    1.0,
                    header_rect.h,
                ),
                border,
                None,
            );
        }
    }

    if !table.column_groups.is_empty() {
        paint_group_titles(table, header_rect, ctx, geometry);
    }

    for zone in [ColumnZone::Middle, ColumnZone::Left, ColumnZone::Right] {
        let Some(zone_clip) = geometry.clip_for(zone, header_rect.y, header_rect.h) else {
            continue;
        };
        ctx.canvas_2d().push_clip(zone_clip);
        for laid_out in geometry.columns.iter().filter(|column| column.zone == zone) {
            let grouped = table
                .column_groups
                .iter()
                .find(|group| {
                    laid_out.index >= group.start && laid_out.index < group.start + group.len
                })
                .is_some_and(|group| group.title.is_some());
            let leaf_rect = if grouped {
                Rect::new(
                    header_rect.x,
                    header_rect.y + table.header_h,
                    header_rect.w,
                    table.header_h,
                )
            } else {
                header_rect
            };
            let column = &table.columns[laid_out.index];
            let text_y = ctx.visual_center_y(leaf_rect, 13.0);
            ctx.draw_text(
                &column.title,
                Point::new(laid_out.x + 8.0, text_y),
                text_color,
                13.0,
            );
            if table.sortable || column.sortable {
                let indicator = match column.sort_direction {
                    SortDirection::Asc => "chevron-up",
                    SortDirection::Desc | SortDirection::None => "chevron-down",
                };
                let (color, font_size) = if column.sort_direction == SortDirection::None {
                    (text_secondary, 10.0)
                } else {
                    (primary, 11.0)
                };
                crate::ui::widgets::icon::paint_icon_in_frame(
                    ctx,
                    indicator,
                    Rect::new(
                        laid_out.x + laid_out.width - 24.0,
                        leaf_rect.y,
                        18.0,
                        leaf_rect.h,
                    ),
                    color,
                    font_size,
                );
            }
            if table.bordered {
                ctx.fill_rect(
                    Rect::new(
                        laid_out.x + laid_out.width - 1.0,
                        leaf_rect.y,
                        1.0,
                        leaf_rect.h,
                    ),
                    border,
                    None,
                );
            }
        }
        ctx.canvas_2d().pop_clip();
    }
}

fn paint_group_titles(
    table: &Table,
    header_rect: Rect,
    ctx: &mut PaintContext,
    geometry: &TableColumnGeometry,
) {
    let border = ctx.tokens().color_border();
    let text_color = ctx.tokens().color_text();
    let group_rect = Rect::new(header_rect.x, header_rect.y, header_rect.w, table.header_h);
    let text_y = ctx.visual_center_y(group_rect, 13.0);

    for group in &table.column_groups {
        let Some(title) = group.title.as_deref() else {
            continue;
        };
        for zone in [ColumnZone::Middle, ColumnZone::Left, ColumnZone::Right] {
            let mut members = geometry.columns.iter().filter(|column| {
                column.zone == zone
                    && column.index >= group.start
                    && column.index < group.start + group.len
            });
            let Some(first) = members.next() else {
                continue;
            };
            let mut left = first.x;
            let mut right = first.x + first.width;
            for member in members {
                left = left.min(member.x);
                right = right.max(member.x + member.width);
            }
            let Some(zone_clip) = geometry.clip_for(zone, group_rect.y, group_rect.h) else {
                continue;
            };
            let Some(segment) =
                Rect::new(left, group_rect.y, right - left, group_rect.h).intersect(&zone_clip)
            else {
                continue;
            };
            ctx.canvas_2d().push_clip(segment);
            ctx.draw_text(title, Point::new(segment.x + 8.0, text_y), text_color, 13.0);
            ctx.fill_rect(
                Rect::new(segment.x, segment.y + segment.h - 1.0, segment.w, 1.0),
                border,
                None,
            );
            if table.bordered {
                ctx.fill_rect(
                    Rect::new(segment.x + segment.w - 1.0, segment.y, 1.0, segment.h),
                    border,
                    None,
                );
            }
            ctx.canvas_2d().pop_clip();
        }
    }
}
