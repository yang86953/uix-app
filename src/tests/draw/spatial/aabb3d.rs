use super::*;

    #[test]
    fn from_center_size() {
        let aabb = AABB3D::from_center(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 2.0, 2.0));
        assert!((aabb.min.x + 1.0).abs() < 1e-10);
        assert!((aabb.max.x - 1.0).abs() < 1e-10);
    }

    #[test]
    fn corners_count() {
        let aabb = AABB3D::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
        let corners = aabb.corners();
        assert_eq!(corners.len(), 8);
        // 验证所有顶点都在范围内
        for c in &corners {
            assert!(aabb.contains(*c));
        }
    }

    #[test]
    fn contains_point() {
        let aabb = AABB3D::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        assert!(aabb.contains(Vec3::new(5.0, 5.0, 5.0)));
        assert!(!aabb.contains(Vec3::new(15.0, 5.0, 5.0)));
        assert!(!aabb.contains(Vec3::new(5.0, -1.0, 5.0)));
    }

    #[test]
    fn intersects_overlap() {
        let a = AABB3D::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(5.0, 5.0, 5.0));
        let b = AABB3D::new(Vec3::new(3.0, 3.0, 3.0), Vec3::new(8.0, 8.0, 8.0));
        assert!(a.intersects(&b));
    }

    #[test]
    fn intersects_no_overlap() {
        let a = AABB3D::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 2.0, 2.0));
        let b = AABB3D::new(Vec3::new(5.0, 5.0, 5.0), Vec3::new(8.0, 8.0, 8.0));
        assert!(!a.intersects(&b));
    }

    #[test]
    fn union_expands() {
        let a = AABB3D::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(3.0, 3.0, 3.0));
        let b = AABB3D::new(Vec3::new(5.0, 5.0, 5.0), Vec3::new(8.0, 8.0, 8.0));
        let u = a.union(&b);
        assert!((u.min.x - 0.0).abs() < 1e-10);
        assert!((u.max.x - 8.0).abs() < 1e-10);
    }

    #[test]
    fn is_empty_positive() {
        let aabb = AABB3D::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0));
        assert!(aabb.is_empty());
    }

    #[test]
    fn from_rect_z() {
        let aabb = AABB3D::from_rect_z(10.0, 20.0, 100.0, 50.0, -5.0, 10.0);
        assert!((aabb.min.x - 10.0).abs() < 1e-10);
        assert!((aabb.min.y - 20.0).abs() < 1e-10);
        assert!((aabb.min.z - (-5.0)).abs() < 1e-10);
        assert!((aabb.max.x - 110.0).abs() < 1e-10);
        assert!((aabb.max.y - 70.0).abs() < 1e-10);
        assert!((aabb.max.z - 5.0).abs() < 1e-10);
    }
