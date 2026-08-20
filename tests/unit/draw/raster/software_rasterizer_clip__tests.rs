    // 引入路径构建器与共享渲染器的底层类型。
    use super::*;
    use crate::draw::geometry::path::PathBuilder;

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
