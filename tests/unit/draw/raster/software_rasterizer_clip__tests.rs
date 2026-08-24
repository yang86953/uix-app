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
