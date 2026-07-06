use super::*;
    use crate::draw::spatial::PhysicalUnit;

    #[test]
    fn resolve_font_size_no_unit_returns_base() {
        assert_eq!(resolve_font_size(14.0, None, 96.0), 14.0);
    }

    #[test]
    fn resolve_font_size_zero_base() {
        assert_eq!(resolve_font_size(0.0, None, 96.0), 0.0);
    }

    #[test]
    fn resolve_font_size_with_dip_unit_returns_base() {
        let unit = PhysicalUnit::Px(16.0);
        let result = resolve_font_size(14.0, Some(unit), 96.0);
        // Dip 直接返回其值
        assert_eq!(result, 16.0);
    }

    #[test]
    fn resolve_font_size_with_mm_unit() {
        let unit = PhysicalUnit::Mm(10.0);
        // 10mm @ 96 DPI ≈ 37.795
        let result = resolve_font_size(14.0, Some(unit), 96.0);
        assert!((result - 37.795).abs() < 0.01);
    }

    #[test]
    fn resolve_font_size_with_pt_unit() {
        let unit = PhysicalUnit::Pt(12.0);
        // 12pt @ 96 DPI = 12 * 96/72 = 16
        let result = resolve_font_size(14.0, Some(unit), 96.0);
        assert!((result - 16.0).abs() < 0.01);
    }

    #[test]
    fn resolve_font_size_with_pt_unit_high_dpi() {
        let unit = PhysicalUnit::Pt(12.0);
        let result = resolve_font_size(14.0, Some(unit), 192.0);
        // 12pt @ 192 DPI = 12 * 192/72 = 32
        assert!((result - 32.0).abs() < 0.01);
    }

    #[test]
    fn resolve_font_size_with_px_unit() {
        let unit = PhysicalUnit::Px(20.0);
        let result = resolve_font_size(14.0, Some(unit), 96.0);
        // px 直接返回其值
        assert_eq!(result, 20.0);
    }
