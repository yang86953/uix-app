// 引入同一字体解析器，独立计算光栅后端采用的字形推进。
use ab_glyph::{Font as _, FontRef, PxScale, ScaleFont as _};
// 引入公开字体后端与布局契约，覆盖应用实际调用边界。
use uix::draw::resources::font::text_backend::TextLayoutOptions;
use uix::draw::resources::font::text_backends::ab_glyph::AbGlyphBackend;
use uix::draw::{HAlign, TextBackend, VAlign};

#[test]
fn shaped_advances_match_bundled_font_raster_scale() {
    // 使用主演示的确定性字体复现字体高度与 UPEM 不相等的条件。
    let data = include_bytes!("../assets/fonts/NotoSansCJKsc-Regular.otf");
    // 通过公开后端加载与主演示相同的字体字节。
    let mut backend = AbGlyphBackend::new();
    let handle = backend
        .load_font(data)
        .expect("主演示字体必须能由 ab_glyph 后端加载");
    // 使用与 UI 正文一致的有限单行布局约束。
    let options = TextLayoutOptions {
        max_width: 4096.0,
        max_height: 0.0,
        line_height: 30.0,
        word_wrap: false,
        h_align: HAlign::Left,
        v_align: VAlign::Top,
        font_size: 20.0,
    };
    // 选择无连写和字距调整的 CJK 文本，直接比较每个字形推进总量。
    let text = "组件全景";
    let layout = backend.layout_text(&handle, text, &options);
    let font = FontRef::try_from_slice(data).expect("主演示字体必须能独立解析");
    let scaled = font.as_scaled(PxScale::from(options.font_size));
    let expected_width = text
        .chars()
        .map(|character| scaled.h_advance(scaled.glyph_id(character)))
        .sum::<f32>();
    // shaping 坐标和实际光栅必须共享同一设计单位缩放。
    assert!((layout.width - expected_width).abs() < 0.01);
}
