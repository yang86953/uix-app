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
    // 读取分组表头边框颜色。
    let border = ctx.tokens().color_border();
    // 读取分组表头文字颜色。
    let text_color = ctx.tokens().color_text();
    // 计算上层分组表头实际矩形。
    let group_rect = Rect::new(header_rect.x, header_rect.y, header_rect.w, table.header_h);
    // 无分配访问每个可见分组片段并立即绘制。
    visit_group_title_segments(table, group_rect, geometry, |title, _, segment| {
        // 将后续文字、底边和列边界裁剪在当前分组片段内。
        ctx.push_clip(segment);
        // 按片段宽度收敛分组标题水平留白。
        let horizontal_inset = 8.0_f32.min(segment.w * 0.25);
        // 在裁剪后的分组片段中绘制单行标题。
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
        // 绘制分组表头与叶表头之间的底部分隔线。
        ctx.fill_rect(
            Rect::new(segment.x, segment.y + segment.h - 1.0, segment.w, 1.0),
            border,
            None,
        );
        // 仅在带边框表格中绘制分组片段右边界。
        if table.bordered {
            // 将右边界限制在当前分组片段高度内。
            ctx.fill_rect(
                Rect::new(segment.x + segment.w - 1.0, segment.y, 1.0, segment.h),
                border,
                None,
            );
        }
        // 恢复进入分组片段前的裁剪状态。
        ctx.pop_clip();
        // 结束当前可见分组片段绘制。
    });
}

// 按全局列区层级访问可见分组表头片段，生产绘制路径不分配中间集合。
fn visit_group_title_segments(
    // 提供分组声明及列配置。
    table: &Table,
    // 提供上层分组表头的实际矩形。
    group_rect: Rect,
    // 提供各列在当前视口中的共享几何。
    geometry: &TableColumnGeometry,
    // 接收标题、列区与裁剪后片段。
    mut visit: impl FnMut(&str, super::geometry::ColumnZone, Rect),
) {
    // 先按共享全局列区层级遍历，避免后声明的低层分组覆盖高层固定区。
    for zone in COLUMN_PAINT_ORDER {
        // 取得当前列区在分组表头高度内的共同可见裁剪。
        let Some(zone_clip) = geometry.clip_for(zone, group_rect.y, group_rect.h) else {
            // 不可见列区不访问任何分组片段。
            continue;
        };
        // 在同一列区内保持分组声明顺序稳定。
        for group in &table.column_groups {
            // 跳过显式声明为跨两层单列的无标题分组。
            let Some(title) = group.title.as_deref() else {
                // 继续访问当前列区的后续分组。
                continue;
            };
            // 筛选当前分组且属于当前列区的已布局列。
            let mut members = geometry.columns.iter().filter(|column| {
                // 同时约束列区和扁平列索引范围。
                column.zone == zone
                    && column.index >= group.start
                    && column.index < group.start + group.len
            });
            // 没有当前列区成员时跳过该片段。
            let Some(first) = members.next() else {
                // 继续检查当前列区的后续分组。
                continue;
            };
            // 用首列初始化片段左边界。
            let mut left = first.x;
            // 用首列末端初始化片段右边界。
            let mut right = first.x + first.width;
            // 折叠同一分组同一列区内其余列的水平范围。
            for member in members {
                // 扩展到当前成员的最左坐标。
                left = left.min(member.x);
                // 扩展到当前成员的最右坐标。
                right = right.max(member.x + member.width);
            }
            // 把分组水平范围收敛到当前列区可见裁剪内。
            let Some(segment) =
                Rect::new(left, group_rect.y, right - left, group_rect.h).intersect(&zone_clip)
            else {
                // 完全位于裁剪外的分组片段不参与绘制。
                continue;
            };
            // 将可见片段交给调用方立即处理。
            visit(title, zone, segment);
        }
    }
}

// 仅在单元测试中编译分组表头层级契约。
#[cfg(test)]
// 将分组表头重叠绘制顺序回归收拢在表头模块。
mod tests {
    // 复用分组片段访问器和父模块导入的几何类型。
    use super::*;
    // 引入固定列、表格列与分组声明类型。
    use super::super::types::{Fixed, TableColumn, TableColumnGroup};

    // 标记分组声明顺序不得覆盖全局列区层级的契约。
    #[test]
    // 验证右固定组即使声明在前也必须最后绘制。
    fn overlapping_group_headers_follow_global_zone_paint_order() {
        // 构造声明在前且位于最上层的右固定分组。
        let right_group = TableColumnGroup::new(
            // 设置可识别的右组标题。
            "右组",
            // 放入一个宽于剩余视口的右固定列。
            vec![TableColumn::new("右列", 80.0).fixed(Fixed::Right)],
        );
        // 构造声明在后但视觉层级较低的左固定分组。
        let left_group = TableColumnGroup::new(
            // 设置可识别的左组标题。
            "左组",
            // 放入一个同样宽的左固定列。
            vec![TableColumn::new("左列", 80.0).fixed(Fixed::Left)],
        );
        // 按右组在前、左组在后的顺序构造表格。
        let table = Table::new().column_groups(vec![right_group, left_group]);
        // 在一百像素视口中形成二十到八十像素的固定区重叠。
        let geometry = table.column_geometry(0.0, 100.0);
        // 收集生产访问器产生的列区绘制顺序。
        let mut painted_zones = Vec::new();
        // 使用真实分组表头矩形访问所有可见片段。
        visit_group_title_segments(
            // 传入当前表格声明。
            &table,
            // 两层表头的上半层使用默认表头高度。
            Rect::new(0.0, 0.0, 100.0, table.header_h),
            // 传入与命中测试相同的列几何。
            &geometry,
            // 只记录每个片段所属列区。
            |_, zone, _| painted_zones.push(zone),
        );
        // 全局层级要求左固定区先绘制、右固定区最后覆盖。
        assert_eq!(
            painted_zones,
            vec![
                // 左固定分组应先进入绘制队列。
                super::super::geometry::ColumnZone::Left,
                // 右固定分组必须最后进入绘制队列。
                super::super::geometry::ColumnZone::Right,
            ]
        );
        // 重叠点命中仍以最后绘制的右固定列为准。
        assert_eq!(geometry.column_at(50.0), Some(0));
        // 结束分组表头全局列区层级契约。
    }
    // 结束分组表头测试模块。
}
