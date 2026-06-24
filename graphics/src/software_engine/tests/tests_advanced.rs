use super::{blue, make_rt, unpack_premul};
use crate::engine::GraphicsEngine;
use crate::{Color, Radius};
use uix_core::Rect;

// ── 综合：圆角矩形 fill+stroke 叠加 ──

struct TestSystemInfo;
impl uix_platform::ISystemInfo for TestSystemInfo {
    fn os_info(&self) -> uix_platform::types::OsInfo { uix_platform::types::OsInfo { name: "test".into(), version: "1.0".into(), build: "0".into(), is_64bit: true } }
    fn cpu_count(&self) -> u32 { 1 }
    fn memory_info(&self) -> uix_platform::types::MemoryInfo { uix_platform::types::MemoryInfo { total_bytes: 0, available_bytes: 0, process_working_set: 0, process_private_bytes: 0 } }
    fn hostname(&self) -> String { "test".into() }
    fn username(&self) -> String { "test".into() }
    fn up_time(&self) -> u64 { 0 }
    fn default_font_path(&self) -> Option<String> { None }
}

#[test]
fn diag_pixel_format() {
    use crate::SoftwareEngine;
    let mut engine = SoftwareEngine::new();
    let test_si = TestSystemInfo;
    engine.initialize(4, 1, &test_si).unwrap_or(());
    let dirty = crate::DirtyRegion::full();

    engine.begin_frame(&dirty);
    engine.fill_rect(
        Rect::new(0.0, 0.0, 4.0, 1.0),
        Color::from_rgb(255, 0, 0),
        None,
    );
    engine.end_frame(&dirty);
    let p = engine.pixels()[0];
    eprintln!(
        "RED  pixel=0x{:08X} bytes=[{:02X},{:02X},{:02X},{:02X}]",
        p,
        (p & 0xFF) as u8,
        ((p >> 8) & 0xFF) as u8,
        ((p >> 16) & 0xFF) as u8,
        ((p >> 24) & 0xFF) as u8
    );
    eprintln!("  GDI expects BGRA. If bytes=[FF,00,00,FF], GDI reads as BLUE (R/B swap!)");

    engine.begin_frame(&dirty);
    engine.fill_rect(
        Rect::new(0.0, 0.0, 4.0, 1.0),
        Color::from_rgb(0, 0, 255),
        None,
    );
    engine.end_frame(&dirty);
    let p = engine.pixels()[0];
    eprintln!(
        "BLUE pixel=0x{:08X} bytes=[{:02X},{:02X},{:02X},{:02X}]",
        p,
        (p & 0xFF) as u8,
        ((p >> 8) & 0xFF) as u8,
        ((p >> 16) & 0xFF) as u8,
        ((p >> 24) & 0xFF) as u8
    );
    eprintln!("  GDI expects BGRA. If bytes=[00,00,FF,FF], GDI reads as RED (R/B swap!)");
}

#[test]
fn diag_bottom_up() {
    // 从 GDI 呈现角度检查：渲染后写入 BMP 文件可直接查看
    let mut rt = make_rt(200, 100);
    rt.clear_all();
    // 背景
    rt.fill_rect(
        Rect::new(0.0, 0.0, 200.0, 100.0),
        Color::from_rgb(18, 18, 24),
        None,
    );
    // 蓝色圆角矩形
    rt.fill_rect(
        Rect::new(30.0, 20.0, 140.0, 60.0),
        Color::from_rgb(22, 119, 255),
        Some(Radius::uniform(12.0)),
    );

    // 逐像素检查：alpha 是否全部为 255
    let mut min_a = 255u32;
    for y in 0..100 {
        for x in 0..200 {
            let a = (rt.pixels()[y * 200 + x] >> 24) & 0xFF;
            if a < min_a {
                min_a = a;
            }
        }
    }
    eprintln!("最小 alpha: {} (应为 255)", min_a);

    // 输出一行像素的 RGB 值
    eprintln!("y=20 行 (圆角上缘) 的 B,G,R 值:");
    for x in (25..45).step_by(2) {
        let p = rt.pixels()[20 * 200 + x];
        let b = p & 0xFF;
        let g = (p >> 8) & 0xFF;
        let r = (p >> 16) & 0xFF;
        eprint!("({:3},{:3},{:3}) ", r, g, b);
    }
    eprintln!();

    // 对比：内部 vs 直边 vs 圆角边缘
    let p_inner = rt.pixels()[50 * 200 + 100]; // 内部
    let p_edge = rt.pixels()[20 * 200 + 100]; // 直边
    let p_corner = rt.pixels()[20 * 200 + 35]; // 圆角
    eprintln!(
        "内部(100,50): R={} G={} B={}",
        (p_inner >> 16) & 0xFF,
        (p_inner >> 8) & 0xFF,
        p_inner & 0xFF
    );
    eprintln!(
        "直边(100,20): R={} G={} B={}",
        (p_edge >> 16) & 0xFF,
        (p_edge >> 8) & 0xFF,
        p_edge & 0xFF
    );
    eprintln!(
        "圆角(35,20):  R={} G={} B={}",
        (p_corner >> 16) & 0xFF,
        (p_corner >> 8) & 0xFF,
        p_corner & 0xFF
    );
}

#[test]
fn rounded_fill_stroke_overlay_premul_consistency() {
    // 模拟按钮渲染：先 fill 再 stroke 同色
    let mut rt = make_rt(40, 40);
    let color = blue();
    let radius = Some(Radius::uniform(6.0));
    let rect = Rect::new(10.0, 10.0, 20.0, 20.0);

    rt.fill_rect(rect, color, radius);
    rt.stroke_rect(rect, color, 1.0, radius);

    // 扫描整个渲染区域，确保预乘一致性
    for y in 5..30 {
        for x in 5..30 {
            let (pr, pg, pb, pa) = unpack_premul(rt.pixels()[y * 40 + x]);
            assert!(
                pr <= pa && pg <= pa && pb <= pa,
                "fill+stroke 预乘不一致 @({},{}): r={} g={} b={} a={}",
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

// ── 尖角描边：45° miter AA 正确性 ──

#[test]
fn sharp_stroke_miter_1px_transition() {
    // 验证尖角描边在 45° 拐角处 AA 过渡区为 1px（不因切比雪夫展宽到 ~1.4px）
    let mut rt = make_rt(20, 20);
    // lw=2 → h=1。矩形 (4,4,12,12)，角在 (4,4)
    // miter: 外距角 1px，内距角 1px
    rt.stroke_rect(
        Rect::new(4.0, 4.0, 12.0, 12.0),
        Color::from_rgb(255, 255, 255),
        2.0,
        None,
    );

    // 左上角 miter 沿 45° 采样：从 (3,3)→(4,4)→(5,5)→(6,6)
    // 注意 ux=px+0.5
    let px = |x: i32, y: i32| rt.pixels()[y as usize * 20 + x as usize];
    let alpha = |x: i32, y: i32| (px(x, y) >> 24) & 0xFF;

    // (3,3): ux=3.5 → d_left=0.5, coverage=1.0（在 miter 内）
    let a33 = alpha(3, 3);
    assert!(a33 > 200, "(3,3) 应在 miter 内: a={}", a33);

    // (4,4): ux=4.5 → d_left=0.5, coverage=1.0（在角点）
    let a44 = alpha(4, 4);
    assert!(a44 > 200, "(4,4) 应在 miter 内: a={}", a44);

    // (5,5): ux=5.5 → d_left=1.5 → stroke_sd=0.5 → coverage ~0.0（刚好在 miter 外）
    let a55 = alpha(5, 5);
    assert!(a55 < 100, "(5,5) 应在 miter 外: a={}", a55);

    // (6,6): ux=6.5 → d_left=2.5 → coverage=0
    let a66 = alpha(6, 6);
    assert_eq!(a66, 0, "(6,6) 应透明: a={}", a66);

    // (2,2): ux=2.5 → d_left=1.5 → coverage=0（在 miter 边缘外）
    let a22 = alpha(2, 2);
    assert!(a22 < 100, "(2,2) 应在 miter 外: a={}", a22);
}

// ── 圆角描边：45° AA 过渡宽度 1px（轴向投影） ──

#[test]
fn rounded_stroke_aa_transition_width() {
    // 验证圆角描边在 45° 对角线上 AA 过渡宽度的轴向投影 ≈1px。
    // 角度自适应 AA 补偿使过渡区在 45° 方向加宽，
    // 轴向投影 AA 宽度 ≥1px，与直线边一致。
    let mut rt = make_rt(20, 20);
    // rect(2,2,16,16), r=6, lw=2 → 左上角弧中心在 (8,8)
    rt.stroke_rect(
        Rect::new(2.0, 2.0, 16.0, 16.0),
        Color::from_rgb(255, 255, 255),
        2.0,
        Some(Radius::uniform(6.0)),
    );
    let px = |x: i32, y: i32| rt.pixels()[y as usize * 20 + x as usize];
    let alpha = |x: i32, y: i32| (px(x, y) >> 24) & 0xFF;

    // 45° 对角线采样 px 从 2 到 10，弧中心 (8,8)
    let vals: Vec<(i32, u32)> = (2..=10).map(|i| (i, alpha(i, i))).collect();
    for &(i, a) in &vals {
        eprintln!("  diag({},{}): alpha={}", i, i, a);
    }

    let a33 = alpha(3, 3);
    let a44 = alpha(4, 4);
    let a55 = alpha(5, 5);

    eprintln!("  a33={} a44={} a55={}", a33, a44, a55);
    // 角度自适应 AA 使过渡区加宽：
    // (3,3) 在描边外缘过渡区附近，覆盖应较高但可能不满
    assert!(a33 > 200, "(3,3) 应为高覆盖: a={}", a33);
    // (4,4) 在描边内缘过渡区
    assert!(a44 > 30 && a44 < 220, "(4,4) 应在过渡区: a={}", a44);
    // (5,5) 在描边内侧——透明
    assert_eq!(a55, 0, "(5,5) 应透明: a={}", a55);
}

// ── 圆角描边：圆角与直线边描边宽度一致性 ──

#[test]
fn rounded_stroke_corner_width_consistency() {
    // 验证：圆角处的描边像素覆盖宽度与直线边一致，
    // 不再出现圆角比直线边细的问题。
    //
    // 方法：在直线边和 45° 圆角方向分别统计描边的像素跨度，
    // 圆角处的径向覆盖宽度应与直线边法线方向的覆盖宽度相同。
    let mut rt = make_rt(60, 60);
    let lw = 4.0;
    rt.stroke_rect(
        Rect::new(10.0, 10.0, 40.0, 40.0),
        Color::from_rgb(255, 255, 255),
        lw,
        Some(Radius::uniform(10.0)),
    );

    let px = |x: i32, y: i32| rt.pixels()[y as usize * 60 + x as usize];
    let alpha = |x: i32, y: i32| (px(x, y) >> 24) & 0xFF;

    // ── 直线边（顶部）的描边像素宽度 ──
    // 顶部边在 y=10 附近，4px 描边覆盖 y=8..11（中心在 y=10，±h=2）
    // 只统计上边附近的像素（y < 20 = tl_cy）
    let top_stroke_width = (0..20).filter(|&y| alpha(30, y) > 0).count();

    // ── 圆角（左上角 45° 方向）的描边像素宽度 ──
    // 左上角弧心在 (20, 20)，45° 方向即从 (20,20) 向左上
    // 沿 45° 对角线统计有覆盖的像素，然后换算为径向像素宽度
    let corner_diag_width = (-20..=20)
        .filter(|&d| {
            let x = 20 - d;
            let y = 20 - d;
            (0..60).contains(&x) && (0..60).contains(&y) && alpha(x, y) > 0
        })
        .count();

    // 对角线上的像素间距为 √2，所以径向等效宽度 = 对角线像素数 / √2
    let corner_radial_width = corner_diag_width as f32 / std::f32::consts::SQRT_2;

    eprintln!(
        "top_stroke_width={}, corner_diag_width={}, corner_radial_width={:.1}",
        top_stroke_width, corner_diag_width, corner_radial_width
    );

    // 圆角径向宽度应与直线边宽度相近（允许 1px 容差，因 AA 过渡的像素边界可能不对齐）
    let diff = (corner_radial_width - top_stroke_width as f32).abs();
    assert!(
        diff <= 1.5,
        "圆角径向宽度({:.1})应与直线边宽度({})一致，差值={:.1}",
        corner_radial_width,
        top_stroke_width,
        diff
    );
}
