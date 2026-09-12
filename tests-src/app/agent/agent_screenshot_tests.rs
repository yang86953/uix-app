//! `app/agent/agent_screenshot.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

#[test]
fn base64_encode_matches_known_vectors() {
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"f"), "Zg==");
    assert_eq!(base64_encode(b"fo"), "Zm8=");
    assert_eq!(base64_encode(b"foo"), "Zm9v");
    assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
    assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
    assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
}

#[test]
fn encode_png_bounded_rejects_invalid_readback() {
    // 合法载荷应能编码，随后破坏尺寸与载荷一致性必须被拒绝。
    let mut readback = SurfaceReadback::from_argb(2, 2, vec![0xFF000000; 4]);
    assert!(encode_png_bounded(&readback).is_ok());
    readback.width = 0;
    assert_eq!(
        encode_png_bounded(&readback),
        Err(ScreenshotEncodeError::InvalidReadback)
    );
    readback.width = 2;
    readback.pixels.truncate(3);
    assert_eq!(
        encode_png_bounded(&readback),
        Err(ScreenshotEncodeError::InvalidReadback)
    );
}

#[test]
fn encode_png_bounded_produces_decodable_image() {
    // 2x2 红/绿/蓝/白像素：验证 PNG 头与解码回路。
    let pixels = vec![0xFFFF0000, 0xFF00FF00, 0xFF0000FF, 0xFFFFFFFF];
    let readback = SurfaceReadback::from_argb(2, 2, pixels);
    let png = encode_png_bounded(&readback).expect("valid readback must encode");
    assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n");
    let decoder = png::Decoder::new(std::io::Cursor::new(&png[..]));
    let mut reader = decoder.read_info().expect("encoded png must be decodable");
    assert_eq!(reader.info().width, 2);
    assert_eq!(reader.info().height, 2);
    assert_eq!(reader.info().color_type, png::ColorType::Rgb);
    let buffer_size = reader
        .output_buffer_size()
        .expect("in-memory frame has a known buffer size");
    let mut decoded = vec![0u8; buffer_size];
    reader.next_frame(&mut decoded).expect("frame must decode");
    // 红、绿、蓝、白按行展开的 RGB 字节。
    assert_eq!(
        &decoded[..12],
        &[0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF]
    );
}
