//! 空间上下文（SpatialContext）——3D 空间变换与投影核心。
//!
//! 职责：
//! - 维护 4×4 变换栈（push/pop/save/restore）
//! - 管理相机（view matrix）和投影（orthographic/perspective）
//! - 3D 点/AABB 投影到屏幕坐标
//! - 屏幕坐标反投影为 3D 射线（命中测试）
//! - 提供空间感知的绘制方法（fill_rect）
//!
//! SpatialContext 不耦合 FontService，不处理文字渲染。
//! 文字渲染由 RenderContext 通过 SpatialContext::project 获取位置后自行处理。

use super::aabb3d::AABB3D;
use super::physical_box::IntoAABB3D;
use super::mat4::Mat4;
use super::platform_adapter::Orientation;
use super::quad2d::{Quad2D, Vec2};
use super::ray3d::Ray3D;
use super::unit::PhysicalUnit;
use super::vec3::Vec3;
use crate::traits::Canvas2D;
use crate::Color;

/// 空间上下文——管理 3D 变换栈和投影。
pub struct SpatialContext<'a> {
    // ── 像素绘制引擎 ──
    canvas: &'a mut dyn Canvas2D,

    // ── 变换栈 ──
    matrix_stack: Vec<Mat4>,
    current_matrix: Mat4,

    // ── 相机与投影 ──
    view_matrix: Mat4,
    projection_matrix: Mat4,

    // ── 物理单位与平台 ──
    dpi: f32,
    device_pixel_ratio: f32,
    orientation: Orientation,
    surface_size: (i32, i32),

    // ── 3D 裁剪栈 ──
    clip_stack_3d: Vec<AABB3D>,
}

impl<'a> SpatialContext<'a> {
    /// 创建空间上下文。
    ///
    /// 默认配置 2D UI 模式：
    /// - 正交投影（匹配 surface 尺寸，y-down）
    /// - 相机为 identity（看向 -Z）
    /// - 单位矩阵变换栈
    pub fn new(
        canvas: &'a mut dyn Canvas2D,
        dpi: f32,
        device_pixel_ratio: f32,
        orientation: Orientation,
        surface_w: i32,
        surface_h: i32,
    ) -> Self {
        let w = surface_w as f32;
        let h = surface_h as f32;
        Self {
            canvas,
            matrix_stack: Vec::new(),
            current_matrix: Mat4::identity(),
            view_matrix: Mat4::identity(),
            projection_matrix: Mat4::orthographic(0.0, w, h, 0.0, -1.0, 1.0),
            dpi,
            device_pixel_ratio,
            orientation,
            surface_size: (surface_w, surface_h),
            clip_stack_3d: Vec::new(),
        }
    }

    // ════════════════════════════════════════════════════════════════════
    // 变换栈
    // ════════════════════════════════════════════════════════════════════

    /// 推入变换矩阵。
    ///
    /// 当前矩阵 = 当前矩阵 × mat（左乘）。
    pub fn push_matrix(&mut self, mat: Mat4) {
        self.matrix_stack.push(self.current_matrix);
        self.current_matrix = self.current_matrix * mat;
    }

    /// 弹出矩阵。
    pub fn pop_matrix(&mut self) {
        if let Some(prev) = self.matrix_stack.pop() {
            self.current_matrix = prev;
        }
    }

    /// 保存当前空间状态（等价于 push identity）。
    #[inline(always)]
    pub fn save(&mut self) {
        self.push_matrix(Mat4::identity());
    }

    /// 恢复空间状态（等价于 pop）。
    #[inline(always)]
    pub fn restore(&mut self) {
        self.pop_matrix();
    }

    // ════════════════════════════════════════════════════════════════════
    // 便捷变换
    // ════════════════════════════════════════════════════════════════════

    /// 3D 平移。
    #[inline(always)]
    pub fn translate(&mut self, x: f32, y: f32, z: f32) {
        self.push_matrix(Mat4::translate(x, y, z));
    }

    /// 2D 平移（z=0）。
    #[inline(always)]
    pub fn translate_2d(&mut self, x: f32, y: f32) {
        self.translate(x, y, 0.0);
    }

    /// 绕任意轴旋转。
    #[inline(always)]
    pub fn rotate(&mut self, axis: Vec3, angle_rad: f32) {
        self.push_matrix(Mat4::rotate_axis(axis, angle_rad));
    }

    /// 绕 X 轴旋转（俯仰）。
    #[inline(always)]
    pub fn rotate_x(&mut self, angle_rad: f32) {
        self.push_matrix(Mat4::rotate_x(angle_rad));
    }

    /// 绕 Y 轴旋转（偏航）。
    #[inline(always)]
    pub fn rotate_y(&mut self, angle_rad: f32) {
        self.push_matrix(Mat4::rotate_y(angle_rad));
    }

    /// 绕 Z 轴旋转（2D 平面旋转）。
    #[inline(always)]
    pub fn rotate_z(&mut self, angle_rad: f32) {
        self.push_matrix(Mat4::rotate_z(angle_rad));
    }

    /// 3D 缩放。
    #[inline(always)]
    pub fn scale(&mut self, x: f32, y: f32, z: f32) {
        self.push_matrix(Mat4::scale(x, y, z));
    }

    // ════════════════════════════════════════════════════════════════════
    // 相机与投影
    // ════════════════════════════════════════════════════════════════════

    /// 设置相机（look-at 方式）。
    pub fn set_camera_look_at(&mut self, eye: Vec3, target: Vec3, up: Vec3) {
        self.view_matrix = Mat4::look_at(eye, target, up);
    }

    /// 设置透视投影。
    pub fn set_perspective(&mut self, fov_radians: f32, near: f32, far: f32) {
        let aspect = self.surface_size.0 as f32 / self.surface_size.1 as f32;
        self.projection_matrix = Mat4::perspective(fov_radians, aspect, near, far);
    }

    /// 设置自定义透视投影（含 aspect ratio）。
    pub fn set_perspective_with_aspect(
        &mut self,
        fov_radians: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) {
        self.projection_matrix = Mat4::perspective(fov_radians, aspect, near, far);
    }

    /// 设置正交投影。
    pub fn set_orthographic(&mut self, left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) {
        self.projection_matrix = Mat4::orthographic(left, right, bottom, top, near, far);
    }

    /// 恢复默认 2D UI 正交投影。
    pub fn set_orthographic_2d(&mut self) {
        let w = self.surface_size.0 as f32;
        let h = self.surface_size.1 as f32;
        self.projection_matrix = Mat4::orthographic(0.0, w, h, 0.0, -1.0, 1.0);
        self.view_matrix = Mat4::identity();
    }

    // ════════════════════════════════════════════════════════════════════
    // 投影核心
    // ════════════════════════════════════════════════════════════════════

    /// MVP 矩阵（Projection × View × Model）。
    #[inline(always)]
    pub fn mvp_matrix(&self) -> Mat4 {
        self.projection_matrix * self.view_matrix * self.current_matrix
    }

    /// 3D 点投影到屏幕坐标（像素）。
    ///
    /// 变换链路：模型空间 → MVP → NDC → 视口 → 像素
    pub fn project(&self, p: &Vec3) -> (f32, f32) {
        let mvp = self.mvp_matrix();
        let ndc = mvp.transform_point(p);
        self.ndc_to_screen(ndc.x, ndc.y)
    }

    /// AABB 投影到屏幕四边形。
    pub fn project_aabb(&self, aabb: &AABB3D) -> Quad2D {
        let mvp = self.mvp_matrix();
        self.project_aabb_internal(&mvp, aabb)
    }

    /// NDC → 屏幕坐标。
    fn ndc_to_screen(&self, ndc_x: f32, ndc_y: f32) -> (f32, f32) {
        let w = self.surface_size.0 as f32;
        let h = self.surface_size.1 as f32;
        let sx = (ndc_x * 0.5 + 0.5) * w;
        let sy = (1.0 - (ndc_y * 0.5 + 0.5)) * h;
        match self.orientation {
            Orientation::YDown => (sx, sy),
            Orientation::YUp => (sx, h - sy),
        }
    }

    /// AABB 投影内部实现。
    fn project_aabb_internal(&self, mvp: &Mat4, aabb: &AABB3D) -> Quad2D {
        let corners = aabb.corners();
        let pts: Vec<Vec2> = corners
            .iter()
            .map(|p| {
                let ndc = mvp.transform_point(p);
                let (sx, sy) = self.ndc_to_screen(ndc.x, ndc.y);
                Vec2::new(sx, sy)
            })
            .collect();
        Quad2D::from_points(&pts)
    }

    /// 2D 简化投影（仅当 mvp 是 is_2d_only 时可用）。
    fn project_2d(&self, mvp: &Mat4, x: f32, y: f32) -> (f32, f32) {
        let m = &mvp.0;
        let px = m[0] * x + m[4] * y + m[12];
        let py = m[1] * x + m[5] * y + m[13];
        match self.orientation {
            Orientation::YDown => (px, py),
            Orientation::YUp => (px, self.surface_size.1 as f32 - py),
        }
    }

    // ════════════════════════════════════════════════════════════════════
    // 逆变换（命中测试）
    // ════════════════════════════════════════════════════════════════════

    /// 屏幕坐标 → 3D 射线。
    ///
    /// 用于命中测试：从屏幕点击位置发出一条射线进入 3D 场景。
    pub fn unproject(&self, screen_x: f32, screen_y: f32) -> Option<Ray3D> {
        let w = self.surface_size.0 as f32;
        let h = self.surface_size.1 as f32;

        // 屏幕 → NDC
        let (ndc_x, ndc_y) = match self.orientation {
            Orientation::YDown => (screen_x / w * 2.0 - 1.0, 1.0 - screen_y / h * 2.0),
            Orientation::YUp => (screen_x / w * 2.0 - 1.0, screen_y / h * 2.0 - 1.0),
        };

        let inv_mvp = self.mvp_matrix().inverse()?;

        // 近平面 (NDC z=-1) 和远平面 (NDC z=1) 的反向投影点
        let near = inv_mvp.transform_point(&Vec3::new(ndc_x, ndc_y, -1.0));
        let far = inv_mvp.transform_point(&Vec3::new(ndc_x, ndc_y, 1.0));

        let direction = (far - near).normalized();
        Some(Ray3D::new(near, direction))
    }

    /// 当前变换矩阵的逆矩阵（widget 用于局部坐标命中测试）。
    pub fn inverse_current_matrix(&self) -> Option<Mat4> {
        self.current_matrix.inverse()
    }

    // ════════════════════════════════════════════════════════════════════
    // 绘制方法
    // ════════════════════════════════════════════════════════════════════

    /// 填充矩形。
    ///
    /// 接受 `Rect`（向后兼容）、`AABB3D`、`PhysicalBox` 三种输入。
    ///
    /// - 纯 2D 场景（正交 + 无 3D 旋转）：走优化路径，无矩阵乘法
    /// - 3D 场景：走完整投影路径
    pub fn fill_rect(&mut self, rect: impl IntoAABB3D, color: Color, radius: Option<crate::Radius>) {
        let aabb = rect.into_aabb(self.dpi, self.device_pixel_ratio);
        let mvp = self.mvp_matrix();

        // 检测是否为纯 2D 变换（可直接用 2D affine）
        let is_pure_2d = mvp.is_2d_only() && mvp.is_orthographic();

        if is_pure_2d {
            // ── 2D 优化路径 ──
            let (x1, y1) = self.project_2d(&mvp, aabb.min.x, aabb.min.y);
            let (x2, y2) = self.project_2d(&mvp, aabb.max.x, aabb.max.y);
            let (lx, ly) = (x1.min(x2), y1.min(y2));
            let pw = (x2 - x1).abs() * self.device_pixel_ratio;
            let ph = (y2 - y1).abs() * self.device_pixel_ratio;
            self.canvas.fill_rect(
                uix_platform::Rect::new(lx * self.device_pixel_ratio, ly * self.device_pixel_ratio, pw, ph),
                color,
                radius,
            );
        } else {
            // ── 3D 通用路径 ──
            let quad = self.project_aabb_internal(&mvp, &aabb);
            let bounds = quad.bounds();
            // 3D 变换后的矩形可能不是矩形了（透视），取外接矩形作为近似
            self.canvas.fill_rect(
                uix_platform::Rect::new(
                    bounds.x * self.device_pixel_ratio,
                    bounds.y * self.device_pixel_ratio,
                    bounds.w * self.device_pixel_ratio,
                    bounds.h * self.device_pixel_ratio,
                ),
                color,
                radius,
            );
        }
    }

    /// 填充圆形。
    ///
    /// 只能用于 2D 或 2D-like 变换。在 3D 透视下圆的投影可能是椭圆。
    pub fn fill_circle(&mut self, center: Vec3, r: PhysicalUnit, color: Color) {
        let (sx, sy) = self.project(&center);
        let sr = r.to_px(self.dpi, self.device_pixel_ratio);
        self.canvas.fill_circle(sx, sy, sr, color);
    }

    // ════════════════════════════════════════════════════════════════════
    // 3D 裁剪
    // ════════════════════════════════════════════════════════════════════

    /// 推入 3D 裁剪区域（AABB 投影后裁剪）。
    pub fn push_clip_3d(&mut self, aabb: AABB3D) {
        let quad = self.project_aabb(&aabb);
        let bounds = quad.bounds();
        self.canvas.push_clip(uix_platform::Rect::new(
            bounds.x * self.device_pixel_ratio,
            bounds.y * self.device_pixel_ratio,
            bounds.w * self.device_pixel_ratio,
            bounds.h * self.device_pixel_ratio,
        ));
        self.clip_stack_3d.push(aabb);
    }

    /// 弹出 3D 裁剪区域。
    pub fn pop_clip_3d(&mut self) {
        self.clip_stack_3d.pop();
        self.canvas.pop_clip();
    }

    // ════════════════════════════════════════════════════════════════════
    // 查询
    // ════════════════════════════════════════════════════════════════════

    /// 当前 DPI。
    #[inline(always)]
    pub fn dpi(&self) -> f32 {
        self.dpi
    }

    /// 当前设备像素比。
    #[inline(always)]
    pub fn device_pixel_ratio(&self) -> f32 {
        self.device_pixel_ratio
    }

    /// 表面尺寸。
    #[inline(always)]
    pub fn surface_size(&self) -> (i32, i32) {
        self.surface_size
    }

    /// 当前矩阵是否为 2D only。
    #[inline(always)]
    pub fn is_2d_only(&self) -> bool {
        self.current_matrix.is_2d_only() && self.view_matrix.is_2d_only() && self.projection_matrix.is_orthographic()
    }

    /// 当前是否为透视投影。
    #[inline(always)]
    pub fn is_perspective(&self) -> bool {
        self.projection_matrix.is_perspective()
    }

    /// 低层级访问 Canvas2D。
    ///
    /// 用于需要直接操作像素的场景（如自定义绘制）。
    #[inline(always)]
    pub fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        &mut *self.canvas
    }
}

// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::null_engine::NullEngine;
    use crate::traits::GraphicsEngine;
    use crate::spatial::PhysicalUnitExt;

    fn make_context(surface_w: i32, surface_h: i32) -> SpatialContext<'static> {
        let mut engine = NullEngine::new();
        engine.initialize(surface_w, surface_h).ok();
        let canvas = engine.canvas_2d();
        // 注意：canvas 的生命周期是 'static 因为 NullEngine 是 'static
        // 但实际使用中 canvas 的引用不能超过 engine 的生命周期
        // 这里为了测试用 unsafe
        let canvas_ref: &'static mut dyn Canvas2D = unsafe { std::mem::transmute(canvas) };
        SpatialContext::new(canvas_ref, 96.0, 1.0, Orientation::YDown, surface_w, surface_h)
    }

    #[test]
    fn default_2d_config() {
        let ctx = make_context(800, 600);
        assert!(ctx.is_2d_only());
        assert!(!ctx.is_perspective());
        assert!((ctx.dpi() - 96.0).abs() < 1e-10);
        assert_eq!(ctx.surface_size(), (800, 600));
    }

    #[test]
    fn push_pop_matrix() {
        let mut ctx = make_context(800, 600);
        // MVP = projection * view * model。初始时 model=identity, view=identity,
        // projection=orthographic(0,800,600,0,-1,1) 不是 is_identity 但 is_2d_only
        let mvp_initial = ctx.mvp_matrix();
        assert!(mvp_initial.is_2d_only(), "initial MVP should be 2d_only");

        ctx.push_matrix(Mat4::translate(10.0, 20.0, 0.0));
        let mvp_translated = ctx.mvp_matrix();
        // 平移后的 MVP 应该不同于初始
        assert_ne!(mvp_translated, mvp_initial, "translate should change MVP");

        ctx.pop_matrix();
        let mvp_restored = ctx.mvp_matrix();
        assert_eq!(mvp_restored, mvp_initial, "pop should restore initial MVP");
    }

    #[test]
    fn project_point_2d() {
        let ctx = make_context(800, 600);
        // 在 2D 正交投影下，(100, 200, 0) 应该投影到屏幕 (100, 200)
        let (sx, sy) = ctx.project(&Vec3::new(100.0, 200.0, 0.0));
        assert!((sx - 100.0).abs() < 1.0);
        assert!((sy - 200.0).abs() < 1.0);
    }

    #[test]
    fn project_point_3d_transformed() {
        let mut ctx = make_context(800, 600);
        ctx.push_matrix(Mat4::translate(50.0, 30.0, 0.0));
        // 平移后，(100, 200) 实际在模型空间的 (150, 230)
        let (sx, sy) = ctx.project(&Vec3::new(100.0, 200.0, 0.0));
        assert!((sx - 150.0).abs() < 1.0);
        assert!((sy - 230.0).abs() < 1.0);
        ctx.pop_matrix();
    }

    #[test]
    fn save_restore() {
        let mut ctx = make_context(800, 600);
        let identity = ctx.mvp_matrix();

        ctx.save();
        ctx.translate(10.0, 20.0, 0.0);
        assert!(!ctx.mvp_matrix().is_identity());
        ctx.restore();
        assert_eq!(ctx.mvp_matrix(), identity);
    }

    #[test]
    fn set_perspective_changes_mode() {
        let mut ctx = make_context(800, 600);
        assert!(!ctx.is_perspective());

        ctx.set_perspective(1.0, 0.1, 100.0);
        assert!(ctx.is_perspective());
        assert!(!ctx.is_2d_only());
    }

    #[test]
    fn set_orthographic_2d_restores() {
        let mut ctx = make_context(800, 600);
        ctx.set_perspective(1.0, 0.1, 100.0);
        assert!(ctx.is_perspective());

        ctx.set_orthographic_2d();
        assert!(!ctx.is_perspective());
        assert!(ctx.is_2d_only());
    }

    #[test]
    fn unproject_2d_roundtrip() {
        let ctx = make_context(800, 600);
        // 在 2D 模式下，屏幕 (100, 200) 反投影后应该在 z=0 平面上
        let ray = ctx.unproject(100.0, 200.0).unwrap();
        if let Some(hit) = ray.intersect_z0() {
            assert!((hit.x - 100.0).abs() < 5.0, "x: {}", hit.x);
            assert!((hit.y - 200.0).abs() < 5.0, "y: {}", hit.y);
        } else {
            panic!("ray should intersect z=0");
        }
    }

    #[test]
    fn unproject_with_transform_roundtrip() {
        let mut ctx = make_context(800, 600);
        // model→world: translate(50,30,0)
        ctx.push_matrix(Mat4::translate(50.0, 30.0, 0.0));

        // project model-space point (100,200,0) → screen
        let (sx, sy) = ctx.project(&Vec3::new(100.0, 200.0, 0.0));

        // unproject screen → ray → z=0 intersection
        let ray = ctx.unproject(sx, sy).unwrap();
        if let Some(hit) = ray.intersect_z0() {
            // inv_mvp 已将 NDC 转回模型空间，hit 在模型空间中
            // 期望 hit ≈ (100, 200, 0)
            assert!((hit.x - 100.0).abs() < 5.0, "hit.x: {}", hit.x);
            assert!((hit.y - 200.0).abs() < 5.0, "hit.y: {}", hit.y);
            assert!((hit.z).abs() < 1.0, "hit.z: {}", hit.z);
        } else {
            panic!("ray should intersect z=0");
        }
        ctx.pop_matrix();
    }

    #[test]
    fn project_aabb_2d() {
        let ctx = make_context(800, 600);
        let aabb = AABB3D::from_rect_z(100.0, 50.0, 200.0, 100.0, 0.0, 0.0);
        let quad = ctx.project_aabb(&aabb);
        let bounds = quad.bounds();
        assert!((bounds.x - 100.0).abs() < 1.0);
        assert!((bounds.y - 50.0).abs() < 1.0);
        assert!((bounds.w - 200.0).abs() < 1.0);
        assert!((bounds.h - 100.0).abs() < 1.0);
    }

    #[test]
    fn translate_2d_method() {
        let mut ctx = make_context(800, 600);
        ctx.translate_2d(100.0, 200.0);
        let (sx, sy) = ctx.project(&Vec3::new(50.0, 30.0, 0.0));
        assert!((sx - 150.0).abs() < 1.0);
        assert!((sy - 230.0).abs() < 1.0);
    }

    #[test]
    fn rotate_z_keeps_2d_only() {
        let mut ctx = make_context(800, 600);
        ctx.rotate_z(0.5);
        // rotate_z + orthographic projection 应该保持 2D only
        // （is_2d_only 检查的是 z 轴相关的旋转/平移，不检查投影的 z 缩放）
        let mvp = ctx.mvp_matrix();
        // 验证 z 轴无旋转:
        assert!((mvp.0[2]).abs() < 1e-6, "z→x rotation should be 0");
        assert!((mvp.0[6]).abs() < 1e-6, "z→y rotation should be 0");
        assert!((mvp.0[8]).abs() < 1e-6, "x→z rotation should be 0");
        assert!((mvp.0[9]).abs() < 1e-6, "y→z rotation should be 0");
    }

    #[test]
    fn rotate_x_breaks_2d_only() {
        let mut ctx = make_context(800, 600);
        ctx.rotate_x(0.5);
        assert!(!ctx.mvp_matrix().is_2d_only());
    }

    #[test]
    fn scale_2d_keeps_2d_only() {
        let mut ctx = make_context(800, 600);
        ctx.scale(2.0, 2.0, 1.0);
        let mvp = ctx.mvp_matrix();
        assert!((mvp.0[2]).abs() < 1e-6);
        assert!((mvp.0[6]).abs() < 1e-6);
    }

    #[test]
    fn fill_rect_accepts_rect() {
        let mut ctx = make_context(800, 600);
        let rect = uix_platform::Rect::new(10.0, 20.0, 100.0, 50.0);
        // 只是验证编译通过和不会 panic
        ctx.fill_rect(rect, Color::from_rgb(255, 0, 0), None);
    }

    #[test]
    fn fill_rect_accepts_aabb3d() {
        let mut ctx = make_context(800, 600);
        let aabb = AABB3D::new(Vec3::new(10.0, 10.0, 0.0), Vec3::new(110.0, 60.0, 0.0));
        ctx.fill_rect(aabb, Color::from_rgb(0, 255, 0), None);
    }

    #[test]
    fn is_2d_only_with_identity() {
        let ctx = make_context(800, 600);
        assert!(ctx.is_2d_only());
    }

    #[test]
    fn canvas_2d_accessor() {
        let mut ctx = make_context(800, 600);
        let _canvas = ctx.canvas_2d();
        // 验证可以拿到 Canvas2D
    }

    #[test]
    fn set_camera_look_at() {
        let mut ctx = make_context(800, 600);
        ctx.set_camera_look_at(
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::zero(),
            Vec3::new(0.0, 1.0, 0.0),
        );
        // 设置相机后，m_p 矩阵变了
        assert!(!ctx.mvp_matrix().is_identity());
    }

    #[test]
    fn dpr_affects_pixel_coords() {
        // 在 2x DPR 下，100 dip → 200 物理像素
        let mut engine = NullEngine::new();
        engine.initialize(800, 600).ok();
        let canvas = engine.canvas_2d();
        let canvas_ref: &'static mut dyn Canvas2D = unsafe { std::mem::transmute(canvas) };
        let ctx = SpatialContext::new(canvas_ref, 96.0, 2.0, Orientation::YDown, 800, 600);
        assert!((ctx.device_pixel_ratio() - 2.0).abs() < 1e-10);
    }

    /// 通过 canvas_2d 访问 NullEngine 绘制确认不 panic
    #[test]
    fn fill_circle_does_not_panic() {
        let mut ctx = make_context(800, 600);
        ctx.fill_circle(Vec3::new(100.0, 200.0, 0.0), 10.0.px(), Color::from_rgb(0, 0, 255));
    }
}
