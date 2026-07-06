use super::*;
    use crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::spatial::vec3::Vec3;
    use crate::draw::spatial::Orientation;

    #[test]
    fn from_rect_z_builds_aabb() {
        let region = DirtyRegion3D::from_rect_z(Rect::new(10.0, 20.0, 100.0, 50.0), -1.0, 2.0);
        // z 范围 [-1, -1+2] = [-1, 1]
        assert!((region.aabb.min.z - (-1.0)).abs() < 1e-6);
        assert!((region.aabb.max.z - 1.0).abs() < 1e-6);
        assert!((region.aabb.min.x - 10.0).abs() < 1e-6);
        assert!((region.aabb.max.x - 110.0).abs() < 1e-6);
    }

    #[test]
    fn project_2d_maps_directly_to_screen() {
        let mut canvas = NoopCanvas2D;
        let ctx = SpatialContext::new(&mut canvas, 96.0, 1.0, Orientation::YDown, 800, 600);
        // 2D 正交投影下，模型 (100,100)-(300,200) 直接映射到屏幕同坐标
        let region = DirtyRegion3D::from_rect_z(Rect::new(100.0, 100.0, 200.0, 100.0), 0.0, 0.0);
        let r = region.project(&ctx);
        assert!((r.x - 100.0).abs() < 1.0, "x: {r:?}");
        assert!((r.y - 100.0).abs() < 1.0, "y: {r:?}");
        assert!((r.w - 200.0).abs() < 1.0, "w: {r:?}");
        assert!((r.h - 100.0).abs() < 1.0, "h: {r:?}");
    }

    #[test]
    fn project_scales_by_device_pixel_ratio() {
        let mut canvas = NoopCanvas2D;
        // 2x DPR：屏幕坐标应翻倍
        let ctx = SpatialContext::new(&mut canvas, 96.0, 2.0, Orientation::YDown, 800, 600);
        let region = DirtyRegion3D::from_rect_z(Rect::new(50.0, 50.0, 100.0, 100.0), 0.0, 0.0);
        let r = region.project(&ctx);
        assert!((r.x - 100.0).abs() < 2.0, "x: {r:?}");
        assert!((r.w - 200.0).abs() < 2.0, "w: {r:?}");
    }
