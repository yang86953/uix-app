use crate::core::Rect;
// 引入表头分层绘制需要的颜色与圆角类型。
use crate::draw::{Color, Radius};
use crate::ui::component::paint_context::PaintContext;

// 引入共享列区绘制层级与列几何快照。
use super::Table;
use super::geometry::{COLUMN_PAINT_ORDER, TableColumnGeometry};
use super::types::SortDirection;

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
    let text_secondary = ctx.tokens().color_text_secondary();
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

    // 通过共享阶段访问器驱动真实绘制，使测试能够直接约束跨层片段顺序。
    visit_header_zone_layers(
        // 把绘制上下文作为两个阶段共享的可变状态。
        ctx,
        // 分组阶段绘制当前列区的上层标题片段。
        |ctx, zone| paint_group_titles_for_zone(table, header_rect, ctx, geometry, zone),
        // 叶阶段绘制当前列区的跨层单列或下层叶表头。
        |ctx, zone| paint_leaf_headers_for_zone(table, header_rect, ctx, geometry, zone),
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
) {
    // 不可见列区不产生任何叶表头绘制。
    let Some(zone_clip) = geometry.clip_for(zone, header_rect.y, header_rect.h) else {
        // 提前结束当前列区。
        return;
    };
    // 读取叶表头边框颜色。
    let border: Color = ctx.tokens().color_border();
    // 读取叶表头文字颜色。
    let text_color: Color = ctx.tokens().color_text();
    // 读取未激活排序图标颜色。
    let text_secondary: Color = ctx.tokens().color_text_secondary();
    // 读取激活排序与调整线颜色。
    let primary: Color = ctx.tokens().color_primary();
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
                ctx.fill_rect(pressed_frame, ctx.tokens().color_fill_secondary(), None);
            }
        }
        // 为可排序列预留右侧图标槽。
        let sort_slot = if table.sortable || column.sortable {
            // 图标槽不超过列宽的四成。
            24.0_f32.min(laid_out.width * 0.4)
        } else {
            // 不可排序列不预留图标槽。
            0.0
        };
        // 水平留白随窄列宽度收敛。
        let horizontal_inset = 8.0_f32.min(laid_out.width * 0.25);
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
            Table::paint_single_line(ctx, &column.title, text_frame, text_color, 13.0);
        }
        // 表级或列级排序开启时绘制方向图标。
        if table.sortable || column.sortable {
            // 根据当前排序方向选择图标。
            let indicator = match column.sort_direction {
                // 升序使用向上图标。
                SortDirection::Asc => "chevron-up",
                // 降序和未排序使用向下图标。
                SortDirection::Desc | SortDirection::None => "chevron-down",
            };
            // 根据激活状态选择图标颜色与字号。
            let (color, font_size) = if column.sort_direction == SortDirection::None {
                // 未排序状态使用次要颜色与较小字号。
                (text_secondary, 10.0)
            } else {
                // 已排序状态使用主色与稍大字号。
                (primary, 11.0)
            };
            // 图标宽度同时受槽位和列宽限制。
            let icon_width = 18.0_f32.min(sort_slot).min(laid_out.width);
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
                            - 4.0_f32.min(sort_slot * 0.25),
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
                    laid_out.x + laid_out.width - 1.0,
                    // 分隔线从当前叶层顶部开始。
                    leaf_rect.y,
                    // 分隔线宽度固定一像素。
                    1.0,
                    // 分隔线高度等于当前叶层。
                    leaf_rect.h,
                ),
                // 使用共享边框颜色。
                border,
                // 竖线无需圆角。
                None,
            );
        }
        // 可调整列绘制调整线。
        if column.resizable {
            // 判断当前列是否为激活调整目标。
            let active = table.resize_indicator_column() == Some(laid_out.index);
            // 激活调整线使用两像素，否则使用一像素。
            let indicator_width = if active { 2.0 } else { 1.0 };
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
                if active { primary } else { border },
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
) {
    // 读取分组表头边框颜色。
    let border = ctx.tokens().color_border();
    // 读取分组表头文字颜色。
    let text_color = ctx.tokens().color_text();
    // 计算上层分组表头实际矩形。
    let group_rect = Rect::new(header_rect.x, header_rect.y, header_rect.w, table.header_h);
    // 无分配访问每个可见分组片段并立即绘制。
    visit_group_title_segments_for_zone(table, group_rect, geometry, zone, |title, segment| {
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

    // 标记跨两层单列必须服从全局列区层级的契约。
    #[test]
    // 验证较低层左固定单列不能覆盖右固定分组的上层标题。
    fn spanning_leaf_header_cannot_cover_higher_fixed_group_title() {
        // 构造声明在前且位于最高列区的右固定分组。
        let right_group = TableColumnGroup::new(
            // 设置可识别的分组标题。
            "右组",
            // 放入一个八十像素右固定列。
            vec![TableColumn::new("右列", 80.0).fixed(Fixed::Right)],
        );
        // 构造声明在后且跨越两层的左固定单列。
        let left_column = TableColumnGroup::column(
            // 单列宽度同为八十像素，从而与右固定区重叠。
            TableColumn::new("左列", 80.0).fixed(Fixed::Left),
        );
        // 按右分组在前、左单列在后的顺序构造两层表头。
        let table = Table::new().column_groups(vec![right_group, left_column]);
        // 在一百像素视口中形成二十到八十像素的重叠区。
        let geometry = table.column_geometry(0.0, 100.0);
        // 构造覆盖两层的完整表头矩形。
        let header_rect = Rect::new(0.0, 0.0, 100.0, table.total_header_height());
        // 构造仅覆盖上层分组标题的矩形。
        let group_rect = Rect::new(0.0, 0.0, 100.0, table.header_h);
        // 记录经过重叠点的真实阶段片段访问顺序。
        let mut overlap_paints = Vec::new();
        // 使用生产绘制调用的共享阶段访问器。
        visit_header_zone_layers(
            // 以访问顺序集合承载两个阶段的共享状态。
            &mut overlap_paints,
            // 记录经过上层重叠点的分组标题片段。
            |paints, zone| {
                // 访问当前列区的真实分组片段几何。
                visit_group_title_segments_for_zone(
                    // 传入当前表格声明。
                    &table,
                    // 传入上层分组矩形。
                    group_rect,
                    // 传入共享列几何。
                    &geometry,
                    // 限定当前阶段列区。
                    zone,
                    // 只记录覆盖横坐标五十、纵坐标十的标题片段。
                    |title, segment| {
                        // 检查目标点是否位于当前分组片段中。
                        if 50.0 >= segment.x
                            // 排除右开边界之外的点。
                            && 50.0 < segment.x + segment.w
                            // 检查目标点位于片段顶部之后。
                            && 10.0 >= segment.y
                            // 排除底部右开边界之外的点。
                            && 10.0 < segment.y + segment.h
                        {
                            // 记录可识别的分组标题。
                            paints.push(format!("分组:{title}"));
                        }
                    },
                );
            },
            // 记录经过同一上层重叠点的叶表头片段。
            |paints, zone| {
                // 按生产叶表头路径访问当前列区的已布局列。
                for laid_out in geometry.columns.iter().filter(|column| column.zone == zone) {
                    // 判断当前列是否属于有标题分组。
                    let grouped = table
                        // 访问全部分组范围。
                        .column_groups
                        // 转为稳定迭代器。
                        .iter()
                        // 查找覆盖当前扁平列索引的分组。
                        .find(|group| {
                            // 同时约束分组起点与末端。
                            laid_out.index >= group.start
                                // 继续检查当前索引位于分组末端之前。
                                && laid_out.index < group.start + group.len
                        })
                        // 只有有标题分组才使用下层叶矩形。
                        .is_some_and(|group| group.title.is_some());
                    // 复现生产路径中的跨层单列与下层叶矩形选择。
                    let leaf_rect = if grouped {
                        // 有标题分组的叶表头只占下半层。
                        Rect::new(
                            // 继承完整表头左边界。
                            header_rect.x,
                            // 从一层表头高度之后开始。
                            header_rect.y + table.header_h,
                            // 继承完整表头宽度。
                            header_rect.w,
                            // 叶层高度等于单层表头高度。
                            table.header_h,
                        )
                    } else {
                        // 无标题单列跨越完整两层。
                        header_rect
                    };
                    // 检查当前列水平范围与叶矩形是否共同覆盖目标点。
                    if 50.0 >= laid_out.x
                        // 排除列水平右开边界之外的点。
                        && 50.0 < laid_out.x + laid_out.width
                        // 检查目标点位于叶矩形顶部之后。
                        && 10.0 >= leaf_rect.y
                        // 排除叶矩形底部右开边界之外的点。
                        && 10.0 < leaf_rect.y + leaf_rect.h
                    {
                        // 记录可识别的叶表头标题。
                        paints.push(format!("单列:{}", table.columns[laid_out.index].title));
                    }
                }
            },
        );
        // 目标点应先经过低层左单列，最后由高层右分组标题覆盖。
        assert_eq!(
            // 比较经过目标点的完整片段顺序。
            overlap_paints,
            // 期望顺序与 Middle 到 Left 到 Right 的全局层级一致。
            vec!["单列:左列".to_owned(), "分组:右组".to_owned()]
        );
        // 同一点的列命中必须继续落在视觉最上层右固定列。
        assert_eq!(geometry.column_at(50.0), Some(0));
        // 结束跨两层单列表头层级契约。
    }
    // 结束分组表头测试模块。
}
