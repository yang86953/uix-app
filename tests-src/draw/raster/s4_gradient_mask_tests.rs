//! S4 圆角裁剪的 CPU 像素证明：渐变掩码在真实光栅输出上采样。
//!
//! 采样点与 SDF 预期来自独立几何计算（3b049 修正轮 verify-corrections
//! 脚本留证），不复制生产实现输出。

use crate::core::Rect;
use crate::draw::Color;
use crate::draw::geometry::types::{GradientDirection, Radius};
use crate::draw::raster::software_rasterizer::SoftwareRasterizer;

// 提取像素的 (a, r, g, b) 通道；缓冲布局为 (a<<24)|(r<<16)|(g<<8)|b。
fn channels(pixel: u32) -> (u32, u32, u32, u32) {
    (
        (pixel >> 24) & 0xFF,
        (pixel >> 16) & 0xFF,
        (pixel >> 8) & 0xFF,
        pixel & 0xFF,
    )
}

fn surface(w: i32, h: i32) -> (SoftwareRasterizer, Vec<u32>) {
    (SoftwareRasterizer::new(w, h), vec![0u32; (w * h) as usize])
}

// 垂直双色渐变 + 统一圆角的角部透明与内部取色证明。
#[test]
fn vertical_gradient_uniform_corner_mask_pixels() {
    let (raster, mut pixels) = surface(100, 60);
    raster.fill_linear_gradient(
        &mut pixels,
        100,
        60,
        Rect::new(0.0, 0.0, 100.0, 60.0),
        Color::from_rgb(255, 0, 0),
        Color::from_rgb(0, 0, 255),
        GradientDirection::Vertical,
        Radius::uniform(20.0),
    );
    // 左上角外（SDF=+4.75）：完全透明。
    let (a, _, _, _) = channels(pixels[2 * 100 + 2]);
    assert_eq!(a, 0, "圆角外像素必须透明");
    // 顶部直边内侧（x∈[20,80]）：不透明。
    let (a, _, _, _) = channels(pixels[1 * 100 + 50]);
    assert_eq!(a, 255, "顶直边像素必须不透明");
    // 中心：不透明且为红蓝混合（t=30.5/60≈0.508，r≈b≈128）。
    let (a, r, _, b) = channels(pixels[30 * 100 + 50]);
    assert_eq!(a, 255, "中心像素必须不透明");
    assert!(
        (r as i32 - b as i32).abs() <= 4 && (110..=146).contains(&r),
        "中心应为红蓝各半，实际 r={r} b={b}"
    );
}

// 非均匀四角（10/20/30/40）各角独立裁剪证明。
#[test]
fn nonuniform_corner_mask_pixels() {
    let (raster, mut pixels) = surface(100, 60);
    let radii = Radius {
        tl: 10.0,
        tr: 20.0,
        br: 30.0,
        bl: 40.0,
    };
    raster.fill_linear_gradient(
        &mut pixels,
        100,
        60,
        Rect::new(0.0, 0.0, 100.0, 60.0),
        Color::from_rgb(0, 255, 0),
        Color::from_rgb(0, 255, 0),
        GradientDirection::Vertical,
        radii,
    );
    // 角外点（独立 SDF：tl 角 (1.5,1.5)=+2.02、br 角 (95.5,58.5)=+8.24）：透明。
    for (x, y) in [(1usize, 1usize), (95, 58)] {
        let (a, _, _, _) = channels(pixels[y * 100 + x]);
        assert_eq!(a, 0, "({x},{y}) 圆角外必须透明");
    }
    // 直边与角内点（左直边 (1.5,15.5) SDF=-1.5、顶直边 (11.5,1.5)=-1.5、
    // bl 角内 (38.5,58.5)=-1.5）：不透明。
    for (x, y) in [(1usize, 15usize), (11, 1), (38, 58)] {
        let (a, _, _, _) = channels(pixels[y * 100 + x]);
        assert_eq!(a, 255, "({x},{y}) 形内必须不透明");
    }
}

// 零半径保持无掩码旧行为（角部照常填充）。
#[test]
fn zero_radius_gradient_keeps_legacy_full_rect() {
    let (raster, mut pixels) = surface(100, 60);
    raster.fill_linear_gradient(
        &mut pixels,
        100,
        60,
        Rect::new(0.0, 0.0, 100.0, 60.0),
        Color::from_rgb(255, 0, 0),
        Color::from_rgb(255, 0, 0),
        GradientDirection::Vertical,
        Radius::zero(),
    );
    let (a, _, _, _) = channels(pixels[1 * 100 + 1]);
    assert_eq!(a, 255, "零半径角部必须保持旧行为");
}

// 径向渐变圆角掩码：角外透明、中心不透明。
#[test]
fn radial_gradient_corner_mask_pixels() {
    let (raster, mut pixels) = surface(100, 60);
    // 外圆半径取半宽/半高斜边，覆盖四角。
    let outer = (50.0f32).hypot(30.0);
    raster.fill_radial_gradient(
        &mut pixels,
        100,
        60,
        50.0,
        30.0,
        0.0,
        outer,
        Color::from_rgb(0, 255, 0),
        Color::from_rgb(0, 255, 0),
        Rect::new(0.0, 0.0, 100.0, 60.0),
        Radius::uniform(20.0),
    );
    // 角 (2.5,2.5) 距圆心 54.9 < outer（圆内）但 SDF=+4.75（盒角外）→ 透明。
    let (a, _, _, _) = channels(pixels[2 * 100 + 2]);
    assert_eq!(a, 0, "径向圆角外必须透明");
    let (a, _, _, _) = channels(pixels[30 * 100 + 50]);
    assert_eq!(a, 255, "径向中心必须不透明");
}

// 超大半径在填充光栅层按相邻和归一：100x60、tl=tr=80 → 50（factor 0.625）。
#[test]
fn oversized_radius_normalizes_at_raster() {
    let (raster, mut pixels) = surface(100, 60);
    raster.fill_rect(
        &mut pixels,
        100,
        60,
        Rect::new(0.0, 0.0, 100.0, 60.0),
        Color::from_rgb(255, 0, 0),
        Some(Radius {
            tl: 80.0,
            tr: 80.0,
            br: 0.0,
            bl: 0.0,
        }),
    );
    // 归一后 (50,50,0,0)：角 (0.5,0.5) 距圆心 70 > 50 → 透明。
    let (a, _, _, _) = channels(pixels[0 * 100 + 0]);
    assert_eq!(a, 0, "归一后角外必须透明（未归一时半径 80 会覆盖该点）");
    // (25.5,30.5) 距 tl 圆心 (50,50) 为 31.3 < 50 → 填充。
    let (a, _, _, _) = channels(pixels[30 * 100 + 25]);
    assert_eq!(a, 255, "归一轮廓内必须填充");
}

// 三类几何独立验证（反例修正）：tl=80 跨中线大角经 fill 消费共享 SDF。
#[test]
fn fill_tl80_crossing_midline_matches_independent_geometry() {
    let (raster, mut pixels) = surface(200, 100);
    raster.fill_rect(
        &mut pixels,
        200,
        100,
        Rect::new(0.0, 0.0, 200.0, 100.0),
        Color::from_rgb(0, 0, 255),
        Some(Radius {
            tl: 80.0,
            tr: 0.0,
            br: 0.0,
            bl: 0.0,
        }),
    );
    // 独立几何：tl 圆心 (80,80)，切线区域 [0,80]x[0,80]。
    // (15.5,20.5) 距 87.7、(2.5,55.5) 距 81.3、(2.5,49.5)/(2.5,51.5) 均
    // 距 >80（圆外且跨中线连续，无 y=50 断层）。
    for (x, y) in [
        (15usize, 20usize),
        (2, 55),
        (2, 49),
        (2, 51),
        (2, 2),
        (15, 2),
    ] {
        let (a, _, _, _) = channels(pixels[y * 200 + x]);
        assert_eq!(a, 0, "({x},{y}) 大角圆外必须透明");
    }
    // 切线右侧与底部直边保持不透明（角未越界扩张）。
    for (x, y) in [(100usize, 50usize), (2, 90), (150, 5)] {
        let (a, _, _, _) = channels(pixels[y * 200 + x]);
        assert_eq!(a, 255, "({x},{y}) 直边形内必须不透明");
    }
}

// 非均匀角 + 相邻和缩放两类几何的 fill 级回归（与既有掩码/归一用例
// 互为独立入口）。
#[test]
fn fill_nonuniform_and_sum_scaled_geometry() {
    let (raster, mut pixels) = surface(100, 60);
    raster.fill_rect(
        &mut pixels,
        100,
        60,
        Rect::new(0.0, 0.0, 100.0, 60.0),
        Color::from_rgb(0, 255, 0),
        Some(Radius {
            tl: 10.0,
            tr: 20.0,
            br: 30.0,
            bl: 40.0,
        }),
    );
    // 非均匀：角外透明/形内不透明（同 3b04 独立 SDF）。
    for (x, y, expect_transparent) in [
        (1usize, 1usize, true),
        (98, 2, true),
        (95, 58, true),
        (1, 15, false),
        (11, 1, false),
        (38, 58, false),
    ] {
        let (a, _, _, _) = channels(pixels[y * 100 + x]);
        assert_eq!(
            a,
            if expect_transparent { 0 } else { 255 },
            "({x},{y}) 非均匀角判定错误"
        );
    }
    // 相邻和缩放：tl=tr=80 于 100x60 → 50（factor 0.625）。
    let (_, mut pixels) = surface(100, 60);
    let (raster, mut pixels2) = surface(100, 60);
    drop(pixels);
    raster.fill_rect(
        &mut pixels2,
        100,
        60,
        Rect::new(0.0, 0.0, 100.0, 60.0),
        Color::from_rgb(255, 0, 0),
        Some(Radius {
            tl: 80.0,
            tr: 80.0,
            br: 0.0,
            bl: 0.0,
        }),
    );
    let (a, _, _, _) = channels(pixels2[0]);
    assert_eq!(a, 0, "归一后 (0.5,0.5) 角外必须透明");
    let (a, _, _, _) = channels(pixels2[30 * 100 + 25]);
    assert_eq!(a, 255, "归一轮廓内 (25.5,30.5) 必须填充");
}

// ═══ 对角大角切线方形重叠（本轮反例）═══
// 100x100 tl=br=80：相邻和全不超边（80≤100），对角切线方形
// [0,80]² 与 [20,100]² 重叠于 [20,80]²。独立几何：TL/BR 圆心分别为
// (80,80)/(20,20)，重叠区点须同时满足两角弧约束（交集）。
#[test]
fn fill_diagonal_tl_br_80_clips_overlap_region() {
    let (raster, mut pixels) = surface(100, 100);
    raster.fill_rect(
        &mut pixels,
        100,
        100,
        Rect::new(0.0, 0.0, 100.0, 100.0),
        Color::from_rgb(255, 0, 0),
        Some(Radius {
            tl: 80.0,
            tr: 0.0,
            br: 80.0,
            bl: 0.0,
        }),
    );
    // (21.5,21.5) 与 (78.5,78.5) 到对侧圆心均为 82.73>80 → 透明
    //（反例：78,78 曾因 TL 分支先行返回被错误保留为不透明）。
    for (x, y) in [(21usize, 21usize), (78, 78), (2, 2), (97, 97)] {
        let (a, _, _, _) = channels(pixels[y * 100 + x]);
        assert_eq!(a, 0, "({x},{y}) 对角重叠区圆外必须透明");
    }
    // 中心同时满足两角约束（各距 42.43<80）→ 不透明。
    let (a, _, _, _) = channels(pixels[50 * 100 + 50]);
    assert_eq!(a, 255, "中心必须不透明");
}

// 180 度旋转对称性质：tl==br 时整幅 alpha 必须点对称（独立性质，
// 不复述实现条件）。
#[test]
fn fill_diagonal_tl_br_is_180_degree_symmetric() {
    let (raster, mut pixels) = surface(100, 100);
    raster.fill_rect(
        &mut pixels,
        100,
        100,
        Rect::new(0.0, 0.0, 100.0, 100.0),
        Color::from_rgb(0, 0, 255),
        Some(Radius {
            tl: 80.0,
            tr: 0.0,
            br: 80.0,
            bl: 0.0,
        }),
    );
    for y in (1..99).step_by(3) {
        for x in (1..99).step_by(3) {
            let (a, _, _, _) = channels(pixels[y * 100 + x]);
            let (b, _, _, _) = channels(pixels[(99 - y) * 100 + (99 - x)]);
            assert_eq!(a, b, "({x},{y}) 与其 180 度对称点的 alpha 必须相等");
        }
    }
}

// 另一对角 tr=bl=80 与非正方形比例 100x80 tl=br=70（对角方形重叠的
// 两个独立变体）。
#[test]
fn fill_other_diagonal_and_nonsquare_ratio() {
    // tr=bl=80 于 100x100：圆心 (20,80)/(80,20)。
    let (raster, mut pixels) = surface(100, 100);
    raster.fill_rect(
        &mut pixels,
        100,
        100,
        Rect::new(0.0, 0.0, 100.0, 100.0),
        Color::from_rgb(0, 255, 0),
        Some(Radius {
            tl: 0.0,
            tr: 80.0,
            br: 0.0,
            bl: 80.0,
        }),
    );
    // (78.5,21.5) 距 BL 圆心 (20,80) 为 82.85>80 → 透明；对称点同。
    for (x, y) in [(78usize, 21usize), (21, 78)] {
        let (a, _, _, _) = channels(pixels[y * 100 + x]);
        assert_eq!(a, 0, "tr/bl 对角 ({x},{y}) 圆外必须透明");
    }
    let (a, _, _, _) = channels(pixels[50 * 100 + 50]);
    assert_eq!(a, 255);
    // 100x80 tl=br=70：TL 圆心 (70,70)、BR 圆心 (30,10)；重叠区
    // [30,70]x[10,70]。(30.5,10.5) 距 TL 圆心 71.42>70（SDF +1.42，
    // 超出 0.5px 抗锯齿带）→ 透明；其 180 度对称点 (69.5,69.5) 距 BR
    // 圆心同为 71.42 → 透明。
    let (raster, mut pixels) = surface(100, 80);
    raster.fill_rect(
        &mut pixels,
        100,
        80,
        Rect::new(0.0, 0.0, 100.0, 80.0),
        Color::from_rgb(255, 0, 255),
        Some(Radius {
            tl: 70.0,
            tr: 0.0,
            br: 70.0,
            bl: 0.0,
        }),
    );
    for (x, y) in [(30usize, 10usize), (69, 69)] {
        let (a, _, _, _) = channels(pixels[y * 100 + x]);
        assert_eq!(a, 0, "非正方形对角 ({x},{y}) 圆外必须透明");
    }
    let (a, _, _, _) = channels(pixels[40 * 100 + 50]);
    assert_eq!(a, 255, "中心必须不透明");
}

// 渐变掩码共享同一 SDF：对角重叠反例在渐变路径同样成立。
#[test]
fn gradient_diagonal_tl_br_80_clips_overlap_region() {
    let (raster, mut pixels) = surface(100, 100);
    raster.fill_linear_gradient(
        &mut pixels,
        100,
        100,
        Rect::new(0.0, 0.0, 100.0, 100.0),
        Color::from_rgb(255, 0, 0),
        Color::from_rgb(255, 0, 0),
        GradientDirection::Vertical,
        Radius {
            tl: 80.0,
            tr: 0.0,
            br: 80.0,
            bl: 0.0,
        },
    );
    for (x, y) in [(21usize, 21usize), (78, 78)] {
        let (a, _, _, _) = channels(pixels[y * 100 + x]);
        assert_eq!(a, 0, "渐变对角圆外 ({x},{y}) 必须透明");
    }
    let (a, _, _, _) = channels(pixels[50 * 100 + 50]);
    assert_eq!(a, 255);
}
