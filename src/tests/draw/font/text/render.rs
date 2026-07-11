use super::*;
use crate::draw::font::font_service::FontService;

#[test]
fn new_sets_font_and_service() {
    let fs = FontService::new();
    let fh = FontHandle::default();
    let trs = TextRenderService::new(fh, &fs, 500.0);
    assert_eq!(trs.font, fh);
    assert_eq!(trs.max_text_width, 500.0);
}

#[test]
fn set_font_updates_handle() {
    let fs = FontService::new();
    let fh1 = FontHandle::new(1);
    let fh2 = FontHandle::new(2);
    let mut trs = TextRenderService::new(fh1, &fs, 500.0);
    assert_eq!(*trs.font(), fh1);
    trs.set_font(fh2);
    assert_eq!(*trs.font(), fh2);
}

#[test]
fn set_max_text_width_updates() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    assert_eq!(trs.max_text_width, 500.0);
    trs.set_max_text_width(800.0);
    assert_eq!(trs.max_text_width, 800.0);
    trs.set_max_text_width(0.0);
    assert_eq!(trs.max_text_width, 0.0);
}

#[test]
fn draw_text_empty_returns_early() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    trs.draw_text(&mut canvas, "", Point::new(0.0, 0.0), Color::black(), 14.0);
}

#[test]
fn draw_text_baseline_empty_returns_early() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    trs.draw_text_baseline(&mut canvas, "", 0.0, 0.0, Color::black(), 14.0);
}

#[test]
fn text_center_empty_returns_early() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    trs.text_center(
        &mut canvas,
        "",
        Rect::new(0.0, 0.0, 100.0, 50.0),
        Color::black(),
        14.0,
    );
}

#[test]
fn draw_text_in_frame_empty_returns_early() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    trs.draw_text_in_frame(
        &mut canvas,
        "",
        Rect::new(0.0, 0.0, 100.0, 50.0),
        Color::black(),
        14.0,
    );
}

#[test]
fn draw_text_wrapped_empty_returns_early() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    trs.draw_text_wrapped(
        &mut canvas,
        "",
        Rect::new(0.0, 0.0, 100.0, 50.0),
        Color::black(),
        14.0,
    );
}

#[test]
fn selection_rects_empty_text() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    let rects = trs.selection_rects(&mut canvas, "", 14.0, Point::new(0.0, 0.0), 0, 0);
    assert!(rects.is_empty());
}

#[test]
fn selection_rects_invalid_range() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    // start > end → empty
    let rects = trs.selection_rects(&mut canvas, "hello", 14.0, Point::new(0.0, 0.0), 3, 1);
    assert!(rects.is_empty());
}

#[test]
fn visual_center_y_default_font() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let center = trs.visual_center_y(Rect::new(0.0, 0.0, 100.0, 50.0), 14.0);
    // 只是验证不 panic 且有合理返回值
    assert!(center >= 0.0);
}

#[test]
fn draw_text_with_selection_no_selection() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    // selection=None 时只绘制文本
    trs.draw_text_with_selection(
        &mut canvas,
        "",
        Point::new(0.0, 0.0),
        Color::black(),
        14.0,
        None,
        Color::blue(),
    );
}

#[test]
fn font_service_ref_accessible() {
    let fs = FontService::new();
    let trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    // font_service 指针应指向传入的 fs
    assert_eq!(trs.font_service as *const _, &fs as *const _);
}

#[test]
fn draw_text_spatial_empty_returns_early() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let mut canvas_s = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    let spatial = crate::draw::spatial::SpatialContext::new(
        &mut canvas_s,
        96.0,
        1.0,
        crate::draw::spatial::Orientation::YDown,
        800,
        600,
    );
    let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    trs.draw_text_spatial(
        &mut canvas,
        &spatial,
        "",
        Vec3::zero(),
        Color::black(),
        PhysicalUnit::Px(14.0),
    );
}

#[test]
fn text_center_spatial_empty_returns_early() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let mut canvas_s = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    let spatial = crate::draw::spatial::SpatialContext::new(
        &mut canvas_s,
        96.0,
        1.0,
        crate::draw::spatial::Orientation::YDown,
        800,
        600,
    );
    let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    trs.text_center_spatial(
        &mut canvas,
        &spatial,
        "",
        AABB3D::new(Vec3::zero(), Vec3::new(100.0, 50.0, 0.0)),
        Color::black(),
        PhysicalUnit::Px(14.0),
    );
}

#[test]
fn measure_text_returns_size() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let sz = trs.measure_text("hello", 14.0);
    // 默认字体（ab_glyph）能成功布局，返回实际宽高
    assert!(sz.w > 0.0);
    assert!(sz.h > 0.0);
}

#[test]
fn measure_text_empty_returns_minimal_height() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let sz = trs.measure_text("", 14.0);
    // 空文本：宽度为 0，高度为行高
    assert_eq!(sz.w, 0.0);
    assert!(sz.h > 0.0);
}

#[test]
fn measure_text_wrapped_returns_size() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let sz = trs.measure_text_wrapped("hello world", 14.0, 100.0);
    // 换行模式下返回实际布局尺寸
    assert!(sz.w > 0.0);
    assert!(sz.h > 0.0);
}

#[test]
fn text_hit_test_works_with_default_font() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let result = trs.text_hit_test("hello", 14.0, Point::new(0.0, 0.0));
    // 默认字体可用，命中测试返回有效索引
    assert!(result.is_some());
}

#[test]
fn text_cursor_x_returns_position() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let x = trs.text_cursor_x("hello", 14.0, 0);
    // 默认字体可用，返回有效 x 坐标
    assert!(x >= 0.0);
    // 索引 3 应在索引 0 之后
    let x3 = trs.text_cursor_x("hello", 14.0, 3);
    assert!(x3 >= x);
}

#[test]
fn draw_text_with_selection_empty_renders_text() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    // 空 selection 应只绘制文本
    trs.draw_text_with_selection(
        &mut canvas,
        "text",
        Point::new(0.0, 0.0),
        Color::black(),
        14.0,
        None,
        Color::blue(),
    );
}

#[test]
fn draw_text_with_selection_empty_range_skips_bg() {
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    // selection(0,0) 是空范围 → 跳过背景矩形
    trs.draw_text_with_selection(
        &mut canvas,
        "text",
        Point::new(0.0, 0.0),
        Color::black(),
        14.0,
        Some((0, 0)),
        Color::blue(),
    );
}

#[test]
fn blit_to_with_empty_layout_no_panic() {
    use crate::draw::font::text_backend::TextLayout;
    let fs = FontService::new();
    let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
    let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    let layout = TextLayout {
        width: 0.0,
        height: 0.0,
        lines: vec![],
        glyphs: vec![],
    };
    trs.blit_to(
        &mut canvas,
        &layout,
        Point::new(0.0, 0.0),
        Color::black(),
        14.0,
    );
}

#[test]
fn visual_center_y_is_em_box_geometry_without_optical_nudge() {
    let mut fs = FontService::new();
    let Some(segoe) = fs.load_font_from_path(r"C:\Windows\Fonts\segoeui.ttf", 14.0) else {
        return;
    };
    let m = fs
        .horizontal_line_metrics(&segoe, 14.0)
        .expect("metrics");
    let mut trs = TextRenderService::new(segoe, &fs, 500.0);
    let row = Rect::new(0.0, 0.0, 200.0, 40.0);
    let y = trs.visual_center_y(row, 14.0);
    let em_top = row.y + (row.h - m.ascent - m.descent) * 0.5;
    assert!(
        (y - em_top).abs() < 0.1,
        "visual_center_y must be pure em-box top, got {y} em_top={em_top}"
    );
}

#[test]
fn line_box_height_matches_ascent_plus_descent() {
    let mut fs = FontService::new();
    let Some(segoe) = fs.load_font_from_path(r"C:\Windows\Fonts\segoeui.ttf", 14.0) else {
        return;
    };
    let m = fs
        .horizontal_line_metrics(&segoe, 14.0)
        .expect("metrics");
    let mut trs = TextRenderService::new(segoe, &fs, 500.0);
    let h = trs.line_box_height(14.0);
    assert!(
        (h - (m.ascent + m.descent)).abs() < 0.1,
        "line_box_height={h} ascent+descent={}",
        m.ascent + m.descent
    );
}

#[test]
fn button_content_centers_line_box_not_optical_ink() {
    // 布局契约：content 内居中行盒 (ascent+descent)，draw_text 顶对齐。
    let mut fs = FontService::new();
    let Some(segoe) = fs.load_font_from_path(r"C:\Windows\Fonts\segoeui.ttf", 14.0) else {
        return;
    };
    if let Some(h) = fs.load_font_from_path(r"C:\Windows\Fonts\msyh.ttc", 14.0) {
        fs.add_fallback(h);
        fs.set_fallback_chain(&[h]);
    }
    fs.loaded_font_handle = segoe;
    let mut trs = TextRenderService::new(segoe, &fs, 500.0);
    let content = Rect::new(0.0, 0.0, 60.0, 32.0);
    let fs_px = 14.0;
    let line_h = trs.line_box_height(fs_px);
    let text_w = trs.measure_text("+1", fs_px).w;
    let text_rect = Rect::new(
        content.x + (content.w - text_w) * 0.5,
        content.y + (content.h - line_h) * 0.5,
        text_w,
        line_h,
    );
    let box_cy = text_rect.y + text_rect.h * 0.5;
    let content_cy = content.y + content.h * 0.5;
    assert!(
        (box_cy - content_cy).abs() < 0.1,
        "line box center must match content center: box={box_cy} content={content_cy}"
    );
    // 行顶 = visual_center_y（纯 em-box）
    let y = trs.visual_center_y(content, fs_px);
    assert!(
        (y - text_rect.y).abs() < 0.1,
        "draw origin y={y} should equal text_rect.y={}",
        text_rect.y
    );
}

#[test]
fn missing_glyph_emits_tofu_with_char_index() {
    use crate::draw::font::text_backend::{TextLayoutOptions, TOFU_GLYPH_ID};
    use crate::draw::{HAlign, VAlign};

    let mut fs = FontService::new();
    // 仅加载拉丁字体，不含 CJK → 「中」应走 tofu
    let Some(segoe) = fs.load_font_from_path(r"C:\Windows\Fonts\segoeui.ttf", 14.0) else {
        return;
    };
    fs.loaded_font_handle = segoe;

    let opts = TextLayoutOptions {
        max_width: f32::MAX,
        max_height: 0.0,
        line_height: 21.0,
        word_wrap: false,
        h_align: HAlign::Left,
        v_align: VAlign::Top,
        font_size: 14.0,
    };
    let layout = fs.layout_text(&segoe, "A中B", &opts);
    assert_eq!(layout.glyphs.len(), 3, "must keep one glyph per char");
    assert_eq!(layout.glyphs[0].char_index, 0);
    assert_eq!(layout.glyphs[1].char_index, 1);
    assert_eq!(layout.glyphs[2].char_index, 2);
    assert_eq!(layout.glyphs[1].glyph_id, TOFU_GLYPH_ID);

    let tofu = fs.rasterize_glyph(&segoe, TOFU_GLYPH_ID, 14.0);
    assert!(tofu.width > 0 && tofu.height > 0, "tofu must rasterize");

    // 光标按字符下标：索引 1 应对齐「中」的 x
    let x1 = fs.text_cursor_x(&segoe, "A中B", &opts, 1);
    assert!((x1 - layout.glyphs[1].x).abs() < 0.5);
}

#[test]
fn hit_test_returns_char_index_not_glyph_slot() {
    use crate::draw::font::text_backend::TextLayoutOptions;
    use crate::draw::{HAlign, VAlign};

    let mut fs = FontService::new();
    let Some(segoe) = fs.load_font_from_path(r"C:\Windows\Fonts\segoeui.ttf", 14.0) else {
        return;
    };
    fs.loaded_font_handle = segoe;
    let opts = TextLayoutOptions {
        max_width: f32::MAX,
        max_height: 0.0,
        line_height: 21.0,
        word_wrap: false,
        h_align: HAlign::Left,
        v_align: VAlign::Top,
        font_size: 14.0,
    };
    let layout = fs.layout_text(&segoe, "Hi", &opts);
    assert!(layout.glyphs.len() >= 2);
    let mid = Point::new(layout.glyphs[1].x + 0.1, layout.glyphs[1].y);
    let hit = fs.hit_test_text(&segoe, "Hi", &opts, mid).expect("hit");
    assert_eq!(hit, layout.glyphs[1].char_index);
}
