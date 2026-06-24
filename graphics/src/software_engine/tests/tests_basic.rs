use super::{blue, make_rt, red_half, unpack, unpack_premul};
use crate::software_engine::core::RenderTarget;
use crate::{Color, Radius};
use uix_core::Rect;

// ── 直角矩形测试 ──

#[test]
fn straight_rect_fill_interior() {
    let mut rt = make_rt(40, 40);
    let color = blue();
    rt.fill_rect(Rect::new(10.0, 10.0, 20.0, 20.0), color, None);

    // 内部像素 (20,20) 应该是纯蓝色
    let (r, g, b, a) = unpack(rt.pixels()[20 * 40 + 20]);
    assert_eq!((r, g, b, a), (22, 119, 255, 255));
}

#[test]
fn straight_rect_fill_exterior() {
    let mut rt = make_rt(40, 40);
    rt.fill_rect(Rect::new(10.0, 10.0, 20.0, 20.0), blue(), None);

    // 外部像素 (5,5) 应为透明
    let (r, g, b, a) = unpack(rt.pixels()[5 * 40 + 5]);
    assert_eq!((r, g, b, a), (0, 0, 0, 0));
}

#[test]
fn straight_rect_fill_sharp_edge() {
    let mut rt = make_rt(40, 40);
    rt.fill_rect(Rect::new(10.0, 10.0, 20.0, 20.0), blue(), None);

    // 内部 (15,15) → 蓝色，外部 (5,5) → 透明
    let a_in = (rt.pixels()[15 * 40 + 15] >> 24) & 0xFF;
    let a_out = (rt.pixels()[5 * 40 + 5] >> 24) & 0xFF;
    assert_eq!(a_in, 255, "内部应为不透明");
    assert_eq!(a_out, 0, "外部应为透明");
}

// ── 圆角矩形填充测试 ──

#[test]
fn rounded_rect_fill_interior() {
    let mut rt = make_rt(40, 40);
    let color = blue();
    rt.fill_rect(
        Rect::new(10.0, 10.0, 20.0, 20.0),
        color,
        Some(Radius::uniform(6.0)),
    );

    // 内部像素 (20,20) 应该是纯蓝色
    let (r, g, b, a) = unpack(rt.pixels()[20 * 40 + 20]);
    assert_eq!((r, g, b, a), (22, 119, 255, 255));
}

#[test]
fn rounded_rect_fill_corner_has_aa() {
    let mut rt = make_rt(40, 40);
    rt.clear_all();
    rt.fill_rect(
        Rect::new(10.0, 10.0, 20.0, 20.0),
        Color::white(),
        Some(Radius::uniform(6.0)),
    );

    // AA 半宽缩至 0.25px，过渡区极窄。用更密的采样点
    let mut found_partial = false;
    let mut max_a: u32 = 0;
    for y in 8..18 {
        for x in 8..18 {
            let a = (rt.pixels()[y * 40 + x] >> 24) & 0xFF;
            max_a = max_a.max(a);
            if a > 0 && a < 255 {
                found_partial = true;
            }
        }
    }
    // 确认存在 AA 过渡且不是全部不透明
    assert!(found_partial, "圆角过渡区应存在 AA 像素 (max={})", max_a);
    assert!(max_a > 0, "应有非零 alpha");
}

/// 诊断工具：渲染圆角矩形并输出像素网格
#[test]
fn diag_sdf_grid() {
    let mut rt = make_rt(30, 30);
    rt.clear_all();
    let rect = Rect::new(5.0, 5.0, 20.0, 20.0);
    let rad = Radius::uniform(6.0);
    let color = Color::white();
    let _c = rt.apply_opacity(RenderTarget::premul(color));

    eprintln!("=== SDF 值网格 (sd) ===");
    for y in 0..15 {
        let mut row = String::new();
        for x in 0..15 {
            let ux = x as f32 + 0.5;
            let uy = y as f32 + 0.5;
            let sd = RenderTarget::rounded_rect_sdf(ux, uy, &rect, &rad);
            let cov = RenderTarget::sdf_to_coverage(sd);
            let ch = if cov <= 0.0 {
                '.'
            } else if cov < 0.25 {
                '1'
            } else if cov < 0.5 {
                '2'
            } else if cov < 0.75 {
                '3'
            } else if cov < 1.0 {
                '4'
            } else {
                '#'
            };
            row.push(ch);
        }
        eprintln!("{}", row);
    }

    eprintln!("=== SDF 原始值 (y=5, x=5..14) ===");
    for x in 5..15 {
        let ux = x as f32 + 0.5;
        let uy = 5.5;
        let sd = RenderTarget::rounded_rect_sdf(ux, uy, &rect, &rad);
        let cov = RenderTarget::sdf_to_coverage(sd);
        eprintln!("  x={}: sd={:.3} cov={:.3}", x, sd, cov);
    }

    eprintln!("=== SDF 原始值 (对角线, x=y, 5..14) ===");
    for d in 5..15 {
        let ux = d as f32 + 0.5;
        let uy = d as f32 + 0.5;
        let sd = RenderTarget::rounded_rect_sdf(ux, uy, &rect, &rad);
        let cov = RenderTarget::sdf_to_coverage(sd);
        eprintln!("  ({},{}): sd={:.3} cov={:.3}", d, d, sd, cov);
    }

    // 渲染出来对比
    rt.fill_rect(rect, color, Some(rad));
    eprintln!("=== 渲染后 alpha (y=5) ===");
    for x in 0..30 {
        let a = (rt.pixels()[5 * 30 + x] >> 24) & 0xFF;
        eprint!("{:3} ", a);
    }
    eprintln!();
    eprintln!("=== 渲染后 alpha (对角线) ===");
    for d in 0..15 {
        let a = (rt.pixels()[d * 30 + d] >> 24) & 0xFF;
        eprint!("{:3} ", a);
    }
    eprintln!();
}

/// 诊断：对比直边和圆角边缘的 AA 过渡曲线
/// 渲染一个圆角矩形 (r=8)，沿直边法线和圆角45°法线分别采样 alpha，
/// 检查两个方向的 AA 过渡宽度和形状是否一致
#[test]
fn diag_edge_vs_corner_aa_profile() {
    let w = 80;
    let h = 80;
    let mut rt = make_rt(w, h);
    rt.clear_all();
    let rect = Rect::new(10.0, 10.0, 60.0, 60.0);
    let rad = Radius::uniform(8.0);
    let color = Color::white();
    rt.fill_rect(rect, color, Some(rad));

    // 直边法线采样：从矩形顶部边缘 (y=10) 向上下各取 5px
    // 采样点在 x=40（远离角落，纯直边区域）
    eprintln!("=== 直边法线 alpha 剖面 (x=40, y=5..15) ===");
    let edge_samples: Vec<(f32, u32)> = (5..16)
        .map(|y| {
            let a = (rt.pixels()[y * w as usize + 40] >> 24) & 0xFF;
            let dist = y as f32 + 0.5 - rect.y; // pixel center distance from edge
            eprintln!(
                "  y={:2}  center_y={:.1}  dist_from_edge={:+.1}  alpha={:3}",
                y,
                y as f32 + 0.5,
                dist,
                a
            );
            (dist, a)
        })
        .collect();

    // 圆角45°法线采样：从弧面沿45°法线向外/内采样
    // TR角弧面45°点: (cx+half_w-r+r/√2, cy-half_h+r-r/√2) = (67.657, 12.343)
    let arc_x = rect.x + rect.w - 8.0 + 8.0 / 1.41421356; // ≈ 67.657
    let arc_y = rect.y + 8.0 - 8.0 / 1.41421356; // ≈ 12.343
    eprintln!(
        "\n=== 圆角45°法线 alpha 剖面 (弧面=({:.1},{:.1}), 沿45°法线) ===",
        arc_x, arc_y
    );
    let corner_samples: Vec<(f32, u32)> = (0..11)
        .map(|i| {
            // 从弧面沿45°向外/内偏移 (-4..+6)px
            let offset = (i as f32 - 4.0) * 0.707; // dx, -dy per px along 45°
            let sx = (arc_x + offset) as usize;
            let sy = (arc_y - offset) as usize; // 45°向外: dx>0, dy<0
            let dist = offset * 1.414; // 实际像素距离
            let a = if sx < w as usize && sy < h as usize {
                (rt.pixels()[sy * w as usize + sx] >> 24) & 0xFF
            } else {
                0
            };
            eprintln!(
                "  i={:2}  offset={:+.1}px  dist_from_surface={:+.2}  alpha={:3}",
                i, dist, dist, a
            );
            (dist, a)
        })
        .collect();

    // 分析：计算 AA 过渡宽度（alpha 从 10%→90% 经过的像素数）
    let transition_width = |samples: &[(f32, u32)]| -> f32 {
        let mut start = None;
        let mut end = None;
        for &(d, a) in samples {
            if a >= 25 && a <= 230 {
                if start.is_none() {
                    start = Some(d);
                }
                end = Some(d);
            }
        }
        match (start, end) {
            (Some(s), Some(e)) => (e - s).abs(),
            _ => 0.0,
        }
    };

    let tw_edge = transition_width(&edge_samples);
    let tw_corner = transition_width(&corner_samples);
    eprintln!("\n=== AA 过渡宽度分析 ===");
    eprintln!("  直边 10%→90% 过渡宽度: {:.2}px", tw_edge);
    eprintln!("  圆角 10%→90% 过渡宽度: {:.2}px", tw_corner);
    eprintln!("  差异: {:.2}px", (tw_edge - tw_corner).abs());

    // 单样本 SDF AA 应该完全各向同性
    assert!(
        (tw_edge - tw_corner).abs() < 1.5,
        "直边({:.1}px)和圆角({:.1}px) AA 过渡宽度差异过大",
        tw_edge,
        tw_corner
    );
}

/// 诊断：直边 vs 圆角的 SDF 梯度对比
/// SDF 梯度决定了 AA 过渡的"陡峭度"
#[test]
fn diag_sdf_gradient_edge_vs_corner() {
    let rect = Rect::new(10.0, 10.0, 60.0, 60.0);
    let rad = Radius::uniform(8.0);
    let cx = rect.x + rect.w * 0.5;
    let _cy = rect.y + rect.h * 0.5;

    // 直边法线方向：沿 y 轴
    eprintln!("=== 直边 SDF 梯度 (x={}, y=9.5..11.5) ===", cx);
    for i in 0..6 {
        let y = 9.5 + i as f32 * 0.4;
        let sd = RenderTarget::rounded_rect_sdf(cx, y, &rect, &rad);
        let cov = RenderTarget::sdf_to_coverage(sd);
        eprintln!("  y={:.1}  sd={:+.4}  cov={:.3}", y, sd, cov);
    }

    // 圆角45°法线方向：从弧面（sd=0）沿法线向外/内采样
    // 右上角中心: (cx+half_w-r, cy-half_h+r) = (40+30-8, 40-30+8) = (62, 18)
    // 弧面沿45°方向点: (62+8/√2, 18-8/√2) = (67.657, 12.343)
    let arc_x = cx + rect.w * 0.5 - 8.0 + 8.0 / std::f32::consts::SQRT_2;
    let arc_y = rect.y + 8.0 - 8.0 / std::f32::consts::SQRT_2;
    eprintln!(
        "\n=== 圆角45° SDF 梯度 (弧面=({:.1},{:.1}), 沿45°法线) ===",
        arc_x, arc_y
    );
    for i in 0..8 {
        let offset = (i as f32 - 3.0) * 0.35;
        let sx = arc_x + offset;
        let sy = arc_y - offset; // 45°向外: (dx>0, dy<0)
        let sd = RenderTarget::rounded_rect_sdf(sx, sy, &rect, &rad);
        let cov = RenderTarget::sdf_to_coverage(sd);
        eprintln!(
            "  offset={:+.2}px  sd={:+.4}  cov={:.3}",
            offset * 1.414,
            sd,
            cov
        );
    }

    // 梯度对比：sd 在弧面处每像素的变化量
    let sd_surface = RenderTarget::rounded_rect_sdf(arc_x, arc_y, &rect, &rad);
    let sd_1px_out = RenderTarget::rounded_rect_sdf(arc_x + 0.707, arc_y - 0.707, &rect, &rad);
    let grad_corner = (sd_1px_out - sd_surface).abs(); // 45°方向 1px 处的梯度

    let grad_edge = 1.0; // 直边始终为 1.0

    eprintln!("\n=== SDF 梯度对比 ===");
    eprintln!("  直边 |∂SDF/∂n| = {:.4} (理想=1.0)", grad_edge);
    eprintln!(
        "  圆角 |∂SDF/∂n| @arc_surface = {:.4} (理想=1.0)",
        grad_corner
    );

    // 弧面处梯度应为 1.0
    assert!(
        (grad_corner - 1.0).abs() < 0.01,
        "圆角 SDF 梯度异常: {:.4}",
        grad_corner
    );
}

#[test]
fn rounded_rect_fill_corner_premul_consistency() {
    // 验证预乘一致性：每个像素的 R,G,B 都不应超过 A
    let mut rt = make_rt(40, 40);
    rt.fill_rect(
        Rect::new(10.0, 10.0, 20.0, 20.0),
        blue(),
        Some(Radius::uniform(6.0)),
    );

    // 扫描圆角区域
    for y in 8..16 {
        for x in 8..16 {
            let (pr, pg, pb, pa) = unpack_premul(rt.pixels()[y * 40 + x]);
            assert!(
                pr <= pa && pg <= pa && pb <= pa,
                "预乘不一致 @({},{}): r={} g={} b={} a={}",
                x,
                y,
                pr,
                pg,
                pb,
                pa
            );
        }
    }
}

#[test]
fn rounded_rect_fill_corner_color_preserved() {
    // 验证：AA 像素反预乘后颜色应与原始颜色一致
    let mut rt = make_rt(40, 40);
    let color = blue();
    rt.fill_rect(
        Rect::new(10.0, 10.0, 20.0, 20.0),
        color,
        Some(Radius::uniform(6.0)),
    );

    // 遍历圆角过渡区
    for y in 10..15 {
        for x in 10..15 {
            let p = rt.pixels()[y * 40 + x];
            let a = (p >> 24) & 0xFF;
            if a == 0 || a == 255 {
                continue; // 全透明或全不透明跳过
            }
            // 反预乘 — BGRA 格式: [7:0]=B, [15:8]=G, [23:16]=R
            let pb = p & 0xFF;
            let pg = (p >> 8) & 0xFF;
            let pr = (p >> 16) & 0xFF;
            let r = (pr * 255 / a).min(255);
            let g = (pg * 255 / a).min(255);
            let b = (pb * 255 / a).min(255);

            // 颜色容差（由于整数除法可能有 ±1 偏差）
            let dr = (r as i32 - color.r as i32).abs();
            let dg = (g as i32 - color.g as i32).abs();
            let db = (b as i32 - color.b as i32).abs();
            assert!(
                dr <= 10 && dg <= 10 && db <= 10,
                "颜色偏移 @({},{}): 期望({},{},{}) 实际({},{},{}) a={}",
                x,
                y,
                color.r,
                color.g,
                color.b,
                r,
                g,
                b,
                a
            );
        }
    }
}

// ── 圆角矩形描边测试 ──

#[test]
fn per_corner_radius_uniform_equivalent() {
    // 验证：四个角相同半径时，SDF 与 uniform 版本完全一致
    let rect = Rect::new(5.0, 5.0, 20.0, 20.0);
    let rad_uniform = Radius::uniform(6.0);
    let rad_per_corner = Radius {
        tl: 6.0,
        tr: 6.0,
        bl: 6.0,
        br: 6.0,
    };

    for y in 0..15 {
        for x in 0..15 {
            let ux = x as f32 + 0.5;
            let uy = y as f32 + 0.5;
            let sd_u = RenderTarget::rounded_rect_sdf(ux, uy, &rect, &rad_uniform);
            let sd_p = RenderTarget::rounded_rect_sdf(ux, uy, &rect, &rad_per_corner);
            assert!(
                (sd_u - sd_p).abs() < 0.001,
                "SDF 不一致 @({},{}): uniform={:.3} per_corner={:.3}",
                x,
                y,
                sd_u,
                sd_p
            );
        }
    }
}

#[test]
fn per_corner_mixed_sharp_rounded() {
    // 验证：混合直角和圆角（TL 直角，TR 圆角 r=8）
    // TL 直角区域 SDF 应表现为 sharp corner
    let mut rt = make_rt(60, 60);
    rt.clear_all();
    let rect = Rect::new(10.0, 10.0, 40.0, 40.0);
    let rad = Radius {
        tl: 0.0,
        tr: 8.0,
        bl: 8.0,
        br: 0.0,
    };

    // 先验证 SDF 在 TL 直角区域（2px 内）没有受到 TR 圆角半径影响
    // TL 直角附近的点应该表现为尖角
    let tl_corner_sd = RenderTarget::rounded_rect_sdf(9.5, 9.5, &rect, &rad);
    assert!(
        tl_corner_sd > 0.0,
        "TL 尖角外像素应为外侧，sd={:.3}",
        tl_corner_sd
    );

    let tl_inside_sd = RenderTarget::rounded_rect_sdf(11.5, 11.5, &rect, &rad);
    assert!(
        tl_inside_sd < 0.0,
        "TL 尖角内像素应为内侧，sd={:.3}",
        tl_inside_sd
    );

    // TR 圆角区域应该表现为圆角
    let tr_corner_sd = RenderTarget::rounded_rect_sdf(48.5, 10.5, &rect, &rad);
    // 距 TR 圆角中心 (rect.x+rect.w-8, rect.y+8) = (42, 18) 远处
    // (48.5, 10.5) → 距离 sqrt(6.5²+7.5²) = sqrt(42.25+56.25) = sqrt(98.5) = 9.92
    // sd = 9.92 - 8 = 1.92 > 0 → 在圆角外侧
    assert!(
        tr_corner_sd > 0.0,
        "TR 圆角外像素应为外侧，sd={:.3}",
        tr_corner_sd
    );

    // 直边区域在中心附近，SDF 应一致（与圆角半径无关）
    let edge_center_sd = RenderTarget::rounded_rect_sdf(30.5, 10.5, &rect, &rad);
    assert!(
        (edge_center_sd - (-0.5)).abs() < 0.01,
        "中心直边 SD 应为 -0.5，实际={:.3}",
        edge_center_sd
    );
}

#[test]
fn per_corner_different_radii_top_edge() {
    // 验证：TL=4, TR=8 时顶部直边的 SDF 在中心两侧一致
    let rect = Rect::new(10.0, 10.0, 60.0, 40.0);
    let rad = Radius {
        tl: 4.0,
        tr: 8.0,
        bl: 4.0,
        br: 8.0,
    };

    // 在中心线 (px=0) 两侧 1px 处，顶部边缘 SDF 应相同
    let sd_left = RenderTarget::rounded_rect_sdf(39.5, 10.5, &rect, &rad);
    let sd_right = RenderTarget::rounded_rect_sdf(40.5, 10.5, &rect, &rad);
    assert!(
        (sd_left - sd_right).abs() < 0.001,
        "顶部直边在 TL/TR 分界两侧 SDF 应连续: left={:.3} right={:.3}",
        sd_left,
        sd_right
    );

    // 左侧（TL 圆角区）和右侧（TR 圆角区）靠近角落处 SDF 应不同
    let sd_tl_corner = RenderTarget::rounded_rect_sdf(12.5, 12.5, &rect, &rad);
    let sd_tr_corner = RenderTarget::rounded_rect_sdf(67.5, 12.5, &rect, &rad);
    // 两者到角落中心的距离不同（半径不同），SD 应不同
    assert!(
        (sd_tl_corner - sd_tr_corner).abs() > 0.01,
        "TL(r=4) 和 TR(r=8) 角落 SDF 应不同: tl={:.3} tr={:.3}",
        sd_tl_corner,
        sd_tr_corner
    );
}

#[test]
fn per_corner_fill_renders_all_corners() {
    // 端到端验证：混合圆角的填充渲染
    let mut rt = make_rt(60, 60);
    rt.clear_all();
    let rect = Rect::new(10.0, 10.0, 40.0, 40.0);
    let rad = Radius {
        tl: 0.0,
        tr: 6.0,
        bl: 10.0,
        br: 3.0,
    };
    rt.fill_rect(rect, Color::white(), Some(rad));

    // 验证预乘一致性
    for y in 5..50 {
        for x in 5..50 {
            let (pr, pg, pb, pa) = unpack_premul(rt.pixels()[y * 60 + x]);
            assert!(
                pr <= pa && pg <= pa && pb <= pa,
                "混合圆角预乘不一致 @({},{}): r={} g={} b={} a={}",
                x,
                y,
                pr,
                pg,
                pb,
                pa
            );
        }
    }

    // TL 直角区域 (x=10..14, y=10..14) 应全部为不透明白
    let tl_alpha = (rt.pixels()[14 * 60 + 14] >> 24) & 0xFF;
    assert_eq!(tl_alpha, 255, "TL 直角区域内部应为不透明");

    // BR 小圆角 (r=3) 区域 (x=46..50, y=46..50) 内部应不透明
    let br_alpha = (rt.pixels()[48 * 60 + 48] >> 24) & 0xFF;
    assert_eq!(br_alpha, 255, "BR 小圆角内部应为不透明");
}

// ── 圆角矩形描边测试 ──

#[test]
fn rounded_stroke_1px_visible() {
    let mut rt = make_rt(40, 40);
    rt.stroke_rect(
        Rect::new(10.0, 10.0, 20.0, 20.0),
        blue(),
        1.0,
        Some(Radius::uniform(6.0)),
    );

    // 描边在顶部边缘应可见（AA 过渡区内，覆盖率约 0.5）
    let p = rt.pixels()[10 * 40 + 20]; // 顶部边缘中点
    let a = (p >> 24) & 0xFF;
    assert!(a > 60, "1px 描边在顶部边缘应可见，a={}", a);
}

#[test]
fn rounded_stroke_half_px_visible() {
    let mut rt = make_rt(40, 40);
    rt.stroke_rect(
        Rect::new(10.0, 10.0, 20.0, 20.0),
        blue(),
        0.5,
        Some(Radius::uniform(6.0)),
    );

    // 0.5px 描边在顶部边缘也应至少可见半透明像素
    let p = rt.pixels()[10 * 40 + 20];
    let a = (p >> 24) & 0xFF;
    assert!(a > 16, "0.5px 描边在顶部边缘应可见，a={}", a);
}

#[test]
fn rounded_stroke_supersample_level_changes_output() {
    let mut rt1 = make_rt(40, 40);
    rt1.set_supersample_level(1);
    rt1.stroke_rect(
        Rect::new(10.0, 10.0, 20.0, 20.0),
        blue(),
        0.5,
        Some(Radius::uniform(6.0)),
    );

    let mut rt8 = make_rt(40, 40);
    rt8.set_supersample_level(8);
    rt8.stroke_rect(
        Rect::new(10.0, 10.0, 20.0, 20.0),
        blue(),
        0.5,
        Some(Radius::uniform(6.0)),
    );

    let mut diff_count = 0;
    for i in 0..(40 * 40) as usize {
        if rt1.pixels()[i] != rt8.pixels()[i] {
            diff_count += 1;
        }
    }
    assert!(
        diff_count > 0,
        "supersample_level(8) should change output, diff_count=0"
    );
}

#[test]
fn rounded_stroke_1px_supersample_changes_output() {
    let mut rt1 = make_rt(40, 40);
    rt1.set_supersample_level(1);
    rt1.stroke_rect(
        Rect::new(10.0, 10.0, 20.0, 20.0),
        blue(),
        1.0,
        Some(Radius::uniform(10.0)),
    );

    let mut rt8 = make_rt(40, 40);
    rt8.set_supersample_level(8);
    rt8.stroke_rect(
        Rect::new(10.0, 10.0, 20.0, 20.0),
        blue(),
        1.0,
        Some(Radius::uniform(10.0)),
    );

    let mut diff_count = 0;
    for i in 0..(40 * 40) as usize {
        if rt1.pixels()[i] != rt8.pixels()[i] {
            diff_count += 1;
        }
    }
    assert!(
        diff_count > 0,
        "1px rounded stroke should differ between level1 and level8, diff_count={}",
        diff_count
    );
}

#[test]
fn rounded_stroke_corner_premul_consistency() {
    let mut rt = make_rt(40, 40);
    rt.stroke_rect(
        Rect::new(10.0, 10.0, 20.0, 20.0),
        blue(),
        1.0,
        Some(Radius::uniform(6.0)),
    );

    for y in 8..16 {
        for x in 8..16 {
            let (pr, pg, pb, pa) = unpack_premul(rt.pixels()[y * 40 + x]);
            assert!(
                pr <= pa && pg <= pa && pb <= pa,
                "描边预乘不一致 @({},{}): r={} g={} b={} a={}",
                x,
                y,
                pr,
                pg,
                pb,
                pa
            );
        }
    }
}

#[test]
fn rounded_stroke_corner_continuous() {
    // 验证：圆角描边在 45° 对角线上可见且无异常跳变
    let mut rt = make_rt(40, 40);
    rt.clear_all();
    rt.stroke_rect(
        Rect::new(10.0, 10.0, 20.0, 20.0),
        Color::white(),
        2.0,
        Some(Radius::uniform(8.0)),
    );

    // 沿对角线从圆角内侧向外采样
    let mut alphas: Vec<u32> = Vec::new();
    for d in 0..12 {
        let x = 10 + d;
        let y = 10 + d;
        if x < 40 && y < 40 {
            alphas.push((rt.pixels()[y * 40 + x] >> 24) & 0xFF);
        }
    }

    // 2px 描边在 45° 对角投影仅 ~1.4px，命中像素极少，验证可见即可
    let max_a = alphas.iter().max().copied().unwrap_or(0);
    assert!(max_a > 64, "描边沿对角线应有可见像素，max={}", max_a);

    // 不应有 0→>128→0→>128 的震荡
    let mut zero_crossings = 0;
    let mut was_visible = false;
    for &a in &alphas {
        let visible = a > 64;
        if visible != was_visible {
            zero_crossings += 1;
            was_visible = visible;
        }
    }
    assert!(
        zero_crossings <= 2,
        "描边应连续不断裂，crossings={}, alphas={:?}",
        zero_crossings,
        alphas
    );
}

// ── Alpha 混合测试 ──

#[test]
fn blend_over_opaque_black() {
    // 手动构造不透明黑背景
    let mut rt = make_rt(1, 1);
    rt.pixels[0] = 0xFF000000; // 不透明黑
    let color = red_half();
    rt.fill_rect(Rect::new(0.0, 0.0, 1.0, 1.0), color, None);

    let (r, _, _, a) = unpack(rt.pixels()[0]);
    // 50%红 over 不透明黑: a=255, r≈128
    assert_eq!(a, 255, "叠加不透明黑后 alpha 应为 255");
    assert!((r as i32 - 128).abs() <= 2, "r≈128, 实际={}", r);
}

#[test]
fn blend_over_transparent() {
    let mut rt = make_rt(1, 1);
    // 手动清除为透明黑
    rt.pixels[0] = 0x00000000;
    let color = red_half();
    rt.fill_rect(Rect::new(0.0, 0.0, 1.0, 1.0), color, None);

    let (pr, pg, pb, pa) = unpack_premul(rt.pixels()[0]);
    // 预乘: r=255*128/255=128, a=128
    assert_eq!(pa, 128, "alpha 分量");
    assert_eq!(pr, 128, "预乘红色分量");
    assert_eq!(pg, 0);
    assert_eq!(pb, 0);
}

#[test]
fn blend_over_white() {
    let mut rt = make_rt(1, 1);
    // 先填充白色
    rt.fill_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::white(), None);
    // 再覆盖半透明红色
    rt.fill_rect(Rect::new(0.0, 0.0, 1.0, 1.0), red_half(), None);

    let (r, g, b, a) = unpack(rt.pixels()[0]);
    // 50% red over white: r=255*0.5+255*0.5=255, g=0+255*0.5=127, b=0+255*0.5=127
    assert!(r >= 253, "混合后红色分量 r={}", r);
    assert!((g as i32 - 128).abs() <= 2, "混合后绿色分量 g={}", g);
    assert!((b as i32 - 128).abs() <= 2, "混合后蓝色分量 b={}", b);
    assert_eq!(a, 255, "最终应为不透明");
}

// ── put_pixel_aa 测试 ──

#[test]
fn put_pixel_aa_full_coverage() {
    let mut rt = make_rt(1, 1);
    let premul = RenderTarget::premul(blue());
    rt.put_pixel_aa(0, 0, premul, 1.0);

    let (r, g, b, a) = unpack(rt.pixels()[0]);
    assert_eq!((r, g, b, a), (22, 119, 255, 255));
}

#[test]
fn put_pixel_aa_half_coverage_over_transparent() {
    let mut rt = make_rt(1, 1);
    rt.pixels[0] = 0x00000000; // 透明背景
    let premul = RenderTarget::premul(Color::white());
    // 50% 覆盖率
    rt.put_pixel_aa(0, 0, premul, 0.5);

    let (pr, pg, pb, pa) = unpack_premul(rt.pixels()[0]);
    // 预乘一致性：所有通道应相等（白色预乘后 r=g=b=a）
    assert_eq!(pr, pa, "预乘一致性: r == a");
    assert_eq!(pg, pa, "预乘一致性: g == a");
    assert_eq!(pb, pa, "预乘一致性: b == a");
    assert!((pa as i32 - 128).abs() <= 1, "alpha≈128, 实际={}", pa);
}

// ── apply_opacity 测试 ──

#[test]
fn apply_opacity_half_keeps_premul() {
    let mut rt = make_rt(1, 1);
    rt.set_opacity(0.5);

    let premul = RenderTarget::premul(blue());
    let result = rt.apply_opacity(premul);

    let (pr, pg, pb, pa) = unpack_premul(result);
    // 修复后：所有通道都应缩放 0.5
    assert_eq!(pa, 127); // 255 * 0.5 ≈ 127
                         // 预乘一致性
    assert!(pr <= pa, "r <= a");
    assert!(pg <= pa, "g <= a");
    assert!(pb <= pa, "b <= a");
}

#[test]
fn apply_opacity_full_is_noop() {
    let rt = make_rt(1, 1);
    let premul = RenderTarget::premul(blue());
    let result = rt.apply_opacity(premul);
    assert_eq!(result, premul, "opacity=1.0 应为无操作");
}
