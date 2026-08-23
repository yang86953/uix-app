//! 实心三角网格到逐顶点覆盖率网格的共享 lowering。
//!
//! 输入是物理坐标 `xy` 三角列表；输出是 `xy + coverage` 三角列表。组件只负责
//! 双像素抗锯齿边带，不理解 Path、Widget 或任一原生图形 API。

use std::collections::HashMap;
use std::sync::Arc;

// 抗锯齿边带以数学边界为中心，内外各占一个物理像素；
// 双像素过渡可让单采样目标上的浅斜边稳定落入多个部分覆盖像素。
const FEATHER_RADIUS: f32 = 1.0;
// 限制尖角 miter，避免极小夹角把边带扩展成大面积尖刺。
const MAX_MITER: f32 = 4.0;
// 退化边和翻转三角形共用的物理面积门限。
const AREA_EPSILON: f32 = 1e-5;

// 用精确物理坐标位模式标识网格顶点；零值归一化避免正负零分裂拓扑。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct PointKey([u32; 2]);

impl PointKey {
    // 从已经通过有限值门禁的坐标构造稳定键。
    fn new(point: [f32; 2]) -> Self {
        let canonical = |value: f32| if value == 0.0 { 0.0 } else { value };
        Self([canonical(point[0]).to_bits(), canonical(point[1]).to_bits()])
    }
}

// 保存一个无向三角边的第一次有向出现与累计次数。
#[derive(Debug, Clone, Copy)]
struct MeshEdge {
    start: [f32; 2],
    end: [f32; 2],
    opposite: [f32; 2],
    count: u32,
}

// 汇总同一边界顶点相邻边的外法线，用于构造连续 miter。
#[derive(Debug, Clone, Copy)]
struct NormalAccumulator {
    sum: [f32; 2],
    first: [f32; 2],
    count: u32,
}

// 为无向边生成与端点顺序无关的键。
fn edge_key(start: [f32; 2], end: [f32; 2]) -> (PointKey, PointKey) {
    let start = PointKey::new(start);
    let end = PointKey::new(end);
    if start <= end {
        (start, end)
    } else {
        (end, start)
    }
}

// 计算一条边相对其唯一相邻三角形的单位外法线。
fn outward_normal(edge: MeshEdge) -> Option<[f32; 2]> {
    let dx = edge.end[0] - edge.start[0];
    let dy = edge.end[1] - edge.start[1];
    let length = (dx * dx + dy * dy).sqrt();
    if !length.is_finite() || length <= AREA_EPSILON {
        return None;
    }
    let side = dx * (edge.opposite[1] - edge.start[1]) - dy * (edge.opposite[0] - edge.start[0]);
    if !side.is_finite() || side.abs() <= AREA_EPSILON {
        return None;
    }
    if side > 0.0 {
        Some([dy / length, -dx / length])
    } else {
        Some([-dy / length, dx / length])
    }
}

// 把一条单位外法线累加到边界顶点。
fn accumulate_normal(
    normals: &mut HashMap<PointKey, NormalAccumulator>,
    point: [f32; 2],
    normal: [f32; 2],
) {
    normals
        .entry(PointKey::new(point))
        .and_modify(|value| {
            value.sum[0] += normal[0];
            value.sum[1] += normal[1];
            value.count += 1;
        })
        .or_insert(NormalAccumulator {
            sum: normal,
            first: normal,
            count: 1,
        });
}

// 从相邻外法线生成同时满足两条边半像素距离的 miter 偏移。
fn miter_offset(normals: NormalAccumulator) -> [f32; 2] {
    let length = (normals.sum[0] * normals.sum[0] + normals.sum[1] * normals.sum[1]).sqrt();
    let direction = if length > AREA_EPSILON {
        [normals.sum[0] / length, normals.sum[1] / length]
    } else {
        normals.first
    };
    let projection = (direction[0] * normals.first[0] + direction[1] * normals.first[1]).abs();
    let distance = if normals.count >= 2 && projection > AREA_EPSILON {
        (FEATHER_RADIUS / projection).min(MAX_MITER)
    } else {
        FEATHER_RADIUS
    };
    [direction[0] * distance, direction[1] * distance]
}

// 追加一个逐顶点覆盖率值。
fn push_vertex(output: &mut Vec<f32>, point: [f32; 2], coverage: f32) {
    output.extend_from_slice(&[point[0], point[1], coverage]);
}

// 在无法安全建立边界拓扑时保留原网格，并为全部顶点写入完整覆盖率。
fn opaque_vertices(vertices: &[f32]) -> Arc<[f32]> {
    let mut output = Vec::with_capacity(vertices.len() / 2 * 3);
    for pair in vertices.chunks_exact(2) {
        push_vertex(&mut output, [pair[0], pair[1]], 1.0);
    }
    Arc::from(output)
}

/// 把物理 `xy` 三角列表转换成带双像素连续 coverage 边带的 `xyc` 三角列表。
pub(super) fn antialiased_vertices(vertices: &[f32]) -> Arc<[f32]> {
    // 上层已经验证基本 ABI；这里仍保留局部防御，异常输入只退回不透明网格。
    if vertices.len() < 6 || !vertices.len().is_multiple_of(6) {
        return opaque_vertices(vertices);
    }

    let mut edges = HashMap::<(PointKey, PointKey), MeshEdge>::new();
    for triangle in vertices.chunks_exact(6) {
        let points = [
            [triangle[0], triangle[1]],
            [triangle[2], triangle[3]],
            [triangle[4], triangle[5]],
        ];
        for index in 0..3 {
            let start = points[index];
            let end = points[(index + 1) % 3];
            let opposite = points[(index + 2) % 3];
            edges
                .entry(edge_key(start, end))
                .and_modify(|edge| edge.count = edge.count.saturating_add(1))
                .or_insert(MeshEdge {
                    start,
                    end,
                    opposite,
                    count: 1,
                });
        }
    }

    // 只保留恰好属于一个三角形的真实轮廓边；共享边不得产生内部亮缝。
    let boundary: Vec<_> = edges
        .into_values()
        .filter(|edge| edge.count == 1)
        .filter_map(|edge| outward_normal(edge).map(|normal| (edge, normal)))
        .collect();
    if boundary.is_empty() {
        return opaque_vertices(vertices);
    }

    let mut normals = HashMap::<PointKey, NormalAccumulator>::new();
    for (edge, normal) in &boundary {
        accumulate_normal(&mut normals, edge.start, *normal);
        accumulate_normal(&mut normals, edge.end, *normal);
    }
    let offsets: HashMap<_, _> = normals
        .into_iter()
        .map(|(point, value)| (point, miter_offset(value)))
        .collect();

    // 先把原轮廓内缩一个像素，形成 coverage=1 的稳定内部三角列表。
    let mut inset = Vec::with_capacity(vertices.len());
    for pair in vertices.chunks_exact(2) {
        let point = [pair[0], pair[1]];
        let offset = offsets
            .get(&PointKey::new(point))
            .copied()
            .unwrap_or([0.0, 0.0]);
        inset.extend_from_slice(&[point[0] - offset[0], point[1] - offset[1]]);
    }

    // 极薄或退化几何若被内缩翻转，宁可保留硬边也不能提交破损拓扑。
    for (original, shifted) in vertices.chunks_exact(6).zip(inset.chunks_exact(6)) {
        let original_area = (original[2] - original[0]) * (original[5] - original[1])
            - (original[3] - original[1]) * (original[4] - original[0]);
        let shifted_area = (shifted[2] - shifted[0]) * (shifted[5] - shifted[1])
            - (shifted[3] - shifted[1]) * (shifted[4] - shifted[0]);
        if original_area.abs() > AREA_EPSILON
            && (shifted_area.abs() <= AREA_EPSILON || original_area * shifted_area <= 0.0)
        {
            return opaque_vertices(vertices);
        }
    }

    let mut output = Vec::with_capacity(inset.len() / 2 * 3 + boundary.len() * 18);
    for pair in inset.chunks_exact(2) {
        push_vertex(&mut output, [pair[0], pair[1]], 1.0);
    }
    // 每条轮廓边追加两个三角形；共享 miter 端点让相邻边带无缝闭合。
    for (edge, _) in boundary {
        let start_offset = offsets[&PointKey::new(edge.start)];
        let end_offset = offsets[&PointKey::new(edge.end)];
        let inner_start = [
            edge.start[0] - start_offset[0],
            edge.start[1] - start_offset[1],
        ];
        let inner_end = [edge.end[0] - end_offset[0], edge.end[1] - end_offset[1]];
        let outer_start = [
            edge.start[0] + start_offset[0],
            edge.start[1] + start_offset[1],
        ];
        let outer_end = [edge.end[0] + end_offset[0], edge.end[1] + end_offset[1]];
        push_vertex(&mut output, inner_start, 1.0);
        push_vertex(&mut output, inner_end, 1.0);
        push_vertex(&mut output, outer_end, 0.0);
        push_vertex(&mut output, inner_start, 1.0);
        push_vertex(&mut output, outer_end, 0.0);
        push_vertex(&mut output, outer_start, 0.0);
    }
    Arc::from(output)
}

#[cfg(test)]
#[path = "../../../tests/unit/draw/backend/rhi_renderer_mesh__tests.rs"]
mod tests;
