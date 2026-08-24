//! SoftwareRasterizer — 通用栅格渲染器。
//!
//! 持有渲染状态（裁剪、透明度、偏移、变换、混合模式），
//! 提供所有 Canvas2D 绘制方法的实现，但输出目标由调用者以像素缓冲传递。
//! CPU/GPU 后端只需「往哪写像素」，渲染逻辑由这里统一完成。

use crate::core::Rect;

use crate::draw::geometry::color::Color;
use crate::draw::geometry::types::{BlendMode, Transform};
use crate::draw::raster::rasterizer::core as rast;

// 单个瞬态状态栈最多跨帧保留 16 KiB，避免异常深度永久抬高内存水位。
pub(crate) const MAX_RETAINED_TRANSIENT_STACK_BYTES: usize = 16 * 1024;
// 全部非活动 save 快照合计最多跨帧保留 64 KiB。
const MAX_RETAINED_SNAPSHOT_POOL_BYTES: usize = 64 * 1024;

fn clear_reusable_stack<T>(stack: &mut Vec<T>) {
    stack.clear();
    if stack.capacity().saturating_mul(std::mem::size_of::<T>())
        > MAX_RETAINED_TRANSIENT_STACK_BYTES
    {
        *stack = Vec::new();
    }
}

/// 渲染状态快照（用于 save/restore）。
struct StateSnapshot {
    clip_rect: Rect,
    clip_int: (i32, i32, i32, i32),
    // 保存路径 mask，保证 save/restore 不丢失非矩形裁剪状态。
    clip_mask: Option<Vec<u8>>,
    // 保存矩形/路径裁剪栈，保证 restore 后后续 pop 顺序仍然一致。
    clip_stack: Vec<Rect>,
    // 保存每一级裁剪前的路径 mask。
    clip_mask_stack: Vec<Option<Vec<u8>>>,
    opacity: f32,
    offset_x: f32,
    offset_y: f32,
    transform: Transform,
    invert: Option<[f64; 6]>,
    blend_mode: BlendMode,
}

impl StateSnapshot {
    fn release_payload_for_reuse(&mut self) {
        self.clip_mask = None;
        clear_reusable_stack(&mut self.clip_stack);
        clear_reusable_stack(&mut self.clip_mask_stack);
    }

    fn retained_memory_usage(&self) -> usize {
        self.clip_mask.as_ref().map_or(0, Vec::capacity)
            + self.clip_stack.capacity() * std::mem::size_of::<Rect>()
            + self.clip_mask_stack.capacity() * std::mem::size_of::<Option<Vec<u8>>>()
            + self
                .clip_mask_stack
                .iter()
                .filter_map(Option::as_ref)
                .map(Vec::capacity)
                .sum::<usize>()
    }
}

/// 通用栅格渲染器。
///
/// 管理所有渲染状态，绘制方法写入调用者传入的像素缓冲。
pub(crate) struct SoftwareRasterizer {
    /// 当前像素目标宽度，用于构造路径裁剪 mask。
    pub(crate) surface_w: i32,
    /// 当前像素目标高度，用于构造路径裁剪 mask。
    pub(crate) surface_h: i32,
    /// 当前裁剪矩形（浮点）。
    pub(crate) clip_rect: Rect,
    /// 预计算的整数裁剪边界。
    clip_int: (i32, i32, i32, i32),
    /// 裁剪矩形栈，供路径 mask lowering 维护同一 pop 顺序。
    pub(crate) clip_stack: Vec<Rect>,
    /// 当前路径裁剪的逐像素 coverage mask。
    pub(crate) clip_mask: Option<Vec<u8>>,
    /// 每次 push_clip 前的路径 mask，用于统一 pop_clip。
    pub(crate) clip_mask_stack: Vec<Option<Vec<u8>>>,
    /// 全局透明度。
    opacity: f32,
    /// 像素偏移量（画布平移）。
    pub(crate) offset_x: f32,
    pub(crate) offset_y: f32,
    /// 当前 2D 仿射变换。
    pub(crate) transform: Transform,
    /// 逆变换缓存。
    invert: Option<[f64; 6]>,
    /// 混合模式。
    blend_mode: BlendMode,
    /// 状态快照栈。
    state_stack: Vec<StateSnapshot>,
    /// 当前活动的 save 深度；state_stack 同时充当可复用槽池。
    state_depth: usize,
}

impl SoftwareRasterizer {
    /// 创建新渲染器，默认全屏裁剪、identity 变换。
    pub(crate) fn new(surface_w: i32, surface_h: i32) -> Self {
        // 裁剪计算统一使用至少 1x1 的安全目标尺寸。
        let surface_w = surface_w.max(1);
        let surface_h = surface_h.max(1);
        Self {
            surface_w,
            surface_h,
            clip_rect: Rect::new(0.0, 0.0, surface_w as f32, surface_h as f32),
            clip_int: (0, 0, surface_w, surface_h),
            clip_stack: Vec::new(),
            clip_mask: None,
            clip_mask_stack: Vec::new(),
            opacity: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            transform: Transform::identity(),
            invert: Self::compute_inverse(&Transform::identity()),
            blend_mode: BlendMode::default(),
            state_stack: Vec::new(),
            state_depth: 0,
        }
    }

    /// 重置瞬态画布状态，同时保留稳态裁剪与快照栈容量。
    pub(crate) fn reset_for_extent(&mut self, surface_w: i32, surface_h: i32) {
        let surface_w = surface_w.max(1);
        let surface_h = surface_h.max(1);
        self.surface_w = surface_w;
        self.surface_h = surface_h;
        self.clip_rect = Rect::new(0.0, 0.0, surface_w as f32, surface_h as f32);
        self.clip_int = (0, 0, surface_w, surface_h);
        clear_reusable_stack(&mut self.clip_stack);
        self.clip_mask = None;
        clear_reusable_stack(&mut self.clip_mask_stack);
        self.opacity = 1.0;
        self.offset_x = 0.0;
        self.offset_y = 0.0;
        self.transform = Transform::identity();
        self.invert = Self::compute_inverse(&self.transform);
        self.blend_mode = BlendMode::default();
        self.state_depth = 0;
        for snapshot in &mut self.state_stack {
            snapshot.release_payload_for_reuse();
        }
        self.trim_snapshot_pool();
    }

    #[cfg(test)]
    pub(crate) fn transient_stack_capacities(&self) -> (usize, usize, usize) {
        (
            self.clip_stack.capacity(),
            self.clip_mask_stack.capacity(),
            self.state_stack.capacity(),
        )
    }

    #[cfg(test)]
    pub(crate) fn snapshot_slot_count(&self) -> usize {
        self.state_stack.len()
    }

    #[cfg(test)]
    pub(crate) fn snapshot_clip_stack_allocation(
        &self,
        index: usize,
    ) -> Option<(*const Rect, usize)> {
        self.state_stack
            .get(index)
            .map(|snapshot| (snapshot.clip_stack.as_ptr(), snapshot.clip_stack.capacity()))
    }

    // ═══ 状态访问器 ═══

    pub(crate) fn clip_rect(&self) -> Rect {
        self.clip_rect
    }
    // 告知 recorder 当前裁剪是否包含不能降为矩形 scissor 的 coverage mask。
    pub(crate) fn has_clip_mask(&self) -> bool {
        // mask 存在即要求后续绘制经过共享软件像素入口。
        self.clip_mask.is_some()
    }
    pub(crate) fn opacity(&self) -> f32 {
        self.opacity
    }
    pub(crate) fn offset(&self) -> (f32, f32) {
        (self.offset_x, self.offset_y)
    }
    pub(crate) fn set_offset(&mut self, dx: f32, dy: f32) {
        self.offset_x = dx;
        self.offset_y = dy;
    }
    pub(crate) fn set_transform(&mut self, t: Transform) {
        self.transform = t;
        self.invert = Self::compute_inverse(&t);
    }
    pub(crate) fn transform(&self) -> Transform {
        self.transform
    }

    // ═══ 状态管理 ═══

    pub(crate) fn save(&mut self) {
        if self.state_depth == self.state_stack.len() {
            self.state_stack.push(StateSnapshot {
                clip_rect: self.clip_rect,
                clip_int: self.clip_int,
                clip_mask: self.clip_mask.clone(),
                clip_stack: self.clip_stack.clone(),
                clip_mask_stack: self.clip_mask_stack.clone(),
                opacity: self.opacity,
                offset_x: self.offset_x,
                offset_y: self.offset_y,
                transform: self.transform,
                invert: self.invert,
                blend_mode: self.blend_mode,
            });
        } else {
            let snapshot = &mut self.state_stack[self.state_depth];
            snapshot.clip_rect = self.clip_rect;
            snapshot.clip_int = self.clip_int;
            snapshot.clip_mask.clone_from(&self.clip_mask);
            snapshot.clip_stack.clone_from(&self.clip_stack);
            snapshot.clip_mask_stack.clone_from(&self.clip_mask_stack);
            snapshot.opacity = self.opacity;
            snapshot.offset_x = self.offset_x;
            snapshot.offset_y = self.offset_y;
            snapshot.transform = self.transform;
            snapshot.invert = self.invert;
            snapshot.blend_mode = self.blend_mode;
        }
        self.state_depth += 1;
    }

    pub(crate) fn restore(&mut self) {
        let Some(depth) = self.state_depth.checked_sub(1) else {
            return;
        };
        self.state_depth = depth;
        {
            let snapshot = &mut self.state_stack[depth];
            self.clip_rect = snapshot.clip_rect;
            self.clip_int = snapshot.clip_int;
            std::mem::swap(&mut self.clip_mask, &mut snapshot.clip_mask);
            std::mem::swap(&mut self.clip_stack, &mut snapshot.clip_stack);
            std::mem::swap(&mut self.clip_mask_stack, &mut snapshot.clip_mask_stack);
            self.opacity = snapshot.opacity;
            self.offset_x = snapshot.offset_x;
            self.offset_y = snapshot.offset_y;
            self.transform = snapshot.transform;
            self.invert = snapshot.invert;
            self.blend_mode = snapshot.blend_mode;
            snapshot.release_payload_for_reuse();
        }
        if self.state_depth == 0 {
            self.trim_snapshot_pool();
        }
    }

    fn trim_snapshot_pool(&mut self) {
        let retained = self.state_stack.capacity() * std::mem::size_of::<StateSnapshot>()
            + self
                .state_stack
                .iter()
                .map(StateSnapshot::retained_memory_usage)
                .sum::<usize>();
        if retained > MAX_RETAINED_SNAPSHOT_POOL_BYTES {
            self.state_stack = Vec::new();
        }
    }

    pub(crate) fn push_clip(&mut self, rect: Rect) {
        self.push_clip_surface(self.map_rect(rect));
    }

    pub(crate) fn push_clip_surface(&mut self, rect: Rect) {
        // 统一记录当前路径 mask，让矩形与路径裁剪可以混合嵌套。
        self.clip_stack.push(self.clip_rect);
        self.clip_mask_stack.push(self.clip_mask.clone());
        if let Some(intersection) = self.clip_rect.intersect(&rect) {
            self.clip_rect = intersection;
            self.sync_clip_int();
        } else {
            self.clip_rect = Rect::zero();
            self.clip_int = (0, 0, 0, 0);
        }
    }

    pub(crate) fn pop_clip(&mut self) {
        if let Some(prev) = self.clip_stack.pop() {
            self.clip_rect = prev;
            self.sync_clip_int();
        }
        // 没有对应 mask 时保持旧的矩形裁剪行为。
        if let Some(mask) = self.clip_mask_stack.pop() {
            self.clip_mask = mask;
        }
    }

    pub(crate) fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    pub(crate) fn set_blend_mode(&mut self, mode: BlendMode) {
        self.blend_mode = mode;
    }

    // ═══ 变换工具 ═══

    pub(crate) fn is_identity(t: &Transform) -> bool {
        t.m[0] == 1.0
            && t.m[1] == 0.0
            && t.m[2] == 0.0
            && t.m[3] == 0.0
            && t.m[4] == 1.0
            && t.m[5] == 0.0
    }

    fn compute_inverse(t: &Transform) -> Option<[f64; 6]> {
        let [a, b, tx, c, d, ty] = t.m.map(|v| v as f64);
        let det = a * d - b * c;
        if det.abs() < 1e-12 {
            return None;
        }
        let inv = 1.0 / det;
        Some([
            inv * d,
            inv * (-b),
            inv * (b * ty - d * tx),
            inv * (-c),
            inv * a,
            inv * (c * tx - a * ty),
        ])
    }

    fn apply_transform(&self, x: f32, y: f32) -> (f32, f32) {
        let [a, b, tx, c, d, ty] = self.transform.m;
        (a * x + b * y + tx, c * x + d * y + ty)
    }

    pub(crate) fn apply_inverse(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        self.invert.map(|[a, b, tx, c, d, ty]| {
            let xf = x as f64;
            let yf = y as f64;
            ((a * xf + b * yf + tx) as f32, (c * xf + d * yf + ty) as f32)
        })
    }

    // 路径 mask 更新 clip AABB 后重新计算整数 scissor。
    pub(crate) fn sync_clip_int(&mut self) {
        self.clip_int = rast::clip_to_int(&self.clip_rect);
    }

    pub(crate) fn transform_rect(&self, r: &Rect) -> Rect {
        let (x1, y1) = self.apply_transform(r.x, r.y);
        let (x2, y2) = self.apply_transform(r.x + r.w, r.y);
        let (x3, y3) = self.apply_transform(r.x, r.y + r.h);
        let (x4, y4) = self.apply_transform(r.x + r.w, r.y + r.h);
        let min_x = x1.min(x2).min(x3).min(x4);
        let min_y = y1.min(y2).min(y3).min(y4);
        let max_x = x1.max(x2).max(x3).max(x4);
        let max_y = y1.max(y2).max(y3).max(y4);
        Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    pub(crate) fn map_rect(&self, rect: Rect) -> Rect {
        self.transform_rect(&Rect::new(
            rect.x + self.offset_x,
            rect.y + self.offset_y,
            rect.w,
            rect.h,
        ))
    }

    pub(crate) fn intersect_clip(&self, r: &Rect) -> Option<Rect> {
        rast::intersect_rect(r, &self.clip_rect)
    }

    // ═══ 颜色工具 ═══

    pub(crate) fn premul(c: Color) -> u32 {
        rast::premul(c.to_rgba())
    }
    pub(crate) fn apply_opa(&self, c: u32) -> u32 {
        rast::apply_opacity(c, self.opacity)
    }

    // ═══ 像素操作 ═══

    fn blend_pixel(&self, pixels: &mut [u32], w: i32, h: i32, x: i32, y: i32, color: u32) {
        let (cx0, cy0, cx1, cy1) = self.clip_int;
        if x < cx0.max(0) || y < cy0.max(0) || x >= cx1.min(w) || y >= cy1.min(h) {
            return;
        }
        let idx = (y * w + x) as usize;
        if idx >= pixels.len() {
            return;
        }
        // 路径 mask 以 premultiplied coverage 作用于整条源颜色。
        let mask = self.clip_mask_value(x, y);
        if mask == 0 {
            return;
        }
        let color = if mask == u8::MAX {
            color
        } else {
            Self::modulate_premultiplied(color, mask)
        };
        let src_a = (color >> 24) & 0xFF;
        if src_a == 0 {
            return;
        }
        let dst = pixels[idx];
        if self.blend_mode == BlendMode::Additive {
            let add = |shift: u32| (((color >> shift) & 0xFF) + ((dst >> shift) & 0xFF)).min(0xFF);
            pixels[idx] = (add(24) << 24) | (add(16) << 16) | (add(8) << 8) | add(0);
            return;
        }
        let dst_a = (dst >> 24) & 0xFF;
        if src_a == 0xFF && dst_a == 0 {
            pixels[idx] = color;
            return;
        }
        let src_b = color & 0xFF;
        let src_g = (color >> 8) & 0xFF;
        let src_r = (color >> 16) & 0xFF;
        let dst_b = dst & 0xFF;
        let dst_g = (dst >> 8) & 0xFF;
        let dst_r = (dst >> 16) & 0xFF;
        let out_a = src_a + dst_a - (src_a * dst_a / 255);
        let out_r = src_r + (dst_r * (255 - src_a) / 255);
        let out_g = src_g + (dst_g * (255 - src_a) / 255);
        let out_b = src_b + (dst_b * (255 - src_a) / 255);
        pixels[idx] = out_a << 24 | out_r << 16 | out_g << 8 | out_b;
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "pixel coordinates and surface bounds form the raster primitive contract"
    )]
    pub(crate) fn put_pixel_aa(
        &self,
        pixels: &mut [u32],
        w: i32,
        h: i32,
        x: i32,
        y: i32,
        premul_color: u32,
        coverage: f32,
    ) {
        let (cx0, cy0, cx1, cy1) = self.clip_int;
        if x < cx0 || y < cy0 || x >= cx1 || y >= cy1 {
            return;
        }
        // coverage 小于 1 时在当前像素处叠加路径 mask；完整 coverage 交给
        // blend_pixel，由它统一处理 mask，避免重复相乘。
        let mask = self.clip_mask_value(x, y);
        if mask == 0 {
            return;
        }
        if coverage >= 1.0 - 1e-6 {
            self.blend_pixel(pixels, w, h, x, y, premul_color);
            return;
        }
        let coverage = coverage * (mask as f32 / 255.0);
        if coverage <= 0.0 {
            return;
        }
        let src_a = ((premul_color >> 24) & 0xFF) as f32;
        if src_a <= 0.0 {
            return;
        }
        let src_r_p = ((premul_color >> 16) & 0xFF) as f32 * coverage;
        let src_g_p = ((premul_color >> 8) & 0xFF) as f32 * coverage;
        let src_b_p = (premul_color & 0xFF) as f32 * coverage;
        let src_a_s = src_a * coverage;
        if x < 0 || x >= w || y < 0 || y >= h {
            return;
        }
        let idx = (y * w + x) as usize;
        if idx >= pixels.len() {
            return;
        }
        let dst = pixels[idx];
        let dst_a = ((dst >> 24) & 0xFF) as f32;
        let dst_r_p = ((dst >> 16) & 0xFF) as f32;
        let dst_g_p = ((dst >> 8) & 0xFF) as f32;
        let dst_b_p = (dst & 0xFF) as f32;
        if self.blend_mode == BlendMode::Additive {
            pixels[idx] = ((src_a_s + dst_a).round().min(255.0) as u32) << 24
                | ((src_r_p + dst_r_p).round().min(255.0) as u32) << 16
                | ((src_g_p + dst_g_p).round().min(255.0) as u32) << 8
                | (src_b_p + dst_b_p).round().min(255.0) as u32;
            return;
        }
        let inv = 1.0 - (src_a_s / 255.0);
        let out_a = src_a_s + dst_a * inv;
        let out_r_p = src_r_p + dst_r_p * inv;
        let out_g_p = src_g_p + dst_g_p * inv;
        let out_b_p = src_b_p + dst_b_p * inv;
        pixels[idx] = (out_a.round() as u32).min(255) << 24
            | (out_r_p.round() as u32).min(255) << 16
            | (out_g_p.round() as u32).min(255) << 8
            | (out_b_p.round() as u32).min(255);
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "span coordinates and surface bounds form the raster primitive contract"
    )]
    pub(crate) fn fill_span(
        &self,
        pixels: &mut [u32],
        w: i32,
        h: i32,
        x: i32,
        y: i32,
        span_w: i32,
        color: u32,
    ) {
        if (color >> 24) == 0xFF
            && self.blend_mode != BlendMode::Additive
            && self.clip_mask.is_none()
        {
            let (cx0, cy0, cx1, cy1) = self.clip_int;
            let x0 = x.max(cx0).max(0);
            let x1 = (x + span_w).min(cx1).min(w);
            if y >= cy0.max(0) && y < cy1.min(h) && x0 < x1 {
                let start = (y * w + x0) as usize;
                pixels[start..start + (x1 - x0) as usize].fill(color);
            }
            return;
        }
        for dx in 0..span_w {
            self.blend_pixel(pixels, w, h, x + dx, y, color);
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "rectangle coordinates and surface bounds form the raster primitive contract"
    )]
    pub(crate) fn fill_rect_raw(
        &self,
        pixels: &mut [u32],
        w: i32,
        h: i32,
        x: i32,
        y: i32,
        rw: i32,
        rh: i32,
        color: u32,
    ) {
        for dy in 0..rh {
            self.fill_span(pixels, w, h, x, y + dy, rw, color);
        }
    }

    // ═══ SDF 工具 ═══

    pub(crate) fn rounded_rect_sdf(
        ux: f32,
        uy: f32,
        r: &Rect,
        rad: &crate::draw::geometry::types::Radius,
    ) -> f32 {
        rast::rounded_rect_sdf(ux, uy, r, rad)
    }
    pub(crate) fn line_segment_sdf(ux: f32, uy: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
        rast::line_segment_sdf(ux, uy, x1, y1, x2, y2)
    }
    pub(crate) fn sdf_to_coverage(sd: f32) -> f32 {
        rast::sdf_to_coverage(sd)
    }

    // 读取当前像素的路径 coverage；没有路径裁剪时保持满 coverage。
    pub(crate) fn clip_mask_value(&self, x: i32, y: i32) -> u8 {
        let Some(mask) = self.clip_mask.as_ref() else {
            return u8::MAX;
        };
        if x < 0 || y < 0 || x >= self.surface_w || y >= self.surface_h {
            return 0;
        }
        let index = (y as usize)
            .saturating_mul(self.surface_w as usize)
            .saturating_add(x as usize);
        mask.get(index).copied().unwrap_or(0)
    }

    // 对 premultiplied AARRGGBB 颜色应用路径 coverage。
    fn modulate_premultiplied(color: u32, coverage: u8) -> u32 {
        let factor = coverage as u32;
        let channel = |shift: u32| ((color >> shift) & 0xFF) * factor / 255;
        (channel(24) << 24) | (channel(16) << 16) | (channel(8) << 8) | channel(0)
    }
}

// draw_box_shadow 等由 Canvas2D trait 默认实现调用 rasterizer，
// SoftwareRasterizer 不重复实现——默认方法已经够用。
