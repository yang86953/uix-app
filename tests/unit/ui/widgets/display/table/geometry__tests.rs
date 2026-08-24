    // 复用列布局类型与父模块导入的列声明。
    use super::*;

    // 标记窄视口固定列绘制层级契约。
    #[test]
    // 验证重叠区命中最后绘制的右固定列。
    fn overlapping_fixed_columns_hit_topmost_painted_zone() {
        // 构造宽于视口的左固定列。
        let left = TableColumn::new("左列", 80.0).fixed(Fixed::Left);
        // 构造同样宽于剩余区域的右固定列。
        let right = TableColumn::new("右列", 80.0).fixed(Fixed::Right);
        // 在一百像素视口中形成二十到八十像素的重叠区。
        let geometry = TableColumnGeometry::new(&[left, right], 0.0, 100.0, 0.0, 0.0);
        // 左侧非重叠区仍由左固定列命中。
        assert_eq!(geometry.column_at(10.0), Some(0));
        // 重叠区必须命中绘制顺序中位于最上层的右固定列。
        assert_eq!(geometry.column_at(50.0), Some(1));
        // 右侧非重叠区继续由右固定列命中。
        assert_eq!(geometry.column_at(90.0), Some(1));
        // 结束固定列重叠命中契约。
    }

    // 标记行合并锚点末尾重绘不得覆盖更高固定区的契约。
    #[test]
    // 验证左固定合并重绘只保留未被右固定区覆盖的可见片段。
    fn merged_repaint_clip_preserves_topmost_fixed_zone() {
        // 构造宽于视口剩余区域的左固定列。
        let left = TableColumn::new("左合并列", 80.0).fixed(Fixed::Left);
        // 构造同样宽且视觉层级更高的右固定列。
        let right = TableColumn::new("右普通列", 80.0).fixed(Fixed::Right);
        // 在一百像素视口中形成二十到八十像素的固定区重叠。
        let geometry = TableColumnGeometry::new(&[left, right], 0.0, 100.0, 0.0, 0.0);
        // 初始按层绘制时左区仍可使用完整八十像素裁剪并等待右区覆盖。
        assert_eq!(
            // 查询普通绘制使用的左区裁剪。
            geometry.clip_for(ColumnZone::Left, 32.0, 64.0),
            // 左区从零到八十像素完整参与底层绘制。
            Some(Rect::new(0.0, 32.0, 80.0, 64.0))
        );
        // 末尾重绘发生在右区之后，只能保留右区起点之前的二十像素。
        assert_eq!(
            // 查询行合并锚点使用的最终可见裁剪。
            geometry.merged_repaint_clip_for(ColumnZone::Left, 32.0, 64.0),
            // 左区最终仅有零到二十像素未被右固定区覆盖。
            Some(Rect::new(0.0, 32.0, 20.0, 64.0))
        );
        // 右固定合并锚点仍可使用其完整八十像素顶层裁剪。
        assert_eq!(
            // 查询最高层右区的合并重绘裁剪。
            geometry.merged_repaint_clip_for(ColumnZone::Right, 32.0, 64.0),
            // 右区从二十到一百像素全部可见。
            Some(Rect::new(20.0, 32.0, 80.0, 64.0))
        );
        // 重叠点命中继续返回最终可见的右固定列。
        assert_eq!(geometry.column_at(50.0), Some(1));
        // 结束行合并锚点重绘层级契约。
    }

    // 标记跨固定区列合并必须覆盖全部逻辑列的契约。
    #[test]
    // 验证右固定锚点能够向左覆盖后声明的左固定列。
    fn cross_zone_span_bounds_cover_reordered_fixed_columns() {
        // 先声明位于视觉右侧的合并锚点列。
        let right_anchor = TableColumn::new("右侧锚点", 60.0).fixed(Fixed::Right);
        // 后声明位于视觉左侧的被覆盖列。
        let left_covered = TableColumn::new("左侧覆盖列", 40.0).fixed(Fixed::Left);
        // 在一百像素视口中让两列分别占据左右连续区域。
        let geometry =
            // 保留与公开列声明相同的右前左后逻辑顺序。
            TableColumnGeometry::new(&[right_anchor, left_covered], 0.0, 100.0, 0.0, 0.0);
        // 跨两列的单一合并单元格必须覆盖零到一百像素的视觉联合范围。
        assert_eq!(
            // 查询从右固定锚点开始的两列合并矩形。
            geometry.span_bounds(0, 2, 32.0, 28.0),
            // 期望矩形同时包含视觉左侧与右侧两列。
            Some(Rect::new(0.0, 32.0, 100.0, 28.0))
        );
        // 末尾重绘必须为跨度中的左固定片段保留零到四十像素。
        assert_eq!(
            // 查询逻辑跨度在左固定区中的最终可见片段。
            geometry.merged_span_repaint_clip_for(
                // 传入右固定锚点索引。
                0,
                // 传入覆盖左右两列的跨度。
                2,
                // 查询视觉左侧固定区。
                ColumnZone::Left,
                // 保留测试纵坐标。
                32.0,
                // 保留测试高度。
                28.0,
            ),
            // 左固定覆盖列完整占据零到四十像素。
            Some(Rect::new(0.0, 32.0, 40.0, 28.0))
        );
        // 末尾重绘必须为同一跨度保留四十到一百像素的右固定片段。
        assert_eq!(
            // 查询逻辑跨度在右固定区中的最终可见片段。
            geometry.merged_span_repaint_clip_for(
                // 传入右固定锚点索引。
                0,
                // 传入覆盖左右两列的跨度。
                2,
                // 查询视觉右侧固定区。
                ColumnZone::Right,
                // 保留测试纵坐标。
                32.0,
                // 保留测试高度。
                28.0,
            ),
            // 右固定锚点完整占据四十到一百像素。
            Some(Rect::new(40.0, 32.0, 60.0, 28.0))
        );
        // 结束跨固定区列合并矩形契约。
    }

    // 验证列几何缓存复用数组，并在列宽或固定区原位变化时重新求解。
    #[test]
    fn geometry_cache_reuses_storage_and_detects_column_changes() {
        let mut columns = vec![
            TableColumn::new("左列", 40.0).fixed(Fixed::Left),
            TableColumn::new("中列", 60.0),
        ];
        let mut cache = TableColumnGeometryCache::default();

        let initial_ptr = {
            let geometry = cache.resolve(&columns, 0.0, 100.0, 0.0, 0.0);
            assert_eq!(geometry.columns[0].width, 40.0);
            geometry.columns.as_ptr()
        };
        let stable_ptr = cache
            .resolve(&columns, 0.0, 100.0, 0.0, 0.0)
            .columns
            .as_ptr();
        assert_eq!(stable_ptr, initial_ptr);

        columns[0].width = 50.0;
        columns[0].fixed = Some(Fixed::Right);
        let changed = cache.resolve(&columns, 0.0, 100.0, 0.0, 0.0);
        assert_eq!(changed.columns[0].width, 50.0);
        assert_eq!(changed.columns[0].zone, ColumnZone::Right);
        assert_eq!(changed.columns.as_ptr(), initial_ptr);
    }
    // 结束表格列几何测试模块。
