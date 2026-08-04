//! 验证共享路径细分器的填充规则、复杂拓扑、描边和同机性能基线。

// 引入优化器黑盒，保证人工性能基线实际执行每轮细分。
use std::hint::black_box;
// 引入单调时钟，记录同一测试入口的总耗时。
use std::time::Instant;

// 引入公开路径类型，按真实使用方入口构造轮廓。
use uix::draw::path::{FillRule, Path, PathBuilder};
// 引入公开描边选项，覆盖闭合路径产生的内外轮廓。
use uix::draw::stroker::StrokeOptions;
// 引入公开细分入口，避免测试绕过使用方调用面。
use uix::draw::tessellator::{tessellate_fill, tessellate_stroke};

// 把一个非空点环追加到现有路径构造器。
fn append_ring(builder: &mut PathBuilder, points: &[(f32, f32)]) {
    // 读取首点与剩余点，空环不产生路径段。
    let Some((&(first_x, first_y), rest)) = points.split_first() else {
        // 空环直接返回，保持辅助函数可复用于动态轮廓。
        return;
    };
    // 以首点开始新的子路径。
    builder.move_to(first_x, first_y);
    // 依次追加剩余折线顶点。
    for &(x, y) in rest {
        // 把当前顶点连接到轮廓中。
        builder.line_to(x, y);
    }
    // 闭合轮廓，让填充与描边共享完整边界。
    builder.close();
}

// 按指定方向追加轴对齐矩形轮廓。
fn append_rect(builder: &mut PathBuilder, x0: f32, y0: f32, x1: f32, y1: f32, reverse: bool) {
    // 根据 reverse 选择相反的有向边顺序。
    let points = if reverse {
        // 生成负有向面积的矩形点序。
        [(x0, y0), (x0, y1), (x1, y1), (x1, y0)]
    } else {
        // 生成正有向面积的矩形点序。
        [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
    };
    // 把矩形作为独立闭合子路径追加。
    append_ring(builder, &points);
}

// 从一组矩形描述构造多轮廓路径。
fn rect_path(rects: &[(f32, f32, f32, f32, bool)]) -> Path {
    // 创建空路径构造器。
    let mut builder = PathBuilder::new();
    // 按声明顺序追加每个轮廓。
    for &(x0, y0, x1, y1, reverse) in rects {
        // 保留当前轮廓的方向信息。
        append_rect(&mut builder, x0, y0, x1, y1, reverse);
    }
    // 冻结为使用方可提交的不可变路径。
    builder.build()
}

// 计算三角形列表的无符号总面积。
fn triangle_area(triangles: &[f32]) -> f64 {
    // 初始化面积累加器。
    let mut area = 0.0f64;
    // 每六个坐标读取一个三角形。
    for triangle in triangles.chunks_exact(6) {
        // 读取第一个顶点。
        let (ax, ay) = (triangle[0] as f64, triangle[1] as f64);
        // 读取第二个顶点。
        let (bx, by) = (triangle[2] as f64, triangle[3] as f64);
        // 读取第三个顶点。
        let (cx, cy) = (triangle[4] as f64, triangle[5] as f64);
        // 累加叉积绝对值的一半。
        area += ((bx - ax) * (cy - ay) - (by - ay) * (cx - ax)).abs() * 0.5;
    }
    // 返回稳定的双精度面积。
    area
}

// 计算点相对有向边的叉积符号。
fn edge_sign(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    // 返回二维叉积，供三条边统一判断。
    (px - bx) * (ay - by) - (ax - bx) * (py - by)
}

// 判断点是否位于一个三角形内部或边界上。
fn triangle_contains(triangle: &[f32], x: f32, y: f32) -> bool {
    // 计算点相对第一条边的位置。
    let first = edge_sign(x, y, triangle[0], triangle[1], triangle[2], triangle[3]);
    // 计算点相对第二条边的位置。
    let second = edge_sign(x, y, triangle[2], triangle[3], triangle[4], triangle[5]);
    // 计算点相对第三条边的位置。
    let third = edge_sign(x, y, triangle[4], triangle[5], triangle[0], triangle[1]);
    // 记录是否存在负侧结果。
    let has_negative = first < 0.0 || second < 0.0 || third < 0.0;
    // 记录是否存在正侧结果。
    let has_positive = first > 0.0 || second > 0.0 || third > 0.0;
    // 三条边不同时跨越正负侧即位于三角形内。
    !(has_negative && has_positive)
}

// 判断三角形网格是否覆盖给定采样点。
fn mesh_contains(triangles: &[f32], x: f32, y: f32) -> bool {
    // 任一三角形命中即可证明该点被网格覆盖。
    triangles
        .chunks_exact(6)
        .any(|triangle| triangle_contains(triangle, x, y))
}

// 用稳定容差核对网格总面积。
fn assert_area(triangles: &[f32], expected: f64) {
    // 计算实际无符号面积。
    let actual = triangle_area(triangles);
    // 计算与预期值的绝对偏差。
    let delta = (actual - expected).abs();
    // 面积偏差必须落在细分器现有浮点容差内。
    assert!(
        delta <= 1e-3,
        "triangle area mismatch: expected {expected}, actual {actual}, delta {delta}"
    );
}

// 验证同向与反向内轮廓分别遵守 EvenOdd 和 NonZero 语义。
#[test]
fn strict_hole_respects_fill_rule_and_direction() {
    // 构造同向的外框与内框。
    let same_direction = rect_path(&[
        // 声明正向外框。
        (0.0, 0.0, 20.0, 20.0, false),
        // 声明同向内框。
        (5.0, 5.0, 15.0, 15.0, false),
    ]);
    // 以奇偶规则细分同向轮廓。
    let even_odd = tessellate_fill(&same_direction, FillRule::EvenOdd)
        .expect("EvenOdd strict hole must tessellate");
    // 奇偶规则必须扣除内框面积。
    assert_area(&even_odd, 300.0);
    // 外框实心区域必须保持覆盖。
    assert!(mesh_contains(&even_odd, 2.0, 2.0));
    // 内框中心必须保持为空洞。
    assert!(!mesh_contains(&even_odd, 10.0, 10.0));

    // 以非零规则细分同向轮廓。
    let non_zero_same = tessellate_fill(&same_direction, FillRule::NonZero)
        .expect("same-direction NonZero contours must tessellate");
    // 同向环绕必须填满整个外框。
    assert_area(&non_zero_same, 400.0);
    // 同向内框中心仍应被填充。
    assert!(mesh_contains(&non_zero_same, 10.0, 10.0));

    // 构造方向相反的外框与内框。
    let opposite_direction = rect_path(&[
        // 声明正向外框。
        (0.0, 0.0, 20.0, 20.0, false),
        // 声明反向内框。
        (5.0, 5.0, 15.0, 15.0, true),
    ]);
    // 以非零规则细分反向内轮廓。
    let non_zero_hole = tessellate_fill(&opposite_direction, FillRule::NonZero)
        .expect("opposite-direction NonZero hole must tessellate");
    // 反向环绕必须扣除内框面积。
    assert_area(&non_zero_hole, 300.0);
    // 反向内框中心必须保持为空洞。
    assert!(!mesh_contains(&non_zero_hole, 10.0, 10.0));
}

// 验证三层严格包含树会恢复洞中的岛。
#[test]
fn nested_island_restores_filled_region() {
    // 构造外框、反向洞和同向岛。
    let path = rect_path(&[
        // 声明正向外框。
        (0.0, 0.0, 20.0, 20.0, false),
        // 声明反向洞。
        (4.0, 4.0, 16.0, 16.0, true),
        // 声明洞中的同向岛。
        (8.0, 8.0, 12.0, 12.0, false),
    ]);
    // 以非零规则细分三层包含树。
    let triangles =
        tessellate_fill(&path, FillRule::NonZero).expect("nested NonZero island must tessellate");
    // 总面积应为外框减洞再加岛。
    assert_area(&triangles, 272.0);
    // 外框边缘区域必须填充。
    assert!(mesh_contains(&triangles, 2.0, 2.0));
    // 洞区域必须保持透明。
    assert!(!mesh_contains(&triangles, 6.0, 6.0));
    // 洞中的岛必须恢复填充。
    assert!(mesh_contains(&triangles, 10.0, 10.0));
}

// 验证相交轮廓在两种填充规则下使用不同的重叠语义。
#[test]
fn intersecting_contours_follow_fill_rules() {
    // 构造两个同向且水平重叠的矩形。
    let path = rect_path(&[
        // 声明左侧矩形。
        (0.0, 0.0, 10.0, 10.0, false),
        // 声明右侧重叠矩形。
        (5.0, 0.0, 15.0, 10.0, false),
    ]);
    // 以非零规则细分相交轮廓。
    let non_zero = tessellate_fill(&path, FillRule::NonZero)
        .expect("intersecting NonZero contours must tessellate");
    // 非零规则应产生两个矩形的并集面积。
    assert_area(&non_zero, 150.0);
    // 重叠区域在非零规则下必须填充。
    assert!(mesh_contains(&non_zero, 7.5, 5.0));

    // 以奇偶规则细分相交轮廓。
    let even_odd = tessellate_fill(&path, FillRule::EvenOdd)
        .expect("intersecting EvenOdd contours must tessellate");
    // 奇偶规则应产生两个矩形的对称差面积。
    assert_area(&even_odd, 100.0);
    // 重叠区域在奇偶规则下必须被扣除。
    assert!(!mesh_contains(&even_odd, 7.5, 5.0));
    // 左侧非重叠区域必须保留。
    assert!(mesh_contains(&even_odd, 2.0, 5.0));
    // 右侧非重叠区域必须保留。
    assert!(mesh_contains(&even_odd, 13.0, 5.0));
}

// 验证接触轮廓与自交轮廓都能进入复杂路径分解。
#[test]
fn touching_and_self_intersecting_contours_tessellate() {
    // 构造共享一条边的两个矩形。
    let touching = rect_path(&[
        // 声明左侧矩形。
        (0.0, 0.0, 10.0, 10.0, false),
        // 声明与其接边的右侧矩形。
        (10.0, 0.0, 20.0, 10.0, false),
    ]);
    // 细分接触轮廓。
    let touching_triangles =
        tessellate_fill(&touching, FillRule::NonZero).expect("touching contours must tessellate");
    // 接边不应造成面积重叠或缺口。
    assert_area(&touching_triangles, 200.0);
    // 左侧矩形内部必须覆盖。
    assert!(mesh_contains(&touching_triangles, 5.0, 5.0));
    // 右侧矩形内部必须覆盖。
    assert!(mesh_contains(&touching_triangles, 15.0, 5.0));

    // 创建自交蝴蝶结路径。
    let mut builder = PathBuilder::new();
    // 从左上角开始。
    builder.move_to(0.0, 0.0);
    // 连接到右下角形成第一条对角线。
    builder.line_to(10.0, 10.0);
    // 连接到左下角。
    builder.line_to(0.0, 10.0);
    // 连接到右上角形成第二条对角线。
    builder.line_to(10.0, 0.0);
    // 闭合自交轮廓。
    builder.close();
    // 冻结自交路径。
    let bow_tie = builder.build();
    // 以奇偶规则细分自交路径。
    let bow_tie_triangles = tessellate_fill(&bow_tie, FillRule::EvenOdd)
        .expect("self-intersecting contour must tessellate");
    // 两个三角叶片的总面积必须稳定。
    assert_area(&bow_tie_triangles, 50.0);
    // 上方叶片内部必须覆盖。
    assert!(mesh_contains(&bow_tie_triangles, 5.0, 2.0));
    // 下方叶片内部必须覆盖。
    assert!(mesh_contains(&bow_tie_triangles, 5.0, 8.0));
    // 蝴蝶结左右空白区域不得误填。
    assert!(!mesh_contains(&bow_tie_triangles, 1.0, 5.0));
}

// 验证闭合描边通过内外轮廓保留空心中心。
#[test]
fn closed_stroke_preserves_hollow_center() {
    // 构造单个闭合矩形路径。
    let path = rect_path(&[(0.0, 0.0, 20.0, 20.0, false)]);
    // 设置两像素宽的默认斜接描边。
    let options = StrokeOptions {
        // 指定描边宽度。
        width: 2.0,
        // 继承默认端点、连接和斜接限制。
        ..StrokeOptions::default()
    };
    // 细分闭合描边产生的内外轮廓。
    let triangles =
        tessellate_stroke(&path, &options).expect("closed rectangular stroke must tessellate");
    // 两像素矩形描边面积应保持为 160。
    assert_area(&triangles, 160.0);
    // 描边中线附近必须被覆盖。
    assert!(mesh_contains(&triangles, 0.0, 10.0));
    // 矩形中心必须保持为空心。
    assert!(!mesh_contains(&triangles, 10.0, 10.0));
    // 远离描边的外部点不得被覆盖。
    assert!(!mesh_contains(&triangles, 30.0, 30.0));
}

// 生成指定方向的等分圆环点列。
fn radial_ring(radius: f32, segments: usize, reverse: bool) -> Vec<(f32, f32)> {
    // 为全部顶点预分配精确容量。
    let mut points = Vec::with_capacity(segments);
    // 逐角度生成圆周顶点。
    for index in 0..segments {
        // 反向轮廓按递减相位遍历。
        let phase = if reverse { segments - index } else { index };
        // 把离散相位转换为弧度。
        let angle = phase as f32 * std::f32::consts::TAU / segments as f32;
        // 追加以原点为圆心的顶点。
        points.push((radius * angle.cos(), radius * angle.sin()));
    }
    // 返回完整点列。
    points
}

// 构造固定 128 段的严格圆环路径。
fn profiled_hole_path() -> Path {
    // 创建空路径构造器。
    let mut builder = PathBuilder::new();
    // 生成正向外圆轮廓。
    let outer = radial_ring(100.0, 128, false);
    // 生成反向内圆轮廓。
    let inner = radial_ring(50.0, 128, true);
    // 追加外圆轮廓。
    append_ring(&mut builder, &outer);
    // 追加内圆轮廓。
    append_ring(&mut builder, &inner);
    // 冻结为性能基线路径。
    builder.build()
}

// 提供显式运行的同机性能基线，不把时钟波动纳入默认门禁。
#[test]
// 默认忽略，只有性能审计命令会执行。
#[ignore = "同机手工性能基线"]
fn profile_strict_hole_tessellation() {
    // 固定迭代次数，便于变更前后同口径比较。
    const ITERATIONS: usize = 200;
    // 构造固定顶点数的含洞严格轮廓。
    let path = profiled_hole_path();
    // 预热一次分配与代码路径。
    let warmup = tessellate_fill(black_box(&path), FillRule::NonZero)
        .expect("profiled strict hole must tessellate");
    // 防止预热结果被优化器丢弃。
    black_box(&warmup);
    // 记录正式循环开始时刻。
    let started = Instant::now();
    // 累加坐标数，形成可观察的数据依赖。
    let mut coordinate_count = 0usize;
    // 重复执行同一公开细分入口。
    for _ in 0..ITERATIONS {
        // 执行一次固定圆环细分。
        let triangles = tessellate_fill(black_box(&path), FillRule::NonZero)
            .expect("profiled strict hole must keep tessellating");
        // 累加结果规模，防止整轮调用被删除。
        coordinate_count = coordinate_count.saturating_add(triangles.len());
        // 把结果暴露给优化器黑盒。
        black_box(&triangles);
    }
    // 读取总耗时。
    let elapsed = started.elapsed();
    // 输出稳定字段供同机变更前后比较。
    eprintln!(
        "tessellator_profile iterations={ITERATIONS} elapsed_ns={} coordinates={coordinate_count}",
        elapsed.as_nanos()
    );
    // 结果规模必须非零，保证基线执行了真实几何工作。
    assert!(coordinate_count > 0);
}
