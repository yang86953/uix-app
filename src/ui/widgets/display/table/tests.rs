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
