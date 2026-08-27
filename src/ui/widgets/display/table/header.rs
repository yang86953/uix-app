use crate::core::Rect;
// 引入表头分层绘制需要的颜色与圆角类型。
use crate::draw::Radius;
use crate::ui::widget_runtime::paint_context::PaintContext;

// 引入共享列区绘制层级与列几何快照。
use super::ResolvedTableVisual;
use super::Table;
use super::geometry::{COLUMN_PAINT_ORDER, TableColumnGeometry};
use super::types::SortDirection;

pub(super) fn paint(
    table: &Table,
    frame: Rect,
    ctx: &mut PaintContext,
    geometry: &TableColumnGeometry,
    resolved: ResolvedTableVisual,
) {
    let header_height = table.total_header_height();
    let header_rect = Rect::new(frame.x, frame.y, frame.w, header_height);
    let frame_visual = table.visual.frame;
    let radius = Some(Radius::uniform(resolved.radius));

    ctx.fill_rect(header_rect, resolved.header_background, radius);
    if table.selection {
        if table.header_selection_pressed() {
            ctx.fill_rect(
                Rect::new(
                    header_rect.x,
                    header_rect.y,
                    table.selection_width().min(header_rect.w),
                    header_rect.h,
                ),
                resolved.pressed_background,
                None,
            );
        }
        let all_checked = table.checked_rows.len() == table.rows.len();
        crate::ui::widgets::Icon::paint_in_frame(
            ctx,
            if all_checked {
                frame_visual.checked_icon
            } else {
                frame_visual.unchecked_icon
            },
            Rect::new(
                frame.x + frame_visual.selection_icon_x,
                header_rect.y,
                frame_visual.selection_icon_width,
                header_rect.h,
            ),
            resolved.text_secondary,
            frame_visual.selection_icon_size,
        );
        if table.bordered {
            ctx.fill_rect(
                Rect::new(
                    frame.x + table.selection_width() - frame_visual.selection_divider_width,
                    header_rect.y,
                    frame_visual.selection_divider_width,
                    header_rect.h,
                ),
                resolved.border,
                None,
            );
        }
    }

    // 通过共享阶段访问器驱动真实绘制，使测试能够直接约束跨层片段顺序。
    visit_header_zone_layers(
        // 把绘制上下文作为两个阶段共享的可变状态。
        ctx,
        // 分组阶段绘制当前列区的上层标题片段。
        |ctx, zone| paint_group_titles_for_zone(table, header_rect, ctx, geometry, zone, resolved),
        // 叶阶段绘制当前列区的跨层单列或下层叶表头。
        |ctx, zone| paint_leaf_headers_for_zone(table, header_rect, ctx, geometry, zone, resolved),
    );
}

// 按全局列区层级访问分组标题与叶表头阶段。
fn visit_header_zone_layers<State>(
    // 提供两个绘制阶段共享的可变状态。
    state: &mut State,
    // 接收当前分组标题列区。
    mut visit_groups: impl FnMut(&mut State, super::geometry::ColumnZone),
    // 接收当前叶表头列区。
    mut visit_leaves: impl FnMut(&mut State, super::geometry::ColumnZone),
) {
    // 先完成当前列区全部表头片段，再进入视觉层级更高的列区。
    for zone in COLUMN_PAINT_ORDER {
        // 访问当前列区的分组标题。
        visit_groups(state, zone);
        // 紧接着访问同一列区的叶表头，避免低层跨层单列越级覆盖。
        visit_leaves(state, zone);
    }
}

// 绘制单个列区中的全部叶表头片段。
fn paint_leaf_headers_for_zone(
    // 提供表格声明与交互状态。
    table: &Table,
    // 提供完整表头矩形。
    header_rect: Rect,
    // 接收叶表头绘制命令。
    ctx: &mut PaintContext,
    // 提供当前视口共享列几何。
    geometry: &TableColumnGeometry,
    // 限定本次绘制的列区。
    zone: super::geometry::ColumnZone,
    resolved: ResolvedTableVisual,
) {
    // 不可见列区不产生任何叶表头绘制。
    let Some(zone_clip) = geometry.clip_for(zone, header_rect.y, header_rect.h) else {
        // 提前结束当前列区。
        return;
    };
    let header_visual = table.visual.header;
    // 将当前列区全部叶表头裁剪到共同可见范围。
    ctx.push_clip(zone_clip);
    // 按扁平列声明顺序绘制当前列区。
    for laid_out in geometry.columns.iter().filter(|column| column.zone == zone) {
        // 判断当前列是否位于有标题分组中。
        let grouped = table
            // 访问全部分组范围。
            .column_groups
            // 转为稳定迭代器。
            .iter()
            // 查找覆盖当前扁平列索引的分组。
            .find(|group| {
                // 同时检查范围起点与末端。
                laid_out.index >= group.start && laid_out.index < group.start + group.len
            })
            // 只有带标题分组才把叶表头放入下半层。
            .is_some_and(|group| group.title.is_some());
        // 有标题分组使用下层高度，无标题单列跨越完整两层。
        let leaf_rect = if grouped {
            // 构造两层表头的下半层矩形。
            Rect::new(
                // 继承完整表头左边界。
                header_rect.x,
                // 从一层表头高度之后开始。
                header_rect.y + table.header_h,
                // 继承完整表头宽度。
                header_rect.w,
                // 叶层使用一层表头高度。
                table.header_h,
            )
        } else {
            // 无标题单列占据完整表头高度。
            header_rect
        };
        // 读取当前扁平列声明。
        let column = &table.columns[laid_out.index];
        // 被按下的排序列绘制反馈底色。
        if table.pressed_sort_column() == Some(laid_out.index) {
            // 将反馈矩形限制到当前列区。
            if let Some(pressed_frame) =
                // 以当前列实际水平几何构造反馈范围。
                Rect::new(laid_out.x, leaf_rect.y, laid_out.width, leaf_rect.h)
                        // 与当前列区可见裁剪求交。
                        .intersect(&zone_clip)
            {
                // 绘制按下反馈。
                ctx.fill_rect(pressed_frame, resolved.pressed_background, None);
            }
        }
        // 为可排序列预留右侧图标槽。
        let sort_slot = if table.sortable || column.sortable {
            // 图标槽不超过列宽的四成。
            header_visual
                .sort_slot
                .min(laid_out.width * header_visual.sort_slot_ratio)
        } else {
            // 不可排序列不预留图标槽。
            0.0
        };
        // 水平留白随窄列宽度收敛。
        let horizontal_inset = header_visual
            .horizontal_inset
            .min(laid_out.width * header_visual.horizontal_inset_ratio);
        // 仅绘制与当前列区相交的文字范围。
        if let Some(text_frame) = Rect::new(
            // 文字从列左边界加留白开始。
            laid_out.x + horizontal_inset,
            // 文字垂直范围遵循当前叶层。
            leaf_rect.y,
            // 扣除两侧留白与排序图标槽。
            (laid_out.width - horizontal_inset * 2.0 - sort_slot).max(0.0),
            // 继承叶层高度。
            leaf_rect.h,
        )
        // 将文字矩形裁剪到当前列区。
        .intersect(&zone_clip)
        {
            // 绘制单行列标题。
            Table::paint_single_line(
                ctx,
                &column.title,
                text_frame,
                resolved.text,
                header_visual.title_font_size,
            );
        }
        // 表级或列级排序开启时绘制方向图标。
        if table.sortable || column.sortable {
            // 根据当前排序方向选择图标。
            let indicator = match column.sort_direction {
                // 升序使用向上图标。
                SortDirection::Asc => header_visual.ascending_icon,
                // 降序和未排序使用向下图标。
                SortDirection::Desc => header_visual.descending_icon,
                SortDirection::None => header_visual.unsorted_icon,
            };
            // 根据激活状态选择图标颜色与字号。
            let (color, font_size) = if column.sort_direction == SortDirection::None {
                // 未排序状态使用次要颜色与较小字号。
                (resolved.text_secondary, header_visual.unsorted_icon_size)
            } else {
                // 已排序状态使用主色与稍大字号。
                (resolved.primary, header_visual.sorted_icon_size)
            };
            // 图标宽度同时受槽位和列宽限制。
            let icon_width = header_visual
                .sort_icon_width
                .min(sort_slot)
                .min(laid_out.width);
            // 只为正宽图标提交绘制。
            if icon_width > 0.0 {
                // 在列右侧图标槽中绘制排序图标。
                crate::ui::widgets::Icon::paint_in_frame(
                    // 提供当前绘制上下文。
                    ctx,
                    // 提供已解析的图标名称。
                    indicator,
                    // 构造图标绘制矩形。
                    Rect::new(
                        // 从列右边界向左扣除图标及动态留白。
                        laid_out.x + laid_out.width
                            // 扣除图标宽度。
                            - icon_width
                            // 扣除图标槽四分之一且最多四像素的留白。
                            - header_visual
                                .sort_icon_right
                                .min(sort_slot * header_visual.sort_icon_right_ratio),
                        // 图标遵循当前叶层纵坐标。
                        leaf_rect.y,
                        // 使用收敛后的图标宽度。
                        icon_width,
                        // 图标占据当前叶层高度。
                        leaf_rect.h,
                    ),
                    // 使用当前排序状态颜色。
                    color,
                    // 使用当前排序状态字号。
                    font_size,
                );
            }
        }
        // 带边框表格绘制列右边界。
        if table.bordered {
            // 提交一像素竖向分隔线。
            ctx.fill_rect(
                // 将分隔线放在列实际右边缘。
                Rect::new(
                    // 右边缘向左一像素。
                    laid_out.x + laid_out.width - header_visual.divider_width,
                    // 分隔线从当前叶层顶部开始。
                    leaf_rect.y,
                    // 分隔线宽度固定一像素。
                    header_visual.divider_width,
                    // 分隔线高度等于当前叶层。
                    leaf_rect.h,
                ),
                // 使用共享边框颜色。
                resolved.border,
                // 竖线无需圆角。
                None,
            );
        }
        // 可调整列绘制调整线。
        if column.resizable {
            // 判断当前列是否为激活调整目标。
            let active = table.resize_indicator_column() == Some(laid_out.index);
            // 激活调整线使用两像素，否则使用一像素。
            let indicator_width = if active {
                header_visual.resize_active_indicator_width
            } else {
                header_visual.resize_indicator_width
            };
            // 在列右边缘居中绘制调整线。
            ctx.fill_rect(
                // 构造当前调整线矩形。
                Rect::new(
                    // 让调整线中心对齐列右边缘。
                    laid_out.x + laid_out.width - indicator_width * 0.5,
                    // 调整线从当前叶层顶部开始。
                    leaf_rect.y,
                    // 使用激活状态对应宽度。
                    indicator_width,
                    // 调整线高度等于当前叶层。
                    leaf_rect.h,
                ),
                // 激活时使用主色，否则使用边框色。
                if active {
                    resolved.primary
                } else {
                    resolved.border
                },
                // 调整线无需圆角。
                None,
            );
        }
    }
    // 恢复进入当前列区前的裁剪状态。
    ctx.pop_clip();
}

// 绘制单个列区中的全部有标题分组片段。
fn paint_group_titles_for_zone(
    table: &Table,
    header_rect: Rect,
    ctx: &mut PaintContext,
    geometry: &TableColumnGeometry,
    // 限定本次绘制的列区。
    zone: super::geometry::ColumnZone,
    resolved: ResolvedTableVisual,
) {
    let header_visual = table.visual.header;
    // 计算上层分组表头实际矩形。
    let group_rect = Rect::new(header_rect.x, header_rect.y, header_rect.w, table.header_h);
    // 无分配访问每个可见分组片段并立即绘制。
    visit_group_title_segments_for_zone(table, group_rect, geometry, zone, |title, segment| {
        // 将后续文字、底边和列边界裁剪在当前分组片段内。
        ctx.push_clip(segment);
        // 按片段宽度收敛分组标题水平留白。
        let horizontal_inset = header_visual
            .horizontal_inset
            .min(segment.w * header_visual.horizontal_inset_ratio);
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
            resolved.text,
            header_visual.title_font_size,
        );
        // 绘制分组表头与叶表头之间的底部分隔线。
        ctx.fill_rect(
            Rect::new(
                segment.x,
                segment.y + segment.h - header_visual.divider_width,
                segment.w,
                header_visual.divider_width,
            ),
            resolved.border,
            None,
        );
        // 仅在带边框表格中绘制分组片段右边界。
        if table.bordered {
            // 将右边界限制在当前分组片段高度内。
            ctx.fill_rect(
                Rect::new(
                    segment.x + segment.w - header_visual.divider_width,
                    segment.y,
                    header_visual.divider_width,
                    segment.h,
                ),
                resolved.border,
                None,
            );
        }
        // 恢复进入分组片段前的裁剪状态。
        ctx.pop_clip();
        // 结束当前可见分组片段绘制。
    });
}

// 仅为全局分组片段测试保留跨列区访问器。
#[cfg(test)]
// 按全局列区层级访问可见分组表头片段且不分配中间集合。
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
        // 复用单列区访问器并补回调用方需要的列区信息。
        visit_group_title_segments_for_zone(
            // 传入当前表格声明。
            table,
            // 传入上层分组表头矩形。
            group_rect,
            // 传入共享列几何。
            geometry,
            // 限定当前全局列区。
            zone,
            // 将标题、列区与片段转发给调用方。
            |title, segment| visit(title, zone, segment),
        );
    }
}

// 访问单个列区中的全部可见分组标题片段。
fn visit_group_title_segments_for_zone(
    // 提供分组声明及列配置。
    table: &Table,
    // 提供上层分组表头的实际矩形。
    group_rect: Rect,
    // 提供各列在当前视口中的共享几何。
    geometry: &TableColumnGeometry,
    // 限定本次访问的列区。
    zone: super::geometry::ColumnZone,
    // 接收标题与裁剪后片段。
    mut visit: impl FnMut(&str, Rect),
) {
    // 取得当前列区在分组表头高度内的共同可见裁剪。
    let Some(zone_clip) = geometry.clip_for(zone, group_rect.y, group_rect.h) else {
        // 不可见列区不访问任何分组片段。
        return;
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
                // 约束分组范围起点。
                && column.index >= group.start
                // 约束分组范围末端。
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
            // 以当前分组成员水平包围盒构造片段。
            Rect::new(left, group_rect.y, right - left, group_rect.h)
                // 将片段限制在当前列区可见裁剪中。
                .intersect(&zone_clip)
        else {
            // 完全位于裁剪外的分组片段不参与绘制。
            continue;
        };
        // 将可见片段交给调用方立即处理。
        visit(title, segment);
    }
}
