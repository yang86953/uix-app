//! Table 组件测试 — table 子模块。

use super::*;

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
                TableColumn::new("ID", 80.0).bind(|r: &UserRow| r.id.to_string())
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
