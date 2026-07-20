//! 轮廓 → GPU atlas 边列表（解析 AA / MSDF）。

use std::sync::Arc;

use crate::draw::font::font_service::FontService;
use crate::draw::font::glyph_outline::{
    colorize_edges, coverage_from_edges, coverage_from_msdf, is_outline_edges,
    msdf_encoded_to_coverage, msdf_from_edges, EDGE_BLUE, EDGE_CYAN, EDGE_GREEN, EDGE_MAGENTA,
    EDGE_RED, EDGE_WHITE, EDGE_YELLOW, MSDF_RANGE,
};
use crate::draw::font::text_backend::TOFU_GLYPH_ID;

#[test]
fn rasterize_glyph_prefers_outline_edges_without_cpu_coverage() {
    let mut fonts = FontService::new();
    let handle = fonts
        .load_font(include_bytes!("../../../../assets/fonts/lucide.ttf"))
        .expect("load lucide");
    // Lucide 图标字体：取一个非 tofu glyph。
    let mut glyph_id = 1u32;
    while glyph_id < 512 {
        let raster = fonts.rasterize_glyph(&handle, glyph_id, 24.0);
        if raster.width > 0 && raster.height > 0 && glyph_id != TOFU_GLYPH_ID {
            let mesh = raster
                .outline_mesh
                .as_ref()
                .expect("outlined glyph should expose edges for GPU coverage");
            assert!(is_outline_edges(mesh));
            assert!(
                raster.coverage.is_empty(),
                "GPU outline path must skip ab_glyph CPU coverage pixels"
            );
            // 缓存命中仍保留边列表。
            let again = fonts.rasterize_glyph(&handle, glyph_id, 24.0);
            assert!(again.outline_mesh.is_some());
            assert!(Arc::ptr_eq(
                mesh,
                again.outline_mesh.as_ref().expect("cached edges")
            ));
            return;
        }
        glyph_id += 1;
    }
    panic!("lucide font produced no outline edge glyphs");
}

#[test]
fn coverage_from_edges_fills_solid_rect_interior() {
    // 非整数边，确保像素中心落在 AA 带宽内。
    let edges: Vec<f32> = vec![
        1.2, 1.2, 4.8, 1.2, //
        4.8, 1.2, 4.8, 4.8, //
        4.8, 4.8, 1.2, 4.8, //
        1.2, 4.8, 1.2, 1.2,
    ];
    let coverage = coverage_from_edges(&edges, 6, 6).expect("coverage");
    assert_eq!(coverage.len(), 36);
    // 中心像素应接近满覆盖。
    assert!(
        coverage[3 * 6 + 3] >= 250,
        "interior coverage={}, expected near 255",
        coverage[3 * 6 + 3]
    );
    // 槽位角落在矩形外，应为 0。
    assert_eq!(coverage[0], 0);
    assert_eq!(coverage[5], 0);
    // 紧贴左边内侧的像素应有部分 AA（中心到 x=1.2 距离 0.3）。
    let edge = coverage[3 * 6 + 1];
    assert!(
        edge > 0 && edge < 255,
        "edge AA coverage={edge}, expected fractional"
    );
}

#[test]
fn colorize_edges_assigns_dual_channel_colors_at_rect_corners() {
    // 轴对齐矩形：四角均为角点，边应拿到 CMY 双通道色（非单通道扇区）。
    let edges: Vec<f32> = vec![
        1.0, 1.0, 5.0, 1.0, // →
        5.0, 1.0, 5.0, 5.0, // ↓
        5.0, 5.0, 1.0, 5.0, // ←
        1.0, 5.0, 1.0, 1.0, // ↑
    ];
    let colors = colorize_edges(&edges).expect("colorize");
    assert_eq!(colors.len(), 4);
    for &c in &colors {
        let channels = (c & EDGE_RED != 0) as u8
            + (c & EDGE_GREEN != 0) as u8
            + (c & EDGE_BLUE != 0) as u8;
        assert!(
            channels >= 2,
            "Chlumsky edge color {c:#x} must enable ≥2 channels"
        );
        assert!(
            matches!(
                c,
                EDGE_YELLOW | EDGE_CYAN | EDGE_MAGENTA | EDGE_WHITE
            ),
            "unexpected edge color {c:#x}"
        );
    }
    // 相邻边在角点切换后颜色应不同，才能保角。
    assert_ne!(colors[0], colors[1]);
    assert_ne!(colors[1], colors[2]);
    assert_ne!(colors[2], colors[3]);
}

#[test]
fn msdf_from_edges_encodes_interior_and_samples_to_coverage() {
    let edges: Vec<f32> = vec![
        1.2, 1.2, 4.8, 1.2, //
        4.8, 1.2, 4.8, 4.8, //
        4.8, 4.8, 1.2, 4.8, //
        1.2, 4.8, 1.2, 1.2,
    ];
    let msdf = msdf_from_edges(&edges, 6, 6).expect("msdf");
    assert_eq!(msdf.len(), 6 * 6 * 4);
    // 中心：内部 → 负距离 → 编码 < 0.5；A=255。
    let o = (3 * 6 + 3) * 4;
    assert_eq!(msdf[o + 3], 255);
    let r = msdf[o] as f32 / 255.0;
    let g = msdf[o + 1] as f32 / 255.0;
    let b = msdf[o + 2] as f32 / 255.0;
    assert!(
        r < 0.5 && g < 0.5 && b < 0.5,
        "interior MSDF rgb=({r},{g},{b}) expected < 0.5"
    );
    let cov = msdf_encoded_to_coverage(r, g, b);
    assert!(
        cov > 0.95,
        "interior MSDF coverage={cov}, expected near 1"
    );
    // 角落在外：编码 > 0.5，coverage≈0。
    let outside = msdf_encoded_to_coverage(
        msdf[0] as f32 / 255.0,
        msdf[1] as f32 / 255.0,
        msdf[2] as f32 / 255.0,
    );
    assert!(
        outside < 0.05,
        "exterior MSDF coverage={outside}, expected near 0"
    );
    let from_msdf = coverage_from_msdf(&msdf, 6, 6).expect("coverage from msdf");
    assert!(from_msdf[3 * 6 + 3] >= 240);
    assert!(from_msdf[0] <= 12);
    // 编码半宽契约。
    assert!((MSDF_RANGE - 4.0).abs() < 1e-6);
}

#[test]
fn msdf_median_recovers_corner_better_than_single_channel_proxy() {
    // 模拟角点：两通道靠近 0.5（边），第三通道远离；median 仍贴边。
    let m = msdf_encoded_to_coverage(0.5, 0.5, 0.9);
    assert!(
        (m - 0.5).abs() < 0.05,
        "median of two edge channels should stay near AA mid, got {m}"
    );
}

#[test]
fn colorize_smooth_contour_uses_single_dual_channel_color() {
    // 密多边形：相邻边转角 < arcsin(sin(3))≈8° → 无角点 → 整圈同色。
    let n = 64usize;
    let mut edges = Vec::with_capacity(n * 4);
    let cx = 3.0f32;
    let cy = 3.0f32;
    let r = 2.0f32;
    for i in 0..n {
        let a0 = (i as f32) / (n as f32) * std::f32::consts::TAU;
        let a1 = ((i + 1) as f32) / (n as f32) * std::f32::consts::TAU;
        edges.push(cx + r * a0.cos());
        edges.push(cy + r * a0.sin());
        edges.push(cx + r * a1.cos());
        edges.push(cy + r * a1.sin());
    }
    let colors = colorize_edges(&edges).expect("colorize");
    assert_eq!(colors.len(), n);
    let first = colors[0];
    assert!(first == EDGE_CYAN || first == EDGE_MAGENTA || first == EDGE_YELLOW);
    assert!(
        colors.iter().all(|&c| c == first),
        "smooth contour must share one dual-channel color, got {:?}",
        colors
    );
}
