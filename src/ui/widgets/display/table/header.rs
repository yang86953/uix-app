use crate::core::Rect;
use crate::draw::Radius;
use crate::ui::component::paint_context::PaintContext;

// 引入共享列区绘制层级与列几何快照。
use super::geometry::{TableColumnGeometry, COLUMN_PAINT_ORDER};
use super::types::SortDirection;
use super::Table;

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
        if table.header_selection_pressed() {
            ctx.fill_rect(
                Rect::new(
                    header_rect.x,
                    header_rect.y,
                    table.selection_width().min(header_rect.w),
                    header_rect.h,
                ),
                ctx.tokens().color_fill_secondary(),
                None,
            );
        }
        let all_checked = table.checked_rows.len() == table.rows.len();
        crate::ui::widgets::Icon::paint_in_frame(
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

    // 按共享层级绘制叶表头，确保固定区覆盖关系与命中一致。
    for zone in COLUMN_PAINT_ORDER {
        let Some(zone_clip) = geometry.clip_for(zone, header_rect.y, header_rect.h) else {
            continue;
        };
        ctx.push_clip(zone_clip);
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
            if table.pressed_sort_column() == Some(laid_out.index) {
                if let Some(pressed_frame) =
                    Rect::new(laid_out.x, leaf_rect.y, laid_out.width, leaf_rect.h)
                        .intersect(&zone_clip)
                {
                    ctx.fill_rect(pressed_frame, ctx.tokens().color_fill_secondary(), None);
                }
            }
            let sort_slot = if table.sortable || column.sortable {
                24.0_f32.min(laid_out.width * 0.4)
            } else {
                0.0
            };
            let horizontal_inset = 8.0_f32.min(laid_out.width * 0.25);
            if let Some(text_frame) = Rect::new(
                laid_out.x + horizontal_inset,
                leaf_rect.y,
                (laid_out.width - horizontal_inset * 2.0 - sort_slot).max(0.0),
                leaf_rect.h,
            )
            .intersect(&zone_clip)
            {
                Table::paint_single_line(ctx, &column.title, text_frame, text_color, 13.0);
            }
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
                let icon_width = 18.0_f32.min(sort_slot).min(laid_out.width);
                if icon_width > 0.0 {
                    crate::ui::widgets::Icon::paint_in_frame(
                        ctx,
                        indicator,
                        Rect::new(
                            laid_out.x + laid_out.width
                                - icon_width
                                - 4.0_f32.min(sort_slot * 0.25),
                            leaf_rect.y,
                            icon_width,
                            leaf_rect.h,
                        ),
                        color,
                        font_size,
                    );
                }
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
            if column.resizable {
                let active = table.resize_indicator_column() == Some(laid_out.index);
                let indicator_width = if active { 2.0 } else { 1.0 };
                ctx.fill_rect(
                    Rect::new(
                        laid_out.x + laid_out.width - indicator_width * 0.5,
                        leaf_rect.y,
                        indicator_width,
                        leaf_rect.h,
                    ),
                    if active { primary } else { border },
                    None,
                );
            }
        }
        ctx.pop_clip();
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
    for group in &table.column_groups {
        let Some(title) = group.title.as_deref() else {
            continue;
        };
        // 分组表头复用叶表头的列区层级。
        for zone in COLUMN_PAINT_ORDER {
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
            ctx.push_clip(segment);
            let horizontal_inset = 8.0_f32.min(segment.w * 0.25);
            Table::paint_single_line(
                ctx,
                title,
                Rect::new(
                    segment.x + horizontal_inset,
                    segment.y,
                    (segment.w - horizontal_inset * 2.0).max(0.0),
                    segment.h,
                ),
                text_color,
                13.0,
            );
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
            ctx.pop_clip();
        }
    }
}
