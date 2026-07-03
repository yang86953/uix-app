//! uix-graphics BitmapFont 集成测试。
//! 覆盖 5x7 位图字体的度量、测量和绘制。

use uix_graphics::bitmap_font::BitmapFont;
use uix_graphics::types::TextLayoutOptions;
use uix_platform::geometry::{Point, Size};

// ════════════════════════════════════════════════════════════════════════════
// 基础构造与度量
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn bitmap_font_new() {
    let font = BitmapFont::new();
    assert!((font.char_width() - 6.0).abs() < 1e-6);
    assert!((font.char_height() - 9.0).abs() < 1e-6);
    assert!((font.line_height() - 10.0).abs() < 1e-6);
}

#[test]
fn bitmap_font_default() {
    let font = BitmapFont::default();
    assert!((font.char_width() - 6.0).abs() < 1e-6);
}

// ════════════════════════════════════════════════════════════════════════════
// 文本测量
// ════════════════════════════════════════════════════════════════════════════

fn default_opts() -> TextLayoutOptions {
    TextLayoutOptions {
        max_width: 0.0,
        max_height: 0.0,
        line_height: 10.0,
        word_wrap: false,
        h_align: uix_graphics::HAlign::Left,
        v_align: uix_graphics::VAlign::Top,
        font_size: 12.0,
    }
}

#[test]
fn measure_empty_text() {
    let font = BitmapFont::new();
    let sz = font.measure("", &default_opts());
    assert_eq!(sz, Size::zero());
}

#[test]
fn measure_single_char() {
    let font = BitmapFont::new();
    let sz = font.measure("A", &default_opts());
    assert!((sz.w - 6.0).abs() < 1e-6);
    assert!((sz.h - 10.0).abs() < 1e-6);
}

#[test]
fn measure_multiple_chars() {
    let font = BitmapFont::new();
    let sz = font.measure("Hello", &default_opts());
    // 5 chars * 6px = 30px
    assert!((sz.w - 30.0).abs() < 1e-6);
    assert!((sz.h - 10.0).abs() < 1e-6);
}

#[test]
fn measure_with_newline() {
    let font = BitmapFont::new();
    let sz = font.measure("A\nB", &default_opts());
    // 2 个非换行符 × 6px = 12px 宽度，1 个换行 + 1 = 2 行 = 20px 高度
    assert!((sz.w - 12.0).abs() < 1e-6);
    assert!((sz.h - 20.0).abs() < 1e-6);
}

#[test]
fn measure_word_wrap_no_wrap_needed() {
    let font = BitmapFont::new();
    let opts = TextLayoutOptions {
        max_width: 100.0,
        word_wrap: true,
        ..default_opts()
    };
    let sz = font.measure("Hi", &opts);
    assert!((sz.w - 12.0).abs() < 1e-6);
    assert!((sz.h - 10.0).abs() < 1e-6);
}

#[test]
fn measure_word_wrap_forced() {
    let font = BitmapFont::new();
    let opts = TextLayoutOptions {
        max_width: 10.0,
        word_wrap: true,
        ..default_opts()
    };
    // "Hello" 每个字符 6px，超过 10px 时每个字符都需换行 → 5 行
    let sz = font.measure("Hello", &opts);
    assert!(sz.w <= 10.0);
    assert!((sz.h - 50.0).abs() < 1e-6);
}

#[test]
fn measure_all_ascii_printable() {
    let font = BitmapFont::new();
    let text: String = (32u8..127).map(|c| c as char).collect();
    let sz = font.measure(&text, &default_opts());
    // 95 chars * 6px = 570px
    assert!((sz.w - 570.0).abs() < 1e-6);
    assert!((sz.h - 10.0).abs() < 1e-6);
}

#[test]
fn measure_max_width_clamping() {
    let font = BitmapFont::new();
    let opts = TextLayoutOptions {
        max_width: 20.0,
        word_wrap: false,
        ..default_opts()
    };
    let sz = font.measure("HelloWorld", &opts);
    // 不换行时，宽度被 max_width 限制
    assert!((sz.w - 20.0).abs() < 1e-6);
}

// ════════════════════════════════════════════════════════════════════════════
// 像素绘制
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn draw_colored_basic_char() {
    let font = BitmapFont::new();
    let mut pixels = vec![0u32; 100 * 20]; // 100x20 缓冲
    let mut count = 0usize;
    font.draw_colored("A", Point::new(0.0, 0.0), 0xFFFFFFFF, |x, y, color| {
        let idx = (y * 100 + x) as usize;
        if idx < pixels.len() {
            pixels[idx] = color;
        }
        count += 1;
    });
    // 'A' 至少有 5 个列 × 若干行有像素
    assert!(count > 5);
    // 应有白色像素写入
    assert!(pixels.iter().any(|&p| p == 0xFFFFFFFF));
}

#[test]
fn draw_colored_empty_text() {
    let font = BitmapFont::new();
    let mut count = 0usize;
    font.draw_colored("", Point::new(0.0, 0.0), 0xFFFFFFFF, |_x, _y, _c| {
        count += 1;
    });
    assert_eq!(count, 0);
}

#[test]
fn draw_colored_newline() {
    let font = BitmapFont::new();
    let mut rows_used = std::collections::HashSet::new();
    font.draw_colored("A\nB", Point::new(0.0, 0.0), 0xFFFFFFFF, |_x, y, _c| {
        rows_used.insert(y);
    });
    // 应在两行都有像素（被 y offset 分开）
    assert!(rows_used.len() >= 1);
    // 第二行应比第一行大 lh（9）
    let min_y = *rows_used.iter().min().unwrap();
    let max_y = *rows_used.iter().max().unwrap();
    assert!(max_y > min_y);
}

#[test]
fn draw_colored_out_of_range_char() {
    let font = BitmapFont::new();
    let mut count = 0usize;
    // 0x00 不在 ASCII 32-126 范围内
    font.draw_colored("\x00\x01\x1F", Point::new(0.0, 0.0), 0xFFFFFFFF, |_x, _y, _c| {
        count += 1;
    });
    // 不可见字符应跳过，不绘制任何像素
    assert_eq!(count, 0);
}

#[test]
fn draw_colored_whitespace() {
    let font = BitmapFont::new();
    let mut count = 0usize;
    font.draw_colored(" ", Point::new(0.0, 0.0), 0xFFFFFFFF, |_x, _y, _c| {
        count += 1;
    });
    // 空格（ASCII 32）有字形数据但全为 0 → 无像素
    assert_eq!(count, 0);
}

#[test]
fn draw_colored_offset_position() {
    let font = BitmapFont::new();
    let mut pixels = vec![0u32; 200 * 200];
    font.draw_colored("X", Point::new(50.0, 60.0), 0xFF0000FF, |x, y, color| {
        let idx = (y * 200 + x) as usize;
        if idx < pixels.len() {
            pixels[idx] = color;
        }
    });
    // 像素应在 pos=(50,60) 附近
    assert_eq!(pixels[60 * 200 + 50], 0xFF0000FF);
    // 远距离位置应无像素
    assert_eq!(pixels[0], 0x00000000);
}
