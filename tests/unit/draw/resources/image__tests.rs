use super::*;

#[test]
fn decode_preserves_rgba_dimensions_and_premultiplied_alpha() {
    use image::ImageEncoder;

    // 同时覆盖不透明、半透明、全透明与极低 alpha 的确定性像素。
    let source = [
        255, 128, 64, 255, 240, 120, 60, 128, 255, 255, 255, 0, 255, 128, 64, 1,
    ];
    let mut encoded = Vec::new();
    image::codecs::png::PngEncoder::new(&mut encoded)
        .write_image(&source, 2, 2, image::ExtendedColorType::Rgba8)
        .expect("确定性 RGBA PNG 应可编码");

    let (width, height, pixels) = decode_to_pixels(&encoded).expect("RGBA PNG 应可解码");
    assert_eq!((width, height), (2, 2));
    assert_eq!(
        pixels,
        vec![0xFFFF_8040, 0x8078_3C1E, 0x0000_0000, 0x0101_0000]
    );
}

#[test]
fn decode_rejects_invalid_encoded_bytes_without_publishing_pixels() {
    let error = decode_to_pixels(b"not-an-image").expect_err("无效格式必须返回 typed error");
    assert_eq!(error.code(), crate::core::Errc::InvalidArgument);
}

#[test]
fn cached_path_returns_existing_valid_handle() {
    let service = ImageService::new();
    let path = Path::new("uix-image-cache-hit-does-not-read.png");
    let key = path.to_string_lossy().into_owned();
    let handle = service.insert_slot(ImageSlot::from_decoded(
        1,
        1,
        vec![0xff12_3456],
        Some(key.clone()),
    ));
    service.path_cache.borrow_mut().insert(key, handle);

    assert_eq!(
        service
            .load_from_path(path)
            .expect("valid cached path should not touch the filesystem"),
        handle
    );
    assert_eq!(
        service
            .poll_load_from_path(path)
            .expect("valid cached poll should not start background I/O"),
        Some(handle)
    );
}
