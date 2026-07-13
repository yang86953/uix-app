use crate::draw::image::*;
use crate::tests::common::*;

const RED_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 0,
    5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

#[test]
fn fit_dst_rect_landscape_in_portrait_box() {
    let dst = fit_dst_rect(200, 100, Rect::new(0.0, 0.0, 100.0, 100.0));
    assert!((dst.w - 100.0).abs() < 0.01);
    assert!((dst.h - 50.0).abs() < 0.01);
    assert!((dst.y - 25.0).abs() < 0.01);
}

#[test]
fn path_cache_returns_same_handle() {
    let svc = ImageService::new();
    let h1 = svc.load_from_bytes(RED_PNG).expect("load");
    // 模拟路径缓存：先写入临时逻辑
    let mut cache = svc.path_cache.borrow_mut();
    cache.insert("test.png".into(), h1);
    drop(cache);
    let h2 = svc.ensure_loaded("test.png").expect("cached");
    assert_eq!(h1, h2);
}
