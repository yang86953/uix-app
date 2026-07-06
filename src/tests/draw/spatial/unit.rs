use super::*;

    #[test]
    fn px_to_dip_is_identity() {
        let u = PhysicalUnit::Px(100.0);
        assert!((u.to_dip(96.0) - 100.0).abs() < 1e-10);
    }

    #[test]
    fn mm_to_dip() {
        // 25.4mm = 1 inch = 96 dip (at 96 DPI)
        let u = PhysicalUnit::Mm(25.4);
        assert!((u.to_dip(96.0) - 96.0).abs() < 1e-6);
    }

    #[test]
    fn cm_to_dip() {
        // 2.54cm = 1 inch = 96 dip
        let u = PhysicalUnit::Cm(2.54);
        assert!((u.to_dip(96.0) - 96.0).abs() < 1e-6);
    }

    #[test]
    fn m_to_dip() {
        // 0.0254m = 1 inch = 96 dip
        let u = PhysicalUnit::M(0.0254);
        assert!((u.to_dip(96.0) - 96.0).abs() < 1e-4);
    }

    #[test]
    fn pt_to_dip() {
        // 72pt = 1 inch = 96 dip
        let u = PhysicalUnit::Pt(72.0);
        assert!((u.to_dip(96.0) - 96.0).abs() < 1e-6);
    }

    #[test]
    fn inch_to_dip() {
        let u = PhysicalUnit::Inch(1.0);
        assert!((u.to_dip(96.0) - 96.0).abs() < 1e-6);
    }

    #[test]
    fn high_dpi_scaling() {
        // 在同 DPI 下 pt 和 dip 的关系
        // 12pt @ 96dpi = 16px
        let pt = PhysicalUnit::Pt(12.0);
        assert!((pt.to_dip(96.0) - 16.0).abs() < 1e-6);
        // 12pt @ 192dpi = 32px
        assert!((pt.to_dip(192.0) - 32.0).abs() < 1e-6);
    }

    #[test]
    fn physical_unit_ext_f32() {
        let v = 10.0f32;
        match v.mm() {
            PhysicalUnit::Mm(x) => assert!((x - 10.0).abs() < 1e-10),
            _ => panic!("expected Mm"),
        }
    }

    #[test]
    fn physical_unit_ext_i32() {
        let v = 5i32;
        match v.cm() {
            PhysicalUnit::Cm(x) => assert!((x - 5.0).abs() < 1e-10),
            _ => panic!("expected Cm"),
        }
    }

    #[test]
    fn angle_deg_to_rad() {
        let r = 90.0f32.deg();
        assert!((r - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    }

    #[test]
    fn angle_rad_is_identity() {
        let r = 0.5f32.rad();
        assert!((r - 0.5).abs() < 1e-10);
    }

    #[test]
    fn unit_name() {
        assert_eq!(PhysicalUnit::Px(1.0).unit_name(), "px");
        assert_eq!(PhysicalUnit::Mm(1.0).unit_name(), "mm");
        assert_eq!(PhysicalUnit::Cm(1.0).unit_name(), "cm");
        assert_eq!(PhysicalUnit::M(1.0).unit_name(), "m");
        assert_eq!(PhysicalUnit::Pt(1.0).unit_name(), "pt");
        assert_eq!(PhysicalUnit::Inch(1.0).unit_name(), "inch");
    }

    #[test]
    fn value() {
        assert!((PhysicalUnit::Cm(5.0).value() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn add_units() {
        let a = PhysicalUnit::Mm(10.0);
        let b = PhysicalUnit::Cm(1.0); // 10mm
        let c = a + b;
        match c {
            PhysicalUnit::Mm(v) => assert!((v - 20.0).abs() < 1e-4),
            _ => panic!("expected Mm"),
        }
    }

    #[test]
    fn to_px_with_dpr() {
        let u = PhysicalUnit::Px(100.0);
        assert!((u.to_px(96.0, 2.0) - 200.0).abs() < 1e-10);
    }
