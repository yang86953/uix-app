//! TTF 渲染诊断 — 在文字上绘制辅助线
use crate::graphics::{Color, GraphicsEngine, Point, Rect};
use crate::graphics::software_engine::FontData;

pub fn draw_text_with_guides(
    engine: &mut dyn GraphicsEngine,
    font: &FontData,
    text: &str,
    pos: Point,
    color: Color,
    font_size: f32,
) {
    let fs = font_size.max(1.0);
    
    // 获取字体度量
    let ascent = font.font.horizontal_line_metrics(fs)
        .map(|m| m.ascent).unwrap_or(fs * 0.8);
    let descent = font.font.horizontal_line_metrics(fs)
        .map(|m| m.descent).unwrap_or(fs * -0.2);
    
    // 渲染文本（正常绘制）
    let opts = crate::graphics::TextLayoutOptions {
        max_width: f32::MAX,
        line_height: font_size + 2.0,
        word_wrap: false,
        h_align: crate::graphics::HAlign::Left,
        v_align: crate::graphics::VAlign::Top,
        font_size: fs,
    };
    // 使用 engine 绘制文本
    engine.draw_text(&crate::graphics::FontHandle::default(), text, pos, color, &opts);
    
    // 计算度量线
    let mut cursor_x = pos.x;
    
    // 对每个字符绘制辅助线
    for ch in text.chars() {
        let metrics = font.font.metrics(ch, fs);
        let (raster, _) = font.font.rasterize(ch, fs);
        let advance = metrics.advance_width;
        
        // glyph 顶部 (VAlign::Top 下 = pos.y)
        let top_y = pos.y;
        // baseline
        let baseline_y = pos.y + ascent;
        // glyph 底部
        let bottom = pos.y + ascent + descent;
        // glyph 左边界
        let gx = cursor_x + raster.xmin as f32;
        // glyph 右边界
        let gx2 = gx + raster.width as f32;
        
        // 绘制 glyph bbox（红色虚线框）
        let bbox = Rect::new(gx, top_y, raster.width as f32, raster.height as f32);
        engine.stroke_rect(bbox, Color::from_rgba(255, 0, 0, 180), 1.0, None);
        
        // 绘制基线（蓝色实线）
        engine.fill_rect(
            Rect::new(cursor_x, baseline_y - 0.5, advance.max(2.0), 1.0),
            Color::from_rgba(0, 100, 255, 200),
            None,
        );
        
        // 绘制 ascent 线（绿色）
        let ascent_y = pos.y;
        engine.fill_rect(
            Rect::new(cursor_x, ascent_y - 0.5, advance.max(2.0), 1.0),
            Color::from_rgba(0, 200, 0, 150),
            None,
        );
        
        // 绘制 advance 标记（紫色竖线）
        let adv_x = cursor_x + advance;
        engine.fill_rect(
            Rect::new(adv_x - 0.5, pos.y - 2.0, 1.0, (bottom - pos.y).max(4.0)),
            Color::from_rgba(200, 0, 200, 180),
            None,
        );
        
        // 绘制 origin 点（黄色小圆点）
        engine.fill_rect(
            Rect::new(cursor_x - 1.0, baseline_y - 1.0, 3.0, 3.0),
            Color::from_rgba(255, 255, 0, 220),
            None,
        );
        
        cursor_x += advance;
    }
}
