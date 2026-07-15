use crate::tests::common::*;
use crate::ui::widgets::QRCode;

#[test]
fn qrcode_builds_standard_matrix_with_three_finder_patterns() {
    let code = QRCode::new("https://uix.dev").error_level(2);

    assert!(code.is_valid());
    assert!(code.encoding_error().is_none());
    assert!(code.module_count() >= 21);
    assert_eq!(code.module_count() % 4, 1);

    let width = code.module_count();
    for (origin_x, origin_y) in [(0, 0), (width - 7, 0), (0, width - 7)] {
        for y in 0..7 {
            for x in 0..7 {
                let expected = x == 0
                    || x == 6
                    || y == 0
                    || y == 6
                    || ((2..=4).contains(&x) && (2..=4).contains(&y));
                assert_eq!(code.module(origin_x + x, origin_y + y), Some(expected));
            }
        }
    }
}

#[test]
fn qrcode_reports_oversized_payload_without_panicking() {
    let code = QRCode::new(&"x".repeat(10_000)).error_level(3);

    assert!(!code.is_valid());
    assert_eq!(code.module_count(), 0);
    assert_eq!(code.encoding_error(), Some("data too long"));
}

#[test]
fn qrcode_normalizes_size_and_error_level() {
    let code = QRCode::new("uix").size(f32::NAN).error_level(u8::MAX);

    assert_eq!(
        code.measure(Constraints::unconstrained()),
        Size::new(160.0, 160.0)
    );
    assert!(matches!(
        code.snapshot_fields(),
        SnapshotFields::QRCode { error_level: 3, .. }
    ));
}
