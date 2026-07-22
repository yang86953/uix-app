use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::widgets::Avatar;
use crate::ui::AccessibilityRole;

fn avatar_test_image() -> (std::path::PathBuf, String) {
    static NEXT_FIXTURE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let image = image::RgbaImage::from_fn(8, 4, |x, _| {
        if !(2..6).contains(&x) {
            image::Rgba([255, 255, 0, 255])
        } else if x < 4 {
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
    let sequence = NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "uix-avatar-{}-{nonce}-{sequence}.png",
        std::process::id()
    ));
    std::fs::write(&path, bytes.into_inner()).expect("write avatar fixture");
    let source = path.to_string_lossy().into_owned();
    (path, source)
}

fn solid_avatar_test_image() -> (std::path::PathBuf, String) {
    static NEXT_FIXTURE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("encode solid avatar fixture");
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    let sequence = NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "uix-avatar-solid-{}-{nonce}-{sequence}.png",
        std::process::id()
    ));
    std::fs::write(&path, bytes.into_inner()).expect("write solid avatar fixture");
    let source = path.to_string_lossy().into_owned();
    (path, source)
}

fn render_avatar_in(
    avatar: &Avatar,
    images: &ImageService,
    frame: Rect,
    surface_size: (i32, i32),
) -> (Vec<u32>, String) {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(surface_size.0, surface_size.1));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut display_list = crate::draw::painting::DisplayList::new();
    {
        let mut ctx = PaintContext::new_for_test(
            &mut canvas,
            font,
            &fonts,
            images,
            &tokens,
            96.0,
            1.0,
            Orientation::YDown,
            surface_size.0,
            surface_size.1,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(avatar, frame, ctx, &tree);
        });
    }
    (
        canvas.surface().pixels().to_vec(),
        format!("{display_list:?}"),
    )
}

fn render_avatar(avatar: &Avatar, images: &ImageService) -> Vec<u32> {
    render_avatar_in(avatar, images, Rect::new(0.0, 0.0, 32.0, 32.0), (32, 32)).0
}

fn recorded_text_font_size(display_list: &str) -> f32 {
    let marker = "font_size: ";
    let start = display_list
        .rfind(marker)
        .expect("avatar fallback should record text")
        + marker.len();
    let end = display_list[start..]
        .find([' ', '}', ']'])
        .map_or(display_list.len(), |offset| start + offset);
    display_list[start..end]
        .trim_end_matches(',')
        .parse()
        .expect("recorded Avatar font size")
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
fn circular_avatar_masks_one_pixel_sources_at_the_target_resolution() {
    let (path, source) = solid_avatar_test_image();
    let images = ImageService::new();
    let avatar = Avatar::new("").src(&source);

    let pixels = render_avatar(&avatar, &images);
    assert_eq!(pixels[0], 0);
    assert_eq!(pixels[3 * 32 + 3], 0);
    assert_ne!(
        pixels[16], 0,
        "circle edge should be anti-aliased at target size"
    );
    assert_eq!(pixels[16 * 32 + 16], Color::red().premultiplied());

    std::fs::remove_file(path).expect("remove solid avatar fixture");
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

#[test]
fn constrained_avatar_uses_a_centered_square_and_actual_frame_font_size() {
    let images = ImageService::new();
    let avatar = Avatar::new("X").size(64.0);
    let (_, display_list) =
        render_avatar_in(&avatar, &images, Rect::new(10.0, 5.0, 40.0, 20.0), (80, 40));

    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 5.0, w: 40.0, h: 20.0 } }"),
        "Avatar must clip all paint to the assigned frame: {display_list}"
    );
    assert!(
        display_list
            .contains("TextCenter { text: \"X\", rect: Rect { x: 20.0, y: 5.0, w: 20.0, h: 20.0 }"),
        "Avatar fallback must use the centered square control: {display_list}"
    );
    assert_eq!(recorded_text_font_size(&display_list), 9.0);
}

#[test]
fn avatar_scales_long_fallback_text_to_the_available_inner_width() {
    let images = ImageService::new();
    let avatar = Avatar::new("ABCDEFGHIJ");
    let (_, display_list) =
        render_avatar_in(&avatar, &images, Rect::new(0.0, 0.0, 32.0, 32.0), (64, 40));

    let font_size = recorded_text_font_size(&display_list);
    assert!(
        font_size < 8.0,
        "long initials must be measured and reduced, got {font_size}: {display_list}"
    );
}

#[test]
fn avatar_default_fallback_uses_the_high_contrast_theme_text_color() {
    let images = ImageService::new();
    let avatar = Avatar::new("UI");
    let (_, display_list) =
        render_avatar_in(&avatar, &images, Rect::new(0.0, 0.0, 32.0, 32.0), (32, 32));
    let expected = format!("color: {:?}", DesignTokens::antd_light().color_text);

    assert!(
        display_list.contains(&expected),
        "Avatar fallback should use the theme text color: {display_list}"
    );
}

#[test]
fn square_avatar_center_crops_wide_sources_instead_of_stretching_them() {
    let (path, source) = avatar_test_image();
    let images = ImageService::new();
    let avatar = Avatar::new("AB").square(true).src(&source);

    let pixels = render_avatar(&avatar, &images);
    assert_eq!(
        pixels[0], 0,
        "loaded square image must keep the fallback rounded outline"
    );
    assert_eq!(pixels[16 * 32 + 1], Color::red().premultiplied());
    assert_eq!(pixels[16 * 32 + 30], Color::blue().premultiplied());

    std::fs::remove_file(path).expect("remove avatar fixture");
}
