//! Table 组件测试 — table 子模块。

use super::*;

// 测试断言需要独立拥有片段值时，在访问器作用域内显式创建快照。
fn parent_clip_regions_snapshot(tree: &WidgetTree, id: crate::core::WidgetId) -> Option<Vec<Rect>> {
    let node = tree.get(id)?;
    let mut snapshot = None;
    node.visit_parent_clip_regions(&mut |regions| {
        snapshot = Some(regions.to_vec());
    });
    snapshot
}

#[derive(Clone)]
struct UserRow {
    id: u64,
    name: String,
}

fn data_table(rows: Vec<UserRow>) -> DataTable<UserRow> {
    let Ok(table) = Table::data(rows.clone(), |row| row.id.to_string()) else {
        panic!("Table::data must accept a stable id extractor");
    };
    table
        .columns(vec![
            TableColumn::new("ID", 80.0).bind(|r: &UserRow| r.id.to_string()),
        ])
        .row_height(32.0)
}

#[test]
fn virtualized_true_materializes_viewport_only() {
    let rows = (0..1000)
        .map(|id| UserRow {
            id,
            name: format!("row-{id}"),
        })
        .collect::<Vec<_>>();
    // 读取示例行名称，确保 fixture 的展示字段也参与测试契约。
    assert_eq!(rows[0].name, "row-0");
    let data = data_table(rows).virtualized(true);
    let (table, _, _) = data.into_parts();

    // 1000 行 + 32px 行高 + 320px 视口：只物化可视区与 overscan。
    let (start, end) = table.visible_row_range(320.0);
    assert!(
        end - start <= 30,
        "虚拟化应限制物化行数，实际 {}",
        end - start
    );
    assert!(end > start);
    assert_eq!(table.row_keys().len(), 1000, "行身份仍保留全量");
}

#[test]
fn virtualized_false_materializes_all_rows() {
    let rows = (0..1000)
        .map(|id| UserRow {
            id,
            name: format!("row-{id}"),
        })
        .collect::<Vec<_>>();
    let data = data_table(rows).virtualized(false);
    let (table, _, _) = data.into_parts();

    assert_eq!(table.visible_row_range(320.0), (0, 1000));
}

#[test]
fn virtualized_is_alias_of_virtual_scroll() {
    let rows = (0..100)
        .map(|id| UserRow {
            id,
            name: format!("row-{id}"),
        })
        .collect::<Vec<_>>();
    let on = data_table(rows.clone()).virtualized(true);
    let (table_on, _, _) = on.into_parts();
    assert!(table_on.virtual_scroll);

    let off = data_table(rows).virtualized(false);
    let (table_off, _, _) = off.into_parts();
    assert!(!table_off.virtual_scroll);
}

// 泛型表格的命令式转换必须与普通 View 构建共享 UIX 视觉根。
#[test]
fn data_table_into_widget_node_uses_uix_visual() {
    let direct = Table::new();
    assert!(std::ptr::eq(direct.visual, TABLE_VISUAL_REF));
    let rows = vec![UserRow {
        id: 1,
        name: "row-1".to_string(),
    }];
    let node = crate::ui::IntoWidgetNode::into_node(data_table(rows));
    let table = node
        .widget
        .as_any()
        .downcast_ref::<Table>()
        .expect("DataTable 必须转换为 Table 内核");
    assert!(std::ptr::eq(table.visual, TABLE_VISUAL_REF));
    // 调用方显式声明的行高不能被 UIX 默认值覆盖。
    assert_eq!(table.row_h, 32.0);
}

// 标记跨固定区列合并的命中必须回落到唯一锚点。
#[test]
// 验证视觉左侧覆盖列与视觉右侧锚点返回同一逻辑单元格。
fn cross_zone_col_span_hit_uses_single_anchor() {
    // 构造先声明且固定在视觉右侧的两列合并锚点。
    let right_anchor = TableColumn::new("右侧锚点", 60.0)
        // 让首列覆盖逻辑上紧随其后的左固定列。
        .col_span(|_row, column| if column == 0 { 2 } else { 1 })
        // 将锚点固定在视口右侧。
        .fixed(Fixed::Right);
    // 构造后声明但固定在视觉左侧的覆盖列。
    let left_covered = TableColumn::new("左侧覆盖列", 40.0).fixed(Fixed::Left);
    // 建立包含单行数据的公开表格声明。
    let table = Table::new()
        // 保留右侧锚点先于左侧覆盖列的逻辑顺序。
        .columns(vec![right_anchor, left_covered])
        // 提供锚点文本与不会单独显示的覆盖列文本。
        .rows(vec![vec!["锚点".to_string(), "覆盖".to_string()]]);
    // 为一百像素视口解析左右固定列的真实物理位置。
    let geometry = table.column_geometry(0.0, 100.0);
    // 视觉左侧十像素先命中后声明的左固定列索引一。
    assert_eq!(geometry.column_at(10.0), Some(1));
    // 命中覆盖列后必须继续解析为逻辑首列的唯一合并锚点。
    assert_eq!(
        // 组合视觉列命中与表格合并锚点解析。
        geometry
            // 先取得视觉最上层列索引。
            .column_at(10.0)
            // 再把覆盖列回落到所属合并锚点。
            .and_then(|column| table.cell_anchor(0, column)),
        // 左右两片都属于首行首列的单一逻辑单元格。
        Some((0, 0))
    );
    // 结束跨固定区列合并命中契约。
}

// 标记自定义 View 必须按完整逻辑合并矩形布局。
#[test]
// 验证跨固定区 View 不再收缩到锚点所属的单一列区片段。
fn cross_zone_view_cell_uses_full_span_layout_frame() {
    // 构造先声明且固定在视觉右侧的两列合并锚点。
    let right_anchor = TableColumn::new("右侧视图锚点", 60.0)
        // 让首列覆盖逻辑上紧随其后的左固定列。
        .col_span(|_row, column| if column == 0 { 2 } else { 1 })
        // 将自定义 View 锚点固定到视口右侧。
        .fixed(Fixed::Right);
    // 构造后声明但固定在视觉左侧的覆盖列。
    let left_covered = TableColumn::new("左侧覆盖列", 40.0).fixed(Fixed::Left);
    // 建立包含单行数据的表格并允许直接指定 View 列。
    let mut table = Table::new()
        // 保留右侧锚点先于左侧覆盖列的逻辑顺序。
        .columns(vec![right_anchor, left_covered])
        // 提供单行数据以物化首个自定义单元格。
        .rows(vec![vec!["视图".to_string(), "覆盖".to_string()]]);
    // 将首列标记为由动态 View 子树负责绘制。
    table.view_columns = vec![0];
    // 构造可保存布局片段元数据的真实组件树。
    let mut tree = WidgetTree::new();
    // 让唯一标签节点代表首行首列的有状态 View 子树。
    let child_id = tree.set_root(Box::new(crate::ui::widgets::Label::new("片段")));
    // 使用真实节点身份建立唯一物化 View 子节点的布局描述。
    let child = LayoutChild::new(child_id, Size::zero());
    // 在一百像素视口中执行自定义单元格布局。
    let positions = crate::ui::widget_runtime::traits::WidgetLayout::layout_children(
        // 使用待审计的表格实例。
        &table,
        // 表头之后保留足够容纳首行的表体高度。
        Rect::new(0.0, 0.0, 100.0, 100.0),
        // 只布局首行首列的唯一 View 子树。
        &[child],
        // 传入真实组件树以同步验证片段裁剪元数据。
        &tree,
    );
    // 唯一子项必须从视觉最左侧开始占满完整合并宽度。
    assert_eq!(
        // 读取首个 View 子树的最终布局矩形。
        positions.first().map(|(_, frame)| *frame),
        // 表体首行从三十三像素处开始并覆盖完整一百像素宽度。
        Some(Rect::new(0.0, 33.0, 100.0, 28.0))
    );
    // 同一 View 子树必须记录视觉左右区的两个不连续绘制片段。
    assert_eq!(
        // 从真实节点读取布局阶段保存的父级裁剪片段。
        parent_clip_regions_snapshot(&tree, child_id),
        // 左固定区先绘制，右固定锚点区随后绘制。
        Some(vec![
            // 左固定覆盖列贡献视觉左侧四十像素。
            Rect::new(0.0, 33.0, 40.0, 28.0),
            // 右固定锚点列贡献视觉右侧六十像素。
            Rect::new(40.0, 33.0, 60.0, 28.0),
        ])
    );
    // 结束跨固定区自定义 View 完整布局契约。
}

// 标记父级片段裁剪必须同时约束子树命中范围。
#[test]
// 验证命中可以进入任一片段，但不能穿过两个片段之间的空隙。
fn parent_clip_regions_reject_hit_in_fragment_gap() {
    // 构造承载分片子树的父级标签节点。
    let mut tree = WidgetTree::new();
    // 建立覆盖完整测试视口的根节点。
    let root_id = tree.set_root(Box::new(crate::ui::widgets::Label::new("父级")));
    // 在根节点下挂载唯一有状态子树。
    let child_id = tree.add_child(
        // 指定根节点为直接父级。
        root_id,
        // 使用标签提供可命中的普通组件节点。
        Box::new(crate::ui::widgets::Label::new("子树")),
    );
    // 将父节点布局到一百乘二十像素的测试视口。
    crate::ui::widget_runtime::widget::WidgetCore::set_frame(
        // 获取根节点的可变组件包装。
        tree.get_mut(root_id).expect("根节点必须存在"),
        // 父节点覆盖完整测试区域。
        Rect::new(0.0, 0.0, 100.0, 20.0),
    );
    // 将子树同样布局到完整逻辑跨度，保持单一状态实例。
    crate::ui::widget_runtime::widget::WidgetCore::set_frame(
        // 获取子节点的可变组件包装。
        tree.get_mut(child_id).expect("子节点必须存在"),
        // 子树逻辑 frame 横跨两个可见片段及其间隙。
        Rect::new(0.0, 0.0, 100.0, 20.0),
    );
    // 为子树声明左右两个互不相连的父级可见片段。
    tree.get(child_id)
        // 真实子节点必须可以保存布局元数据。
        .expect("子节点必须存在")
        // 片段集合刻意在中间留下六十像素空隙。
        .set_parent_clip_regions(Some(vec![
            // 左侧片段覆盖前二十像素。
            Rect::new(0.0, 0.0, 20.0, 20.0),
            // 右侧片段覆盖最后二十像素。
            Rect::new(80.0, 0.0, 20.0, 20.0),
        ]));
    // 左侧可见片段必须命中唯一子树。
    assert_eq!(
        // 使用屏幕坐标命中左侧可见区域。
        tree.hit_test(crate::core::Point::new(10.0, 10.0)),
        // 命中结果必须是唯一子树节点。
        Some(child_id)
    );
    // 中间片段空隙必须回落到父节点，不能误命中子树。
    assert_eq!(
        // 使用屏幕坐标命中两个片段之间的空隙。
        tree.hit_test(crate::core::Point::new(50.0, 10.0)),
        // 空隙只允许命中仍覆盖该位置的父节点。
        Some(root_id)
    );
    // 右侧可见片段同样必须命中同一个子树实例。
    assert_eq!(
        // 使用屏幕坐标命中右侧可见区域。
        tree.hit_test(crate::core::Point::new(90.0, 10.0)),
        // 左右片段必须共享同一子树身份。
        Some(child_id)
    );
    // 结束父级多片段命中契约。
}

// 标记固定列中的行合并必须保留唯一逻辑锚点。
#[test]
// 验证被覆盖行、跨度高度和跨度结束后的新单元格保持一致。
fn fixed_column_row_span_keeps_anchor_and_height() {
    // 构造一个固定在左侧且跨越两行的合并列。
    let merged_left = TableColumn::new("行合并锚点", 40.0)
        // 首行首列占用两行，其他位置保持普通单元格。
        .row_span(|_row, column| if column == 0 { 2 } else { 1 })
        // 将合并列固定在视口左侧。
        .fixed(Fixed::Left);
    // 构造一个不参与合并的中间列。
    let middle = TableColumn::new("中间列", 30.0);
    // 构造一个固定在右侧的普通列。
    let right = TableColumn::new("右侧列", 30.0).fixed(Fixed::Right);
    // 建立三行数据以观察合并跨度结束后的新锚点。
    let table = Table::new()
        // 保留左、中、右三种列区。
        .columns(vec![merged_left, middle, right])
        // 提供足够的行数据覆盖两行跨度与第三行锚点。
        .rows(vec![
            // 提供第一行的三列值。
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            // 提供第二行的三列值。
            vec!["d".to_string(), "e".to_string(), "f".to_string()],
            // 提供第三行的三列值。
            vec!["g".to_string(), "h".to_string(), "i".to_string()],
        ])
        // 使用明确的行高使跨度高度可直接核验。
        .row_height(20.0);
    // 首行首列必须声明为两行跨度。
    assert_eq!(table.row_span(0, 0), 2);
    // 跨度中的第二行必须回落到首行首列锚点。
    assert_eq!(table.cell_anchor(1, 0), Some((0, 0)));
    // 跨度结束后的第三行必须拥有自己的首列锚点。
    assert_eq!(table.cell_anchor(2, 0), Some((2, 0)));
    // 同一行的中间列不得被左侧行合并覆盖。
    assert_eq!(table.cell_anchor(1, 1), Some((1, 1)));
    // 行跨度高度必须等于两行自然高度之和。
    assert_eq!(table.span_height(0, 2), 40.0);
}

// 标记二维合并在完整列区与自定义 View 中必须共享同一布局契约。
#[test]
// 验证跨行跨列、片段裁剪、覆盖子树隐藏和行命中都回落到同一锚点。
fn cross_zone_row_and_col_span_keep_view_layout_and_hit_consistent() {
    // 构造同时跨两行和四列的左固定合并锚点。
    let merged_anchor = TableColumn::new("二维合并锚点", 40.0)
        // 让首列锚点覆盖当前行之后的一行。
        .row_span(|_row, column| if column == 0 { 2 } else { 1 })
        // 让首列锚点覆盖左、中、右全部逻辑列。
        .col_span(|_row, column| if column == 0 { 4 } else { 1 })
        // 将锚点固定在视觉左侧。
        .fixed(Fixed::Left);
    // 构造第一列中间滚动区覆盖列。
    let middle_first = TableColumn::new("中间覆盖一", 30.0);
    // 构造第二列中间滚动区覆盖列。
    let middle_second = TableColumn::new("中间覆盖二", 30.0);
    // 构造视觉右侧覆盖列以闭合全部固定区组合。
    let right_covered = TableColumn::new("右侧覆盖列", 30.0).fixed(Fixed::Right);
    // 建立三行数据，使第一行锚点、第二行覆盖和第三行新锚点同时存在。
    let mut table = Table::new()
        // 保留左固定、中间滚动和右固定三种物理列区。
        .columns(vec![
            merged_anchor,
            middle_first,
            middle_second,
            right_covered,
        ])
        // 提供首行、被覆盖行和跨度结束后的新行。
        .rows(vec![
            // 提供首行的四列数据。
            vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
            ],
            // 提供第二行的四列数据。
            vec![
                "e".to_string(),
                "f".to_string(),
                "g".to_string(),
                "h".to_string(),
            ],
            // 提供第三行的四列数据。
            vec![
                "i".to_string(),
                "j".to_string(),
                "k".to_string(),
                "l".to_string(),
            ],
        ])
        // 使用明确行高，使二维跨度高度可直接核验。
        .row_height(20.0);
    // 首行首列必须保留声明的两行跨度。
    assert_eq!(table.row_span(0, 0), 2);
    // 首行首列必须保留声明的四列跨度。
    assert_eq!(table.col_span(0, 0), 4);
    // 被覆盖行的中间列必须回落到首行首列锚点。
    assert_eq!(table.cell_anchor(1, 2), Some((0, 0)));
    // 被覆盖行的右固定列也必须回落到同一锚点。
    assert_eq!(table.cell_anchor(1, 3), Some((0, 0)));
    // 跨度结束后的第三行右列必须回落到第三行首列新锚点。
    assert_eq!(table.cell_anchor(2, 3), Some((2, 0)));
    // 使用一百像素宽的视口形成固定区重叠与完整跨度。
    let frame = Rect::new(0.0, 0.0, 100.0, 120.0);
    // 读取表头与分隔线之后的表体起点。
    let body_top = table.total_header_height() + 1.0;
    // 建立与绘制、布局和命中共享的列几何快照。
    let geometry = table.column_geometry(frame.x, frame.w);
    // 解析首行二维锚点的完整逻辑合并矩形。
    let first_frame = geometry
        // 以首列为锚点并覆盖全部四列。
        .span_bounds(0, 4, body_top, 40.0)
        // 当前列声明一定能生成完整跨度。
        .expect("二维首行跨度必须拥有物理范围");
    // 解析第三行跨度结束后的新逻辑单元格矩形。
    let third_frame = geometry
        // 以第三行首列为锚点并覆盖全部四列。
        .span_bounds(0, 4, body_top + 40.0, 20.0)
        // 当前列声明一定能生成完整跨度。
        .expect("二维末行跨度必须拥有物理范围");
    // 共享列几何必须把四列合并为一百像素完整宽度。
    assert_eq!(first_frame, Rect::new(0.0, body_top, 100.0, 40.0));
    // 第三行新锚点必须复用同一完整物理跨度。
    assert_eq!(third_frame, Rect::new(0.0, body_top + 40.0, 100.0, 20.0));
    // 将首列声明为唯一自定义 View 列，检查组件级子树布局。
    table.view_columns = vec![0];
    // 构造承载三行 View 子树的真实组件树。
    let mut tree = WidgetTree::new();
    // 建立首行二维合并锚点的 View 根节点。
    let first_id = tree.set_root(Box::new(crate::ui::widgets::Label::new("首行锚点")));
    // 建立第二行被覆盖 View 子树节点。
    let second_id = tree.add_child(
        // 将覆盖节点挂到同一父级树中。
        first_id,
        // 使用标签代表被合并覆盖的物化子树。
        Box::new(crate::ui::widgets::Label::new("被覆盖")),
    );
    // 建立第三行跨度结束后新锚点的 View 子树节点。
    let third_id = tree.add_child(
        // 将新锚点节点挂到同一父级树中。
        first_id,
        // 使用标签代表第三行新的逻辑单元格。
        Box::new(crate::ui::widgets::Label::new("新锚点")),
    );
    // 为物化的三行 View 子树提供稳定布局子项身份。
    let children = [
        // 首行子项对应二维合并锚点。
        LayoutChild::new(first_id, Size::zero()),
        // 第二行子项对应被覆盖物理位置。
        LayoutChild::new(second_id, Size::zero()),
        // 第三行子项对应跨度结束后的新锚点。
        LayoutChild::new(third_id, Size::zero()),
    ];
    // 执行真实表格组件的子树布局入口。
    let positions = crate::ui::widget_runtime::traits::WidgetLayout::layout_children(
        // 使用待审计的表格组件。
        &table,
        // 传入会同时影响列几何、可视行和命中范围的真实 frame。
        frame,     // 传入按行排列的三个 View 子树。
        &children, // 传入用于保存父级片段元数据的组件树。
        &tree,
    );
    // 首行 View 必须按完整二维跨度只布局一次。
    assert_eq!(positions[0], (first_id, first_frame));
    // 第二行被覆盖 View 必须收敛为零尺寸并阻止重复绘制。
    assert_eq!(
        positions[1],
        (second_id, Rect::new(0.0, body_top + 20.0, 0.0, 0.0))
    );
    // 第三行必须按跨度结束后的新锚点重新布局。
    assert_eq!(positions[2], (third_id, third_frame));
    // 首行 View 必须记录中间、左固定、右固定三个最终可见片段。
    assert_eq!(
        parent_clip_regions_snapshot(&tree, first_id),
        Some(vec![
            // 中间滚动区保留四十到七十像素片段。
            Rect::new(40.0, body_top, 30.0, 40.0),
            // 左固定区保留零到四十像素片段。
            Rect::new(0.0, body_top, 40.0, 40.0),
            // 右固定区保留七十到一百像素片段。
            Rect::new(70.0, body_top, 30.0, 40.0),
        ])
    );
    // 被覆盖 View 必须记录空片段集合以同时阻止绘制和命中。
    assert_eq!(
        parent_clip_regions_snapshot(&tree, second_id),
        Some(Vec::new())
    );
    // 第三行新锚点必须按相同列区顺序记录可见片段。
    assert_eq!(
        parent_clip_regions_snapshot(&tree, third_id),
        Some(vec![
            // 中间滚动区保留第三行的四十到七十像素片段。
            Rect::new(40.0, body_top + 40.0, 30.0, 20.0),
            // 左固定区保留第三行的零到四十像素片段。
            Rect::new(0.0, body_top + 40.0, 40.0, 20.0),
            // 右固定区保留第三行的七十到一百像素片段。
            Rect::new(70.0, body_top + 40.0, 30.0, 20.0),
        ])
    );
    // 被覆盖行的左固定区域点击必须回落到首行锚点。
    assert_eq!(
        table.action_at_point(crate::core::Point::new(10.0, body_top + 25.0)),
        Some(TablePointerAction::SelectRow(0))
    );
    // 被覆盖行的中间区域点击必须回落到同一首行锚点。
    assert_eq!(
        table.action_at_point(crate::core::Point::new(50.0, body_top + 25.0)),
        Some(TablePointerAction::SelectRow(0))
    );
    // 被覆盖行的右固定区域点击也必须回落到同一首行锚点。
    assert_eq!(
        table.action_at_point(crate::core::Point::new(85.0, body_top + 25.0)),
        Some(TablePointerAction::SelectRow(0))
    );
    // 跨度结束后的第三行点击不得继续回落到旧锚点。
    assert_eq!(
        table.action_at_point(crate::core::Point::new(85.0, body_top + 45.0)),
        Some(TablePointerAction::SelectRow(2))
    );
}
