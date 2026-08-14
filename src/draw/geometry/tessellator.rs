//! 保守网格化（tessellation）：为 GPU 原生的路径填充 / 描边生成三角形列表（#169）。
//!
//! - **填充**：严格轮廓森林走紧凑 ear-clip / `earcut` 快速路径；相交、相触与
//!   自相交轮廓走 fill-rule 感知的连续 y 带分解，切成梯形三角形。
//! - **描边**：先构建共享的 cap/join 感知描边轮廓，再经由相同的严格 / 复杂
//!   路径流程对结果填充区域做网格化。
//!
//! 非有限输入、资源预算溢出或数值歧义的网格化返回 `None`，供调用方软回退。

use super::flattener;
use super::path::{FillRule, Path, PathSegment};
use super::stroker::{self, StrokeOptions};
use crate::core::Point;

/// 尝试网格化的单个环最大顶点数（限制 GPU 上传规模）。
const MAX_RING_VERTS: usize = 512;
/// 全部环累计扁平化顶点上限，超过后拓扑守卫触发回退。
const MAX_PATH_VERTS: usize = 2048;
/// 复杂路径分解中唯一顶点 / 交点 y 事件的上限。
const MAX_COMPLEX_EVENTS: usize = 8192;
/// 顶点对、边带扫描与交点排序的累计工作量上限。
const MAX_COMPLEX_WORK: usize = 4_194_304;
/// 复杂路径分解最多产出的三角形数。
pub(crate) const MAX_COMPLEX_TRIANGLES: usize = 65_536;

/// 把路径网格化为 xy 坐标对三角形列表（`[x0,y0, x1,y1, …]`）。
///
/// 原生约束：
/// - 恒等调用方（变换在上游应用）
/// - 有界的扁平化输出与网格化工作量
/// - 任意轮廓顺序 / 方向、相交、相触与自交叉
/// - 每个 y 带内精确的 `EvenOdd` 奇偶与 `NonZero` 累计绕数状态
pub fn tessellate_fill(path: &Path, fill_rule: FillRule) -> Option<Vec<f32>> {
    if !path_is_finite(path) {
        // 非有限坐标无法可靠网格化，返回 None 让调用方软回退。
        return None;
    }
    let polys = flattener::flatten(path.segments(), 0.25);
    if polys.is_empty() {
        // 空路径视为空三角形列表。
        return Some(Vec::new());
    }
    let mut rings = Vec::with_capacity(polys.len());
    let mut path_vertices = 0usize;
    for poly in &polys {
        // 逐环清理：剔除退化环，并累计全部顶点数。
        let Some(ring) = clean_ring(poly).ok()? else {
            continue;
        };
        // 单环或全局顶点超预算时拒绝网格化，避免 GPU 上传失控。
        path_vertices = path_vertices.checked_add(ring.len())?;
        if ring.len() > MAX_RING_VERTS || path_vertices > MAX_PATH_VERTS {
            return None;
        }
        rings.push(ring);
    }
    if rings.is_empty() {
        return Some(Vec::new());
    }

    if rings.iter().all(|ring| ring_is_simple(ring)) {
        // 全部环简单（无自交、互不相交）：走严格轮廓森林的快速 ear-clip 路径。
        if let Some(tris) = tessellate_strict_fill(&rings, fill_rule) {
            return Some(tris);
        }
    }
    // 存在相交 / 自交轮廓或严格路径失败：降级为 y 带分解的复杂路径网格化。
    tessellate_complex_fill(&rings, fill_rule)
}

/// 检查路径全部控制点是否为有限浮点坐标。
fn path_is_finite(path: &Path) -> bool {
    let finite = |point: Point| point.x.is_finite() && point.y.is_finite();
    path.segments().iter().all(|segment| match *segment {
        PathSegment::MoveTo(point) | PathSegment::LineTo(point) => finite(point),
        PathSegment::QuadTo(control, end) => finite(control) && finite(end),
        PathSegment::CubicTo(control1, control2, end) => {
            finite(control1) && finite(control2) && finite(end)
        }
        PathSegment::Close => true,
    })
}

/// 严格轮廓森林填充：先按包含关系分组，再逐组 ear-clip 成三角形。
fn tessellate_strict_fill(rings: &[Vec<Point>], fill_rule: FillRule) -> Option<Vec<f32>> {
    // 按 fill rule 建立包含森林并划分「外环 + 洞」组。
    let groups = classify_fill_groups(rings, fill_rule)?;
    let mut tris = Vec::new();
    for group in &groups {
        // 每组独立网格化后顺序拼接。
        tris.extend(tessellate_fill_group(rings, group)?);
    }
    // 三角形过少说明输入退化为无面积轮廓，拒绝输出。
    if tris.len() < 6 {
        return None;
    }
    Some(tris)
}

/// 把描边轮廓网格化为不重叠三角形列表。
///
/// cap/join 几何与 CPU 光栅化器共享；不支持的轮廓拓扑返回 `None`，调用方
/// 保留既有软回退。
pub fn tessellate_stroke(path: &Path, opts: &StrokeOptions) -> Option<Vec<f32>> {
    let rings = stroker::stroke_outline_rings(path, opts)?;
    if rings.is_empty() {
        return Some(Vec::new());
    }
    tessellate_fill(&stroker::path_from_outline_rings(rings), FillRule::NonZero)
}

/// 清理单个环：剔除非有限点与相邻重复点，闭合首尾，退化环返回 `None`。
fn clean_ring(pts: &[Point]) -> Result<Option<Vec<Point>>, ()> {
    if pts.len() < 3 {
        return Ok(None);
    }
    let mut out: Vec<Point> = Vec::with_capacity(pts.len());
    for p in pts {
        if !p.x.is_finite() || !p.y.is_finite() {
            return Err(());
        }
        if let Some(last) = out.last() {
            if (last.x - p.x).abs() < 1e-4 && (last.y - p.y).abs() < 1e-4 {
                continue;
            }
        }
        out.push(*p);
    }
    if out.len() >= 2 {
        let first = out[0];
        let &last = out.last().ok_or(())?;
        if (first.x - last.x).abs() < 1e-4 && (first.y - last.y).abs() < 1e-4 {
            out.pop();
        }
    }
    if out.len() < 3 {
        return Ok(None);
    }
    Ok(Some(out))
}

/// 判断环是否简单：任意非相邻边对都不相交或相触。
fn ring_is_simple(ring: &[Point]) -> bool {
    let len = ring.len();
    for i in 0..len {
        let a0 = ring[i];
        let a1 = ring[(i + 1) % len];
        for j in i + 1..len {
            if edges_are_adjacent(i, j, len) {
                continue;
            }
            let b0 = ring[j];
            let b1 = ring[(j + 1) % len];
            if segments_intersect_or_touch(a0, a1, b0, b1) {
                return false;
            }
        }
    }
    true
}

/// 一个严格填充组：一个外环（outer）加若干洞环（holes）。
#[derive(Debug)]
struct FillGroup {
    outer: usize,
    holes: Vec<usize>,
}

/// 构建严格包含森林，并按边界内外两侧的填充状态给每个环归类。
fn classify_fill_groups(rings: &[Vec<Point>], fill_rule: FillRule) -> Option<Vec<FillGroup>> {
    // 预计算每个环的有符号面积（f64 保持精度）。
    let areas: Vec<f64> = rings.iter().map(|ring| polygon_area_f64(ring)).collect();
    // 面积非有限或接近零（退化环）时无法确定绕数方向，拒绝归类。
    if areas
        .iter()
        .any(|area| !area.is_finite() || area.abs() <= 1e-8)
    {
        return None;
    }
    // 预计算包围盒，先做快速排除再逐环判定包含关系。
    let bounds: Vec<_> = rings.iter().map(|ring| ring_bounds(ring)).collect();
    let mut parents = vec![None; rings.len()];
    // 两两检查包围盒重叠的环对：相交 / 相触或互相包含都属于非严格拓扑。
    for i in 0..rings.len() {
        for j in i + 1..rings.len() {
            if !bounds_overlap(bounds[i], bounds[j]) {
                continue;
            }
            if rings_intersect_or_touch(&rings[i], &rings[j]) {
                return None;
            }
            let i_in_j = point_in_ring(rings[i][0], &rings[j]);
            let j_in_i = point_in_ring(rings[j][0], &rings[i]);
            if i_in_j && j_in_i {
                return None;
            }
            if i_in_j {
                // 环 i 完全位于环 j 内：登记父子关系（面积最小者胜出）。
                update_parent(&mut parents, &areas, i, j)?;
            } else if j_in_i {
                update_parent(&mut parents, &areas, j, i)?;
            }
        }
    }

    let mut order: Vec<usize> = (0..rings.len()).collect();
    // 按面积绝对值降序处理，保证父环先于子环被归类。
    order.sort_by(|&a, &b| {
        areas[b]
            .abs()
            .total_cmp(&areas[a].abs())
            .then_with(|| a.cmp(&b))
    });

    // 记录每个环「边界内」的绕数 / 奇偶状态及其所属组，供子环继承。
    let mut inside_winding = vec![None::<i32>; rings.len()];
    let mut inside_parity = vec![None::<bool>; rings.len()];
    let mut active_group = vec![None::<usize>; rings.len()];
    let mut groups = Vec::<FillGroup>::new();

    for ring in order {
        // 从直接父环继承边界外的填充事实；无父环时边界外为空。
        let (outside_winding, outside_parity, outside_group) = match parents[ring] {
            Some(parent) => (
                inside_winding[parent]?,
                inside_parity[parent]?,
                active_group[parent],
            ),
            None => (0, false, None),
        };
        // 面积符号给出环方向：正面积 +1 绕数，负面积 -1。
        let winding_delta = if areas[ring] > 0.0 { 1 } else { -1 };
        let winding = outside_winding.checked_add(winding_delta)?;
        let parity = !outside_parity;
        // 按 fill rule 分别判定边界外 / 内是否被填充。
        let outside_filled = match fill_rule {
            FillRule::EvenOdd => outside_parity,
            FillRule::NonZero => outside_winding != 0,
        };
        let inside_filled = match fill_rule {
            FillRule::EvenOdd => parity,
            FillRule::NonZero => winding != 0,
        };

        // 四种内外组合决定环的角色：新建组 / 加入洞 / 透传组 / 非法拓扑。
        let group = match (outside_filled, inside_filled) {
            // 外空内实：本环是新填充组的外环（同一区域不得嵌套两个组）。
            (false, true) => {
                if outside_group.is_some() {
                    return None;
                }
                let group = groups.len();
                groups.push(FillGroup {
                    outer: ring,
                    holes: Vec::new(),
                });
                Some(group)
            }
            // 外实内空：本环是所在组的一个洞。
            (true, false) => {
                let group = outside_group?;
                groups.get_mut(group)?.holes.push(ring);
                None
            }
            // 外实内实：本环位于既有组的填充内部，继续沿用该组。
            (true, true) => Some(outside_group?),
            // 外空内空：无任何填充贡献的孤立区域，属于未定义拓扑。
            (false, false) => return None,
        };

        // 记录本环边界内的填充事实供子环继承。
        inside_winding[ring] = Some(winding);
        inside_parity[ring] = Some(parity);
        active_group[ring] = group;
    }

    if groups.is_empty() {
        None
    } else {
        Some(groups)
    }
}

/// 若 `candidate` 面积更小则把 `child` 的直接父环更新为 `candidate`。
fn update_parent(
    parents: &mut [Option<usize>],
    areas: &[f64],
    child: usize,
    candidate: usize,
) -> Option<()> {
    if areas[candidate].abs() <= areas[child].abs() {
        return None;
    }
    match parents[child] {
        Some(current) if areas[current].abs() <= areas[candidate].abs() => {}
        _ => parents[child] = Some(candidate),
    }
    Some(())
}

/// 网格化单个填充组：无洞走自实现 ear-clip，有洞把全部环拼给 earcut。
fn tessellate_fill_group(rings: &[Vec<Point>], group: &FillGroup) -> Option<Vec<f32>> {
    let outer = rings.get(group.outer)?;
    // 期望面积 = 外环面积减去全部洞面积；非正说明洞与环重叠或退化。
    let expected_area =
        group
            .holes
            .iter()
            .try_fold(polygon_area_f64(outer).abs(), |area, &hole| {
                let remaining = area - polygon_area_f64(rings.get(hole)?).abs();
                remaining.is_finite().then_some(remaining)
            })?;
    if expected_area <= 1e-8 {
        return None;
    }

    let tris = if group.holes.is_empty() {
        // 简单无洞多边形：自实现 ear-clip。
        ear_clip(outer)?
    } else {
        // 带洞多边形：把所有环顶点拼进同一数组，并记录每个洞的起始索引。
        let vertex_count = group.holes.iter().try_fold(outer.len(), |count, &hole| {
            count.checked_add(rings.get(hole)?.len())
        })?;
        let mut vertices = Vec::<[f32; 2]>::with_capacity(vertex_count);
        vertices.extend(outer.iter().map(|point| [point.x, point.y]));
        let mut hole_indices = Vec::with_capacity(group.holes.len());
        for &hole in &group.holes {
            hole_indices.push(vertices.len());
            vertices.extend(rings.get(hole)?.iter().map(|point| [point.x, point.y]));
        }

        // 交给 earcut 生成三角形索引；索引不完整视为失败。
        let mut indices = Vec::<usize>::new();
        earcut::Earcut::<f32>::new().earcut(vertices.iter().copied(), &hole_indices, &mut indices);
        if indices.len() < 3 || !indices.chunks_exact(3).remainder().is_empty() {
            return None;
        }

        // 把每个三角形索引展开为 xy 坐标序列。
        let mut triangles = Vec::with_capacity(indices.len().checked_mul(2)?);
        for triangle in indices.chunks_exact(3) {
            let a = *vertices.get(triangle[0])?;
            let b = *vertices.get(triangle[1])?;
            let c = *vertices.get(triangle[2])?;
            triangles.extend_from_slice(&[a[0], a[1], b[0], b[1], c[0], c[1]]);
        }
        triangles
    };

    // 总面积校验：网格化结果必须与期望面积在容差内一致。
    validate_triangle_area(&tris, expected_area).then_some(tris)
}

/// 校验三角形列表的累计面积与期望面积一致（容差内），并拒绝非有限坐标。
fn validate_triangle_area(tris: &[f32], expected_area: f64) -> bool {
    if tris.len() < 6 || !tris.chunks_exact(6).remainder().is_empty() {
        return false;
    }
    let mut triangulated_area = 0.0f64;
    for triangle in tris.chunks_exact(6) {
        if triangle.iter().any(|coordinate| !coordinate.is_finite()) {
            return false;
        }
        let (ax, ay) = (triangle[0] as f64, triangle[1] as f64);
        let (bx, by) = (triangle[2] as f64, triangle[3] as f64);
        let (cx, cy) = (triangle[4] as f64, triangle[5] as f64);
        triangulated_area += ((bx - ax) * (cy - ay) - (by - ay) * (cx - ax)).abs() * 0.5;
    }
    let tolerance = expected_area.max(1.0) * 1e-4;
    (triangulated_area - expected_area).abs() <= tolerance
}

/// 双精度点：复杂路径分解中用以保持交点与事件坐标的数值稳定。
#[derive(Debug, Clone, Copy)]
pub(crate) struct F64Point {
    pub(crate) x: f64,
    pub(crate) y: f64,
}

impl From<Point> for F64Point {
    /// f32 点无损提升为 f64 点。
    fn from(point: Point) -> Self {
        Self {
            x: point.x as f64,
            y: point.y as f64,
        }
    }
}

/// 复杂路径中的一条有向边：记录端点、y 范围与方向绕数。
#[derive(Debug, Clone, Copy)]
struct ComplexEdge {
    id: usize,
    start: F64Point,
    end: F64Point,
    ymin: f64,
    ymax: f64,
    winding: i32,
}

impl ComplexEdge {
    /// 求边在给定 y 处的 x 坐标；水平边（dy==0）返回 None。
    fn x_at(self, y: f64) -> Option<f64> {
        let dy = self.end.y - self.start.y;
        if dy == 0.0 {
            return None;
        }
        let x = self.start.x + (y - self.start.y) * (self.end.x - self.start.x) / dy;
        x.is_finite().then_some(x)
    }
}

/// 一个 y 带内某条边在带上下边界与中点处的 x 采样，及其绕数贡献。
#[derive(Debug, Clone, Copy)]
struct BandCrossing {
    edge_id: usize,
    x_mid: f64,
    x0: f64,
    x1: f64,
    winding: i32,
}

/// 一条填充跨度左 / 右边界的线性插值线段。
#[derive(Debug, Clone, Copy)]
struct BoundaryLine {
    x0: f64,
    x1: f64,
}

/// 把任意有向轮廓分解为不重叠填充梯形。
///
/// 每个顶点与正规交点的 y 都是带边界，因此每个开放带内边的顺序保持稳定。
pub(crate) fn tessellate_complex_fill(
    rings: &[Vec<Point>],
    fill_rule: FillRule,
) -> Option<Vec<f32>> {
    let mut edges = Vec::<ComplexEdge>::new();
    let mut events = Vec::<f64>::new();
    for ring in rings {
        // 收集环顶点 y 作为带边界。
        events.extend(ring.iter().map(|point| point.y as f64));
        for index in 0..ring.len() {
            let start = F64Point::from(ring[index]);
            let end = F64Point::from(ring[(index + 1) % ring.len()]);
            let dx = end.x - start.x;
            let dy = end.y - start.y;
            if dx == 0.0 && dy == 0.0 {
                // 退化点边跳过。
                continue;
            }
            if dy == 0.0 {
                // 水平边不参与带扫描，跳过。
                continue;
            }
            // 方向绕数：向下的边 +1，向上的边 -1。
            edges.push(ComplexEdge {
                id: edges.len(),
                start,
                end,
                ymin: start.y.min(end.y),
                ymax: start.y.max(end.y),
                winding: if end.y > start.y { 1 } else { -1 },
            });
        }
    }
    if edges.is_empty() {
        return Some(Vec::new());
    }

    // 边对工作量预算：O(n²) 求交尝试先于实际求交执行。
    let mut work = edges
        .len()
        .checked_mul(edges.len().saturating_sub(1))?
        .checked_div(2)?;
    if work > MAX_COMPLEX_WORK {
        return None;
    }

    // 求所有包围盒重叠边对的正规交点 y，作为额外带边界。
    let max_event_candidates = MAX_COMPLEX_EVENTS.checked_mul(8)?;
    for left in 0..edges.len() {
        for right in left + 1..edges.len() {
            if !complex_edge_bounds_overlap(edges[left], edges[right]) {
                continue;
            }
            if let Some(y) = proper_intersection_y(edges[left], edges[right])? {
                events.push(y);
                if events.len() > max_event_candidates {
                    return None;
                }
            }
        }
    }

    // 排序去重后得到全部带边界；事件过少时没有任何开放带。
    events.sort_by(f64::total_cmp);
    events.dedup_by(|a, b| *a == *b);
    if events.len() > MAX_COMPLEX_EVENTS {
        return None;
    }
    if events.len() < 2 {
        return Some(Vec::new());
    }
    // 带数 × 边数的累计扫描工作量预算。
    work = work.checked_add(edges.len().checked_mul(events.len() - 1)?)?;
    if work > MAX_COMPLEX_WORK {
        return None;
    }

    let mut triangles = Vec::<f32>::new();
    let mut expected_area = 0.0f64;
    for band in events.windows(2) {
        let (y0, y1) = (band[0], band[1]);
        if y1 <= y0 {
            continue;
        }
        if approximately_equal(y0, y1) {
            // 不合并彼此接近但不同的事件：其顺序在数值上存在歧义，
            // 通过软回退保持正确性。
            return None;
        }
        // 扫描与带中点相交的全部边，收集带两端与中点的 x 采样。
        let y_mid = y0 + (y1 - y0) * 0.5;
        let mut crossings = Vec::<BandCrossing>::new();
        for &edge in &edges {
            if !(edge.ymin < y_mid && y_mid < edge.ymax) {
                continue;
            }
            crossings.push(BandCrossing {
                edge_id: edge.id,
                x_mid: edge.x_at(y_mid)?,
                x0: edge.x_at(y0)?,
                x1: edge.x_at(y1)?,
                winding: edge.winding,
            });
        }
        if crossings.is_empty() {
            continue;
        }
        // 排序工作量预算（按 crossing 数估算比较层数）。
        let sort_levels = usize::BITS as usize - crossings.len().leading_zeros() as usize;
        work = work.checked_add(crossings.len().checked_mul(sort_levels)?)?;
        if work > MAX_COMPLEX_WORK {
            return None;
        }
        // 按带中点 x 排序；并列时依次以带两端 x 与边 id 稳定排序。
        crossings.sort_by(|a, b| {
            a.x_mid
                .total_cmp(&b.x_mid)
                .then_with(|| a.x0.total_cmp(&b.x0))
                .then_with(|| a.x1.total_cmp(&b.x1))
                .then_with(|| a.edge_id.cmp(&b.edge_id))
        });

        // 自左向右扫描 crossings：维护绕数 / 奇偶与当前填充跨度的左边界。
        let mut winding = 0i32;
        let mut parity = false;
        let mut left_boundary = None::<BoundaryLine>;
        let mut index = 0usize;
        while index < crossings.len() {
            // 归并中点 x 近似相同的并列 crossing 组，整组一次翻转填充状态。
            let mut end = index + 1;
            while end < crossings.len()
                && approximately_equal(crossings[index].x_mid, crossings[end].x_mid)
            {
                if !same_band_line(crossings[index], crossings[end], &edges) {
                    // 近似接近不能证明共线：接近但不重合的线或丢失的
                    // 交点属于数值歧义，直接回退。
                    return None;
                }
                end += 1;
            }

            // 记录翻转前的填充状态。
            let before = fill_state(fill_rule, winding, parity);
            // 按 fill rule 更新绕数 / 奇偶：EvenOdd 奇数次翻转，NonZero 累加带符号绕数。
            match fill_rule {
                FillRule::EvenOdd => {
                    if (end - index) % 2 == 1 {
                        parity = !parity;
                    }
                }
                FillRule::NonZero => {
                    let delta = crossings[index..end]
                        .iter()
                        .try_fold(0i32, |sum, crossing| sum.checked_add(crossing.winding))?;
                    winding = winding.checked_add(delta)?;
                }
            }
            let after = fill_state(fill_rule, winding, parity);
            // 边界处取带两端 x 的插值作为跨度边界线。
            let boundary = BoundaryLine {
                x0: crossings[index].x0,
                x1: crossings[index].x1,
            };
            // 进入填充：记录左边界；离开填充：用左右边界生成梯形三角形。
            match (before, after) {
                (false, true) => {
                    if left_boundary.replace(boundary).is_some() {
                        return None;
                    }
                }
                (true, false) => {
                    let left = left_boundary.take()?;
                    expected_area += append_complex_span(&mut triangles, left, boundary, y0, y1)?;
                }
                (false, false) | (true, true) => {}
            }
            index = end;
        }
        // 带尾必须回到未填充状态，否则说明 crossing 解析不一致。
        if fill_state(fill_rule, winding, parity) || left_boundary.is_some() {
            return None;
        }
    }

    if triangles.is_empty() {
        return Some(triangles);
    }
    // 与严格路径一致，做总面积校验后交付。
    validate_triangle_area(&triangles, expected_area).then_some(triangles)
}

/// 两条复杂边的包围盒是否重叠（含边界）。
fn complex_edge_bounds_overlap(a: ComplexEdge, b: ComplexEdge) -> bool {
    let a_min_x = a.start.x.min(a.end.x);
    let a_max_x = a.start.x.max(a.end.x);
    let b_min_x = b.start.x.min(b.end.x);
    let b_max_x = b.start.x.max(b.end.x);
    a_min_x <= b_max_x && b_min_x <= a_max_x && a.ymin <= b.ymax && b.ymin <= a.ymax
}

/// 返回正规内点交点的 y：`Some(Some(y))` 表示存在正规交点，
/// `Some(None)` 表示不相交或平行，`None` 表示非有限算术。
fn proper_intersection_y(a: ComplexEdge, b: ComplexEdge) -> Option<Option<f64>> {
    let a_direction = F64Point {
        x: a.end.x - a.start.x,
        y: a.end.y - a.start.y,
    };
    let b_direction = F64Point {
        x: b.end.x - b.start.x,
        y: b.end.y - b.start.y,
    };
    let denominator = cross_f64(a_direction, b_direction);
    if !denominator.is_finite() {
        return None;
    }
    if denominator == 0.0 {
        return Some(None);
    }
    let delta = F64Point {
        x: b.start.x - a.start.x,
        y: b.start.y - a.start.y,
    };
    let t = cross_f64(delta, b_direction) / denominator;
    let u = cross_f64(delta, a_direction) / denominator;
    if !t.is_finite() || !u.is_finite() {
        return None;
    }
    if !(0.0 < t && t < 1.0 && 0.0 < u && u < 1.0) {
        return Some(None);
    }
    let y = a.start.y + t * a_direction.y;
    y.is_finite().then_some(Some(y))
}

/// 按 fill rule 计算当前（绕数, 奇偶）对应的填充布尔值。
fn fill_state(fill_rule: FillRule, winding: i32, parity: bool) -> bool {
    match fill_rule {
        FillRule::EvenOdd => parity,
        FillRule::NonZero => winding != 0,
    }
}

/// 判断两条边在带内是否共线：平行、共起点且带内 x 采样一致。
fn same_band_line(a: BandCrossing, b: BandCrossing, edges: &[ComplexEdge]) -> bool {
    let a_edge = edges[a.edge_id];
    let b_edge = edges[b.edge_id];
    let a_direction = F64Point {
        x: a_edge.end.x - a_edge.start.x,
        y: a_edge.end.y - a_edge.start.y,
    };
    let b_direction = F64Point {
        x: b_edge.end.x - b_edge.start.x,
        y: b_edge.end.y - b_edge.start.y,
    };
    let start_delta = F64Point {
        x: b_edge.start.x - a_edge.start.x,
        y: b_edge.start.y - a_edge.start.y,
    };
    if cross_f64(a_direction, b_direction) != 0.0 || cross_f64(a_direction, start_delta) != 0.0 {
        return false;
    }
    approximately_equal(a.x_mid, b.x_mid)
        && approximately_equal(a.x0, b.x0)
        && approximately_equal(a.x1, b.x1)
}

/// 相对容差近似相等（以两者中较大绝对值为尺度）。
fn approximately_equal(a: f64, b: f64) -> bool {
    let scale = a.abs().max(b.abs()).max(1.0);
    (a - b).abs() <= scale * 1e-10
}

/// 用左右边界线生成一个梯形（两个三角形）并追加到输出，返回其面积。
fn append_complex_span(
    triangles: &mut Vec<f32>,
    left: BoundaryLine,
    right: BoundaryLine,
    y0: f64,
    y1: f64,
) -> Option<f64> {
    let (left0, right0) = ordered_or_snapped(left.x0, right.x0)?;
    let (left1, right1) = ordered_or_snapped(left.x1, right.x1)?;
    let width0 = right0 - left0;
    let width1 = right1 - left1;
    let area = (width0 + width1) * 0.5 * (y1 - y0);
    if !area.is_finite() || area < 0.0 {
        return None;
    }
    if area == 0.0 {
        return Some(0.0);
    }

    let top_left = F64Point { x: left0, y: y0 };
    let top_right = F64Point { x: right0, y: y0 };
    let bottom_right = F64Point { x: right1, y: y1 };
    let bottom_left = F64Point { x: left1, y: y1 };
    let before = triangles.len();
    append_complex_triangle(triangles, top_left, top_right, bottom_right)?;
    append_complex_triangle(triangles, top_left, bottom_right, bottom_left)?;
    if triangles.len() == before {
        return None;
    }
    Some(area)
}

/// 追加一个三角形（6 个 f32），拒绝非有限坐标、零面积与超预算三角形。
pub(crate) fn append_complex_triangle(
    triangles: &mut Vec<f32>,
    a: F64Point,
    b: F64Point,
    c: F64Point,
) -> Option<()> {
    let vertices = [
        a.x as f32, a.y as f32, b.x as f32, b.y as f32, c.x as f32, c.y as f32,
    ];
    if vertices.iter().any(|coordinate| !coordinate.is_finite()) {
        return None;
    }
    let (ax, ay) = (vertices[0] as f64, vertices[1] as f64);
    let (bx, by) = (vertices[2] as f64, vertices[3] as f64);
    let (cx, cy) = (vertices[4] as f64, vertices[5] as f64);
    let area = ((bx - ax) * (cy - ay) - (by - ay) * (cx - ax)).abs() * 0.5;
    if area <= 1e-12 {
        return Some(());
    }
    if triangles.len().checked_div(6)? >= MAX_COMPLEX_TRIANGLES {
        return None;
    }
    triangles.extend_from_slice(&vertices);
    Some(())
}

/// 返回有序的 (left, right)；若仅因浮点误差倒置则在近似相等时收敛为中点。
fn ordered_or_snapped(left: f64, right: f64) -> Option<(f64, f64)> {
    if left <= right {
        return Some((left, right));
    }
    if approximately_equal(left, right) {
        let midpoint = left + (right - left) * 0.5;
        return Some((midpoint, midpoint));
    }
    None
}

/// f64 二维叉积。
fn cross_f64(a: F64Point, b: F64Point) -> f64 {
    a.x * b.y - a.y * b.x
}

/// 鞋带公式计算多边形有符号面积（f64）。
fn polygon_area_f64(pts: &[Point]) -> f64 {
    let mut area = 0.0f64;
    for i in 0..pts.len() {
        let point = pts[i];
        let next = pts[(i + 1) % pts.len()];
        area += point.x as f64 * next.y as f64 - next.x as f64 * point.y as f64;
    }
    area * 0.5
}

/// 计算环的包围盒 (min_x, min_y, max_x, max_y)。
fn ring_bounds(ring: &[Point]) -> (f32, f32, f32, f32) {
    ring.iter().fold(
        (
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ),
        |(min_x, min_y, max_x, max_y), point| {
            (
                min_x.min(point.x),
                min_y.min(point.y),
                max_x.max(point.x),
                max_y.max(point.y),
            )
        },
    )
}

/// 包围盒是否重叠（含 epsilon 容差）。
fn bounds_overlap(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    const EPSILON: f32 = 1e-5;
    a.0 <= b.2 + EPSILON && a.2 + EPSILON >= b.0 && a.1 <= b.3 + EPSILON && a.3 + EPSILON >= b.1
}

/// 两条边是否在环中相邻（共享端点）。
fn edges_are_adjacent(a: usize, b: usize, len: usize) -> bool {
    a == b || (a + 1) % len == b || (b + 1) % len == a
}

/// 两个环的任意边对是否相交或相触。
fn rings_intersect_or_touch(a: &[Point], b: &[Point]) -> bool {
    for i in 0..a.len() {
        let a0 = a[i];
        let a1 = a[(i + 1) % a.len()];
        for j in 0..b.len() {
            let b0 = b[j];
            let b1 = b[(j + 1) % b.len()];
            if segments_intersect_or_touch(a0, a1, b0, b1) {
                return true;
            }
        }
    }
    false
}

/// 判断两线段是否正规相交或端点相触（含 epsilon 容差）。
fn segments_intersect_or_touch(a0: Point, a1: Point, b0: Point, b1: Point) -> bool {
    const EPSILON: f32 = 1e-5;

    let o1 = cross(a0, a1, b0);
    let o2 = cross(a0, a1, b1);
    let o3 = cross(b0, b1, a0);
    let o4 = cross(b0, b1, a1);
    let proper = ((o1 > EPSILON && o2 < -EPSILON) || (o1 < -EPSILON && o2 > EPSILON))
        && ((o3 > EPSILON && o4 < -EPSILON) || (o3 < -EPSILON && o4 > EPSILON));
    proper
        || (o1.abs() <= EPSILON && point_on_segment(b0, a0, a1, EPSILON))
        || (o2.abs() <= EPSILON && point_on_segment(b1, a0, a1, EPSILON))
        || (o3.abs() <= EPSILON && point_on_segment(a0, b0, b1, EPSILON))
        || (o4.abs() <= EPSILON && point_on_segment(a1, b0, b1, EPSILON))
}

/// 点在包围盒意义上是否位于线段上（配合 epsilon）。
fn point_on_segment(p: Point, a: Point, b: Point, epsilon: f32) -> bool {
    p.x >= a.x.min(b.x) - epsilon
        && p.x <= a.x.max(b.x) + epsilon
        && p.y >= a.y.min(b.y) - epsilon
        && p.y <= a.y.max(b.y) + epsilon
}

/// 射线法判断点是否位于环内。
fn point_in_ring(point: Point, ring: &[Point]) -> bool {
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[j];
        let crosses = (a.y > point.y) != (b.y > point.y);
        if crosses {
            let x = (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x;
            if point.x < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// 鞋带公式计算多边形有符号面积（f32，供 ear-clip 定向使用）。
fn polygon_area(pts: &[Point]) -> f32 {
    let n = pts.len();
    let mut a = 0.0;
    for i in 0..n {
        let p = pts[i];
        let q = pts[(i + 1) % n];
        a += p.x * q.y - q.x * p.y;
    }
    a * 0.5
}

/// 以 o 为原点的二维叉积（判断三点转向）。
pub(crate) fn cross(o: Point, a: Point, b: Point) -> f32 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

/// 点是否位于三角形内（含边界，同侧法）。
pub(crate) fn point_in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
    let c1 = cross(a, b, p);
    let c2 = cross(b, c, p);
    let c3 = cross(c, a, p);
    let has_neg = (c1 < 0.0) || (c2 < 0.0) || (c3 < 0.0);
    let has_pos = (c1 > 0.0) || (c2 > 0.0) || (c3 > 0.0);
    !(has_neg && has_pos)
}

/// 顶点在给定环绕方向下是否凸出。
fn is_convex(prev: Point, curr: Point, next: Point, ccw: bool) -> bool {
    let c = cross(prev, curr, next);
    if ccw {
        c > 1e-6
    } else {
        c < -1e-6
    }
}

/// 判断顶点是否为「耳朵」：凸出且三角形内不包含其他顶点。
fn is_ear(pts: &[Point], indices: &[usize], ear_i: usize, ccw: bool) -> bool {
    let n = indices.len();
    let i_prev = indices[(ear_i + n - 1) % n];
    let i_curr = indices[ear_i];
    let i_next = indices[(ear_i + 1) % n];
    let a = pts[i_prev];
    let b = pts[i_curr];
    let c = pts[i_next];
    if !is_convex(a, b, c, ccw) {
        return false;
    }
    for (j, &idx) in indices.iter().enumerate() {
        if j == (ear_i + n - 1) % n || j == ear_i || j == (ear_i + 1) % n {
            continue;
        }
        if point_in_triangle(pts[idx], a, b, c) {
            return false;
        }
    }
    true
}

/// 自实现 ear-clip：对无洞简单多边形输出三角形列表，失败返回 None。
fn ear_clip(ring: &[Point]) -> Option<Vec<f32>> {
    let area = polygon_area(ring);
    // 零面积退化环没有可切内容。
    if area.abs() < 1e-8 {
        return None;
    }
    // 由面积符号确定环绕方向。
    let ccw = area > 0.0;
    // 用剩余顶点索引表模拟不断收缩的多边形。
    let mut indices: Vec<usize> = (0..ring.len()).collect();
    let mut tris: Vec<f32> = Vec::with_capacity((ring.len().saturating_sub(2)) * 6);
    // 防死循环守卫：每轮必须切下一个耳朵，否则放弃。
    let mut guard = ring.len() * ring.len() + 8;
    while indices.len() > 3 {
        if guard == 0 {
            return None;
        }
        guard -= 1;
        // 扫描剩余顶点找第一个耳朵。
        let n = indices.len();
        let mut found = None;
        for i in 0..n {
            if is_ear(ring, &indices, i, ccw) {
                found = Some(i);
                break;
            }
        }
        // 找不到耳朵说明多边形退化，放弃。
        let i = found?;
        // 切下耳朵：输出三角形并删除该顶点。
        let i_prev = indices[(i + n - 1) % n];
        let i_curr = indices[i];
        let i_next = indices[(i + 1) % n];
        let a = ring[i_prev];
        let b = ring[i_curr];
        let c = ring[i_next];
        tris.extend_from_slice(&[a.x, a.y, b.x, b.y, c.x, c.y]);
        indices.remove(i);
    }
    // 最后三个顶点构成收尾三角形。
    let a = ring[indices[0]];
    let b = ring[indices[1]];
    let c = ring[indices[2]];
    tris.extend_from_slice(&[a.x, a.y, b.x, b.y, c.x, c.y]);
    Some(tris)
}
