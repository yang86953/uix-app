use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::widgets::Avatar;
use crate::ui::AccessibilityRole;

fn avatar_test_image() -> (std::path::PathBuf, String) {
    let image = image::RgbaImage::from_fn(6, 4, |x, _| {
        if x < 3 {
            image::Rgba([255, 0, 0, 255])
        } else {
            image::Rgba([0, 0, 255, 255])
        }
    });
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("encode avatar fixture");
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("uix-avatar-{}-{nonce}.png", std::process::id()));
    std::fs::write(&path, bytes.into_inner()).expect("write avatar fixture");
    let source = path.to_string_lossy().into_owned();
    (path, source)
}

fn render_avatar(avatar: &Avatar, images: &ImageService) -> Vec<u32> {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(32, 32));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        font,
        &fonts,
        images,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        32,
        32,
    );
    WidgetRender::render(avatar, Rect::new(0.0, 0.0, 32.0, 32.0), &mut ctx, &tree);
    canvas.surface().pixels().to_vec()
}

#[test]
fn avatar_src_loads_and_masks_circular_corners() {
    let (path, source) = avatar_test_image();
    let images = ImageService::new();
    let avatar = Avatar::new("AB").src(&source);

    let pixels = render_avatar(&avatar, &images);
    assert!(avatar.loaded_handle_for_test().is_some());
    assert_eq!(pixels[0], 0, "circular corner should remain transparent");
    assert_ne!(
        pixels[16 * 32 + 16],
        0,
        "avatar center should contain image"
    );

    std::fs::remove_file(path).expect("remove avatar fixture");
}

#[test]
fn avatar_normalizes_size_and_exposes_image_semantics() {
    let avatar = Avatar::new("Ada").size(f32::NAN).src("avatar.png");
    assert_eq!(
        avatar.measure(Constraints::unconstrained()),
        Size::new(32.0, 32.0)
    );
    let accessibility = avatar.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Image);
    assert_eq!(accessibility.name.as_deref(), Some("Ada"));
}
