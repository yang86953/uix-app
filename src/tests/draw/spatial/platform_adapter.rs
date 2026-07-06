use super::*;

#[test]
fn ydown_no_conversion() {
    let o = Orientation::YDown;
    let (x, y) = o.screen_to_native(600.0, 100.0, 200.0);
    assert!((x - 100.0).abs() < 1e-10);
    assert!((y - 200.0).abs() < 1e-10);
}

#[test]
fn yup_flips_y() {
    let o = Orientation::YUp;
    let (x, y) = o.screen_to_native(600.0, 100.0, 200.0);
    assert!((x - 100.0).abs() < 1e-10);
    assert!((y - 400.0).abs() < 1e-10); // 600 - 200 = 400
}

#[test]
fn roundtrip() {
    let o = Orientation::YUp;
    let (nx, ny) = o.screen_to_native(800.0, 150.0, 300.0);
    let (sx, sy) = o.native_to_screen(800.0, nx, ny);
    assert!((sx - 150.0).abs() < 1e-10);
    assert!((sy - 300.0).abs() < 1e-10);
}
