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

#[test]
fn unloaded_slots_increment_generation_across_multiple_reuses() {
    let service = ImageService::new();
    let first = service.load_from_bytes(RED_PNG).expect("first load");
    service.unload(first);
    let second = service.load_from_bytes(RED_PNG).expect("second load");
    service.unload(second);
    let third = service.load_from_bytes(RED_PNG).expect("third load");

    assert_ne!(first, second);
    assert_ne!(second, third);
    assert_ne!(first, third);
    assert!(!service.is_valid(first));
    assert!(!service.is_valid(second));
    assert!(service.is_valid(third));
}

#[test]
fn target_sized_avatar_masks_cover_low_resolution_sources_and_cache_by_shape() {
    let service = ImageService::new();
    let original = service
        .load_from_bytes(RED_PNG)
        .expect("decode source image");

    let circular = service
        .circular_crop_sized(original, 32)
        .expect("target-sized circular crop");
    assert_eq!(service.circular_crop_sized(original, 32), Some(circular));
    let rounded = service
        .rounded_square_crop_sized(original, 32, 4.0)
        .expect("target-sized rounded crop");
    assert_eq!(
        service.rounded_square_crop_sized(original, 32, 4.0),
        Some(rounded)
    );
    let smaller_circle = service
        .circular_crop_sized(original, 16)
        .expect("second circular size");
    let rounder_square = service
        .rounded_square_crop_sized(original, 32, 8.0)
        .expect("second corner radius");
    assert_ne!(smaller_circle, circular);
    assert_ne!(rounder_square, rounded);

    for derived in [circular, rounded] {
        service
            .with_slot(derived, |slot| {
                assert_eq!((slot.width(), slot.height()), (32, 32));
                assert_eq!(slot.pixels()[0], 0);
                assert_eq!(slot.pixels()[16 * 32 + 16], Color::red().premultiplied());
            })
            .expect("derived avatar slot");
    }
    service
        .with_slot(circular, |slot| {
            assert_eq!(slot.pixels()[3 * 32 + 3], 0);
            assert_ne!(
                slot.pixels()[16],
                0,
                "anti-aliased circle edge should remain visible"
            );
        })
        .expect("circular slot");

    service.unload(original);
    assert!(!service.is_valid(circular));
    assert!(!service.is_valid(rounded));
    assert!(!service.is_valid(smaller_circle));
    assert!(!service.is_valid(rounder_square));
}

#[test]
fn circular_crop_centers_masks_caches_and_unloads_with_source() {
    let source = image::RgbaImage::from_fn(6, 4, |x, _| {
        if x < 3 {
            image::Rgba([255, 0, 0, 255])
        } else {
            image::Rgba([0, 0, 255, 255])
        }
    });
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(source)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("encode test image");
    let service = ImageService::new();
    let original = service
        .load_from_bytes(bytes.get_ref())
        .expect("decode source image");

    let cropped = service.circular_crop(original).expect("circular crop");
    assert_eq!(service.circular_crop(original), Some(cropped));
    let square = service.square_crop(original).expect("square crop");
    assert_eq!(service.square_crop(original), Some(square));
    service
        .with_slot(cropped, |slot| {
            assert_eq!((slot.width(), slot.height()), (4, 4));
            assert_ne!(slot.pixels()[0], Color::red().premultiplied());
            assert_ne!(slot.pixels()[3], Color::blue().premultiplied());
            assert_ne!(slot.pixels()[5], 0);
            assert_ne!(slot.pixels()[6], 0);
        })
        .expect("cropped slot");
    service
        .with_slot(square, |slot| {
            assert_eq!((slot.width(), slot.height()), (4, 4));
            assert_eq!(slot.pixels()[0], Color::red().premultiplied());
            assert_eq!(slot.pixels()[3], Color::blue().premultiplied());
        })
        .expect("square slot");

    service.unload(original);
    assert!(!service.is_valid(original));
    assert!(!service.is_valid(cropped));
    assert!(!service.is_valid(square));
}
