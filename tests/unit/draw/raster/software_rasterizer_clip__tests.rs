    // 引入路径构建器与共享渲染器的底层类型。
    use super::*;
    use crate::draw::geometry::path::PathBuilder;
    use crate::draw::raster::software_rasterizer::MAX_RETAINED_TRANSIENT_STACK_BYTES;

    // 三角形内部应有 coverage，外部应保持为零。
    #[test]
    fn path_clip_builds_coverage_mask() {
        // 创建足够小的目标，覆盖结果可以直接检查。
        let mut rasterizer = SoftwareRasterizer::new(16, 16);
        // 构造一个覆盖左上区域的闭合三角形。
        let mut builder = PathBuilder::new();
        builder
            .move_to(1.0, 1.0)
            .line_to(12.0, 1.0)
            .line_to(1.0, 12.0)
            .close();
        // 路径裁剪应成功建立 mask。
        if let Err(error) = rasterizer.try_push_clip_path(&builder.build()) {
            // 合法路径必须可裁剪。
            panic!("triangle path clip must be supported: {error:?}");
        }
        // 三角形内部的像素应可见。
        assert!(rasterizer.clip_mask_value(2, 2) > 0);
        // 三角形外部的像素应被完全裁掉。
        assert_eq!(rasterizer.clip_mask_value(12, 12), 0);
        // pop_clip 应恢复无路径 mask 的默认 coverage。
        rasterizer.pop_clip();
        assert_eq!(rasterizer.clip_mask_value(12, 12), u8::MAX);
    }

    // 一次性固定裁剪不得为永远不会 pop 的栈申请内存。
    #[test]
    fn fixed_surface_clip_starts_without_stack_allocation() {
        let rasterizer = SoftwareRasterizer::new_with_surface_clip(
            32,
            24,
            Rect::new(2.0, 3.0, 12.0, 10.0),
        );

        assert_eq!(rasterizer.clip_rect, Rect::new(2.0, 3.0, 12.0, 10.0));
        assert_eq!(rasterizer.transient_stack_capacities(), (0, 0, 0));
    }

    // 帧重置应清空状态，但保留已经预热的栈分配。
    #[test]
    fn reset_for_extent_reuses_transient_stack_capacity() {
        let mut rasterizer = SoftwareRasterizer::new(16, 16);
        rasterizer.push_clip(Rect::new(1.0, 2.0, 8.0, 7.0));
        rasterizer.save();
        let capacities = rasterizer.transient_stack_capacities();

        rasterizer.reset_for_extent(32, 24);

        assert_eq!(rasterizer.clip_rect, Rect::new(0.0, 0.0, 32.0, 24.0));
        assert!(rasterizer.clip_stack.is_empty());
        assert!(rasterizer.clip_mask.is_none());
        assert!(rasterizer.clip_mask_stack.is_empty());
        assert_eq!(rasterizer.transient_stack_capacities(), capacities);
        rasterizer.restore();
        assert_eq!(rasterizer.clip_rect, Rect::new(0.0, 0.0, 32.0, 24.0));
    }

    // 复用槽必须保留内部分配，同时恢复任意失衡的裁剪操作。
    #[test]
    fn save_restore_reuses_snapshot_slot_and_exact_clip_state() {
        let mut rasterizer = SoftwareRasterizer::new(32, 24);
        let saved_clip = Rect::new(2.0, 3.0, 12.0, 10.0);
        rasterizer.push_clip(saved_clip);
        rasterizer.save();
        rasterizer.pop_clip();
        rasterizer.push_clip(Rect::new(20.0, 10.0, 4.0, 4.0));
        rasterizer.restore();
        let reusable_allocation = rasterizer
            .snapshot_clip_stack_allocation(0)
            .expect("restore 后应保留可复用槽容量");
        assert_eq!(rasterizer.clip_rect, saved_clip);
        rasterizer.pop_clip();
        assert_eq!(rasterizer.clip_rect, Rect::new(0.0, 0.0, 32.0, 24.0));

        rasterizer.push_clip(saved_clip);
        rasterizer.save();
        assert_eq!(
            rasterizer
                .snapshot_clip_stack_allocation(0)
                .expect("第二次 save 应复用同一槽"),
            reusable_allocation
        );
        rasterizer.restore();
    }

    // 路径 mask 在失衡 pop 后也必须由 save/restore 精确恢复。
    #[test]
    fn save_restore_preserves_path_clip_mask() {
        let mut rasterizer = SoftwareRasterizer::new(16, 16);
        let mut builder = PathBuilder::new();
        builder
            .move_to(1.0, 1.0)
            .line_to(12.0, 1.0)
            .line_to(1.0, 12.0)
            .close();
        rasterizer
            .try_push_clip_path(&builder.build())
            .expect("三角形路径应建立裁剪 mask");
        let inside = rasterizer.clip_mask_value(2, 2);
        let outside = rasterizer.clip_mask_value(12, 12);

        rasterizer.save();
        rasterizer.pop_clip();
        assert_eq!(rasterizer.clip_mask_value(12, 12), u8::MAX);
        rasterizer.restore();

        assert_eq!(rasterizer.clip_mask_value(2, 2), inside);
        assert_eq!(rasterizer.clip_mask_value(12, 12), outside);
    }

    // 异常 save 深度超过总预算后必须释放整个快照池。
    #[test]
    fn save_restore_releases_oversized_snapshot_pool() {
        let mut rasterizer = SoftwareRasterizer::new(16, 16);
        for _ in 0..1024 {
            rasterizer.save();
        }
        for _ in 0..1024 {
            rasterizer.restore();
        }

        assert_eq!(rasterizer.snapshot_slot_count(), 0);
    }

    // 异常深度产生的大栈不得永久保留在光栅化器中。
    #[test]
    fn reset_for_extent_releases_oversized_transient_stack() {
        let mut rasterizer = SoftwareRasterizer::new(16, 16);
        let excessive_depth =
            MAX_RETAINED_TRANSIENT_STACK_BYTES / std::mem::size_of::<Rect>() + 1;
        rasterizer.clip_stack.reserve_exact(excessive_depth);
        assert!(
            rasterizer.clip_stack.capacity() * std::mem::size_of::<Rect>()
                > MAX_RETAINED_TRANSIENT_STACK_BYTES
        );

        rasterizer.reset_for_extent(16, 16);

        assert_eq!(rasterizer.clip_stack.capacity(), 0);
    }
