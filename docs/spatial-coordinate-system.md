# 统一空间坐标系统设计方案

> 版本：v4.0（最终版）
> 日期：2026-06-28
> 状态：定稿 ✓

---

## 目录

1. [核心设计原则](#1-核心设计原则)
2. [坐标系统约定](#2-坐标系统约定)
3. [系统架构](#3-系统架构)
4. [空间核心](#4-空间核心)
5. [2D 零成本路径](#5-2d-零成本路径)
6. [3D 空间路径](#6-3d-空间路径)
7. [物理单位](#7-物理单位)
8. [命中测试](#8-命中测试)
9. [裁剪系统](#9-裁剪系统)
10. [脏区域与增量更新](#10-脏区域与增量更新)
11. [z-index 与 z 坐标](#11-z-index-与-z-坐标)
12. [与现有系统的集成](#12-与现有系统的集成)
13. [API 完整清单](#13-api-完整清单)
14. [实施路径](#14-实施路径)

---

## 1. 核心设计原则

### 原则 1：2D 零成本，3D 按需付出

```
纯 2D UI 场景：
  ctx.fill_rect(rect, color, radius)
    → #[inline(always)]
    → canvas.fill_rect(rect, color, radius)
    → 无矩阵乘法、无条件分支、无额外分配

3D 场景：
  ctx.spatial().fill_rect(box, color, radius)
    → 4×4 MVP 投影
    → 齐次除法
    → canvas.fill_rect
```

**2D 和 3D 路径在 API 层面天然分离**——不需要运行时判断走哪条路。

### 原则 2：SpatialContext 是中立的

不耦合任何渲染方式，只回答「3D 点投影到屏幕哪里」。Canvas2DRenderer 和未来的 MeshRenderer 共享它。

### 原则 3：3D 是完整的，2D 是子集

所有坐标类型、变换、投影都用 3D 定义，2D 场景只是「z=0 + 正交投影」的特殊情况。

---

## 2. 坐标系统约定

### 2.1 坐标系定义

```
坐标系：右手系
  X → 右
  Y → 上（数学标准）  
  Z → 屏幕外（朝向观察者）

屏幕空间：y-down（左上角原点）
  3D → 屏幕的转换自动处理 y-up → y-down

角度：全程使用弧度
  提供便捷方法：deg(°) → rad
```

### 2.2 默认 2D UI 配置

```yaml
投影:     Mat4::orthographic(0, w, h, 0, -1, 1)   # y-down，z 范围 -1 到 1
相机:     identity（固定看向 -Z）
物体平面: z=0
```

### 2.3 默认 3D 配置

```yaml
投影:     Mat4::perspective(45°, aspect, 0.1, 1000)
相机:     Mat4::look_at([0, 0, 5], [0, 0, 0], [0, 1, 0])
单位:     米（世界坐标）
```

---

## 3. 系统架构

### 3.1 最终架构

```
                         ┌──────────────────────┐
                         │     RenderContext     │
                         │                       │
                         │  canvas: &mut Canvas2D│ ← 2D 零成本路径直连
                         │  spatial: SpatialCtx  │ ← 3D 路径
                         └──────┬───────────────┘
                                │
             ┌──────────────────┼──────────────────┐
             │ 2D 零成本路径     │ 3D 空间路径       │
             ▼                  ▼                  ▼
     ┌──────────────┐  ┌────────────────┐  ┌──────────────┐
     │   Canvas2D   │  │ SpatialContext │  │ MeshRenderer │
     │  (纯像素)     │  │  (4×4 MVP)    │  │  (未来)      │
     └──────────────┘  └───────┬────────┘  └──────────────┘
                               │ project()
                               ▼
                       ┌──────────────┐
                       │   Canvas2D   │
                       │  (纯像素)     │
                       └──────────────┘
```

### 3.2 两条路径的代码选择

```rust
// ── 2D 零成本路径（现有 widget 不改）──
// 直接调用 Canvas2D，无任何中间层
ctx.fill_rect(frame, color, None);
ctx.draw_text("hello", pos, color, 14.0);
ctx.text_center("title", rect, color, 16.0);

// ── 3D 空间路径（需要变换/物理单位）──
// 经过 SpatialContext 投影
ctx.spatial().fill_rect(box_3d, color, None);
ctx.spatial().draw_text("hello", pos_3d, color, 14.pt());
```

**两条路径在同一个 RenderContext 中可以混合使用：**

```rust
fn render(&self, _: Rect, ctx: &mut RenderContext, _: &WidgetTree) {
    // 常规 2D 按钮背景（零成本）
    ctx.fill_rect(frame, bg, Some(radius));
    
    // 带 3D 变换的图标（走空间路径）
    ctx.spatial().save();
    ctx.spatial().translate(10.cm(), 5.cm(), 0.cm());
    ctx.spatial().rotate_z(self.angle);
    ctx.spatial().fill_rect(box, color, None);
    ctx.spatial().restore();
    
    // 常规 2D 文字（零成本）
    ctx.draw_text("点击", pos, color, 14.0);
}
```

---

## 4. 空间核心

### 4.1 Vec3 / Vec4

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self { Self { x, y, z } }
    pub const fn zero() -> Self { Self { x: 0.0, y: 0.0, z: 0.0 } }
    
    pub fn dot(&self, rhs: &Self) -> f32 { self.x * rhs.x + self.y * rhs.y + self.z * rhs.z }
    pub fn cross(&self, rhs: &Self) -> Self {
        Self {
            x: self.y * rhs.z - self.z * rhs.y,
            y: self.z * rhs.x - self.x * rhs.z,
            z: self.x * rhs.y - self.y * rhs.x,
        }
    }
    pub fn length(&self) -> f32 { self.dot(self).sqrt() }
    pub fn normalized(&self) -> Self { let l = self.length(); Self::new(self.x/l, self.y/l, self.z/l) }
}

impl Mul<f32> for Vec3 { type Output = Self; fn mul(self, s: f32) -> Self { Self::new(self.x*s, self.y*s, self.z*s) } }
impl Add for Vec3 { type Output = Self; fn add(self, rhs: Self) -> Self { Self::new(self.x+rhs.x, self.y+rhs.y, self.z+rhs.z) } }
impl Sub for Vec3 { type Output = Self; fn sub(self, rhs: Self) -> Self { Self::new(self.x-rhs.x, self.y-rhs.y, self.z-rhs.z) } }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec4 {
    pub x: f32, pub y: f32, pub z: f32, pub w: f32,
}
impl Vec4 {
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self { Self { x, y, z, w } }
    pub fn from_vec3(v: Vec3, w: f32) -> Self { Self::new(v.x, v.y, v.z, w) }
    pub fn to_vec3(&self) -> Vec3 { Vec3::new(self.x, self.y, self.z) }
    pub fn to_vec3_homogeneous(&self) -> Vec3 {
        if self.w != 0.0 && self.w != 1.0 {
            Vec3::new(self.x / self.w, self.y / self.w, self.z / self.w)
        } else {
            self.to_vec3()
        }
    }
}
```

### 4.2 Mat4（4×4 列主序矩阵）

```rust
/// 4×4 列主序矩阵。
///
/// 布局：m[col * 4 + row]
///   m[0]  m[4]  m[8]  m[12]     列主序：第一列在连续内存中
///   m[1]  m[5]  m[9]  m[13]
///   m[2]  m[6]  m[10] m[14]
///   m[3]  m[7]  m[11] m[15]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4(pub [f32; 16]);

impl Mat4 {
    pub fn identity() -> Self {
        Self([
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ])
    }

    pub fn is_identity(&self) -> bool {
        // quick check
        self.0 == [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ]
    }

    // ── 变换构造器 ──

    pub fn translate(x: f32, y: f32, z: f32) -> Self {
        Self([
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            x,   y,   z,   1.0,
        ])
    }

    pub fn rotate_x(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self([
            1.0, 0.0, 0.0, 0.0,
            0.0,   c,   s, 0.0,
            0.0,  -s,   c, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ])
    }

    pub fn rotate_y(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self([
              c, 0.0,  -s, 0.0,
            0.0, 1.0, 0.0, 0.0,
              s, 0.0,   c, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ])
    }

    pub fn rotate_z(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self([
              c,   s, 0.0, 0.0,
             -s,   c, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ])
    }

    pub fn scale(x: f32, y: f32, z: f32) -> Self {
        Self([
            x,   0.0, 0.0, 0.0,
            0.0, y,   0.0, 0.0,
            0.0, 0.0, z,   0.0,
            0.0, 0.0, 0.0, 1.0,
        ])
    }

    // ── 投影构造器 ──

    /// 透视投影（FOV 垂直视角，aspect = w/h）
    pub fn perspective(fov_radians: f32, aspect: f32, near: f32, far: f32) -> Self {
        let f = 1.0 / (fov_radians * 0.5).tan();
        let range_inv = 1.0 / (near - far);
        Self([
            f / aspect, 0.0, 0.0, 0.0,
            0.0,        f,   0.0, 0.0,
            0.0, 0.0, (near + far) * range_inv, -1.0,
            0.0, 0.0, near * far * 2.0 * range_inv, 0.0,
        ])
    }

    /// 正交投影（y-down，适合 2D UI）
    pub fn orthographic(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self {
        let rml = right - left;
        let tmb = top - bottom;
        let fmn = far - near;
        Self([
            2.0 / rml, 0.0, 0.0, 0.0,
            0.0, 2.0 / tmb, 0.0, 0.0,
            0.0, 0.0, -2.0 / fmn, 0.0,
            -(right + left) / rml, -(top + bottom) / tmb, -(far + near) / fmn, 1.0,
        ])
    }

    /// 视图矩阵：从相机位置看向目标点
    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let f = (target - eye).normalized();
        let s = f.cross(&up).normalized();
        let u = s.cross(&f);
        Self([
             s.x,  u.x, -f.x, 0.0,
             s.y,  u.y, -f.y, 0.0,
             s.z,  u.z, -f.z, 0.0,
            -s.dot(&eye), -u.dot(&eye), f.dot(&eye), 1.0,
        ])
    }

    // ── 矩阵运算 ──

    /// 矩阵乘法：self × rhs
    pub fn mul(&self, rhs: &Self) -> Self {
        let a = &self.0;
        let b = &rhs.0;
        let mut r = [0.0f32; 16];
        for col in 0..4 {
            for row in 0..4 {
                r[col * 4 + row] =
                    a[0 * 4 + row] * b[col * 4 + 0] +
                    a[1 * 4 + row] * b[col * 4 + 1] +
                    a[2 * 4 + row] * b[col * 4 + 2] +
                    a[3 * 4 + row] * b[col * 4 + 3];
            }
        }
        Self(r)
    }

    /// 变换 4D 向量
    pub fn transform_vec4(&self, v: &Vec4) -> Vec4 {
        let m = &self.0;
        Vec4::new(
            m[0]*v.x + m[4]*v.y + m[8]*v.z + m[12]*v.w,
            m[1]*v.x + m[5]*v.y + m[9]*v.z + m[13]*v.w,
            m[2]*v.x + m[6]*v.y + m[10]*v.z + m[14]*v.w,
            m[3]*v.x + m[7]*v.y + m[11]*v.z + m[15]*v.w,
        )
    }

    /// 变换 3D 点（w=1，支持透视）
    pub fn transform_point(&self, p: &Vec3) -> Vec3 {
        self.transform_vec4(&Vec4::new(p.x, p.y, p.z, 1.0)).to_vec3_homogeneous()
    }

    /// 变换 3D 方向向量（w=0，不受平移影响）
    pub fn transform_direction(&self, d: &Vec3) -> Vec3 {
        self.transform_vec4(&Vec4::new(d.x, d.y, d.z, 0.0)).to_vec3()
    }

    /// 逆矩阵（用于命中测试的逆变换）
    pub fn inverse(&self) -> Option<Self> {
        let m = &self.0;
        let a = m[0]*m[5] - m[1]*m[4];
        let b = m[0]*m[6] - m[2]*m[4];
        let c = m[0]*m[7] - m[3]*m[4];
        let d = m[1]*m[6] - m[2]*m[5];
        let e = m[1]*m[7] - m[3]*m[5];
        let f = m[2]*m[7] - m[3]*m[6];
        let g = m[8]*m[13] - m[9]*m[12];
        let h = m[8]*m[14] - m[10]*m[12];
        let i = m[8]*m[15] - m[11]*m[12];
        let j = m[9]*m[14] - m[10]*m[13];
        let k = m[9]*m[15] - m[11]*m[13];
        let l = m[10]*m[15] - m[11]*m[14];
        
        let det = a*l - b*k + c*j + d*i - e*h + f*g;
        if det.abs() < 1e-10 { return None; }
        let inv_det = 1.0 / det;
        
        Some(Self([
            ( m[5]*l - m[6]*k + m[7]*j) * inv_det,
            (-m[1]*l + m[2]*k - m[3]*j) * inv_det,
            ( m[13]*f - m[14]*e + m[15]*d) * inv_det,
            (-m[9]*f  + m[10]*e - m[11]*d) * inv_det,
            (-m[4]*l + m[6]*i - m[7]*h) * inv_det,
            ( m[0]*l - m[2]*i + m[3]*h) * inv_det,
            (-m[12]*f + m[14]*c - m[15]*b) * inv_det,
            ( m[8]*f  - m[10]*c + m[11]*b) * inv_det,
            ( m[4]*k - m[5]*i + m[7]*g) * inv_det,
            (-m[0]*k + m[1]*i - m[3]*g) * inv_det,
            ( m[12]*e - m[13]*c + m[15]*a) * inv_det,
            (-m[8]*e  + m[9]*c  - m[11]*a) * inv_det,
            (-m[4]*j + m[5]*h - m[6]*g) * inv_det,
            ( m[0]*j - m[1]*h + m[2]*g) * inv_det,
            (-m[12]*d + m[13]*b - m[14]*a) * inv_det,
            ( m[8]*d  - m[9]*b  + m[10]*a) * inv_det,
        ]))
    }

    /// 行列式
    pub fn determinant(&self) -> f32 {
        // 会被 inverse 调用，精简实现
        self.inverse(); // 仅用于演示，实际实现需要分离
        unimplemented!("见 inverse 中的 det 计算")
    }

    // ── 查询 ──

    /// 是否为正交投影矩阵
    pub fn is_orthographic(&self) -> bool {
        // 正交投影的 m[3]==0, m[7]==0, m[11]==0, m[15]==1
        // 透视投影的 m[11] == -1 或 m[15] == 0
        (self.0[3] - 0.0).abs() < 1e-6
            && (self.0[7] - 0.0).abs() < 1e-6
            && (self.0[15] - 1.0).abs() < 1e-6
    }

    /// 当前矩阵是否仅包含 2D 变换（z 方向无旋转/缩放）
    pub fn is_2d_only(&self) -> bool {
        // 检查 z 轴相关分量是否为单位变换
        (self.0[2] - 0.0).abs() < 1e-6  && (self.0[6] - 0.0).abs() < 1e-6
            && (self.0[8] - 0.0).abs() < 1e-6  && (self.0[9] - 0.0).abs() < 1e-6
            && (self.0[10] - 1.0).abs() < 1e-6 && (self.0[11] - 0.0).abs() < 1e-6
            && (self.0[14] - 0.0).abs() < 1e-6
    }
}

impl Mul for Mat4 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self { self.mul(&rhs) }
}

/// 从现有 3×2 Transform 的转换
impl From<crate::Transform> for Mat4 {
    fn from(t: crate::Transform) -> Self {
        Self([
            t.m[0], t.m[3], 0.0, 0.0,
            t.m[1], t.m[4], 0.0, 0.0,
            0.0,    0.0,    1.0, 0.0,
            t.m[2], t.m[5], 0.0, 1.0,
        ])
    }
}
```

### 4.3 AABB3D（3D 轴对齐包围盒）

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AABB3D {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB3D {
    pub fn new(min: Vec3, max: Vec3) -> Self { Self { min, max } }

    pub fn from_center(center: Vec3, size: Vec3) -> Self {
        Self {
            min: Vec3::new(center.x - size.x*0.5, center.y - size.y*0.5, center.z - size.z*0.5),
            max: Vec3::new(center.x + size.x*0.5, center.y + size.y*0.5, center.z + size.z*0.5),
        }
    }

    /// 从 2D Rect + z 创建
    pub fn from_rect_z(rect: Rect, z: f32, d: f32) -> Self {
        Self {
            min: Vec3::new(rect.x, rect.y, z),
            max: Vec3::new(rect.x + rect.w, rect.y + rect.h, z + d),
        }
    }

    /// 8 个顶点
    pub fn corners(&self) -> [Vec3; 8] {
        let (min, max) = (self.min, self.max);
        [
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(min.x, max.y, max.z),
            Vec3::new(max.x, max.y, max.z),
        ]
    }

    /// 是否包含 3D 点
    pub fn contains(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x
            && p.y >= self.min.y && p.y <= self.max.y
            && p.z >= self.min.z && p.z <= self.max.z
    }
}
```

### 4.4 Quad2D（投影后的四边形）

```rust
/// 屏幕空间的四边形（3D AABB 投影后的形状）。
///
/// 在透视投影下可能不是矩形（梯形/任意四边形）。
#[derive(Debug, Clone, Copy)]
pub struct Quad2D {
    pub p0: Vec2, pub p1: Vec2, pub p2: Vec2, pub p3: Vec2,
}

#[derive(Debug, Clone, Copy)]
pub struct Vec2 {
    pub x: f32, pub y: f32,
}

impl Quad2D {
    /// 外接矩形
    pub fn bounds(&self) -> Rect {
        let xs = [self.p0.x, self.p1.x, self.p2.x, self.p3.x];
        let ys = [self.p0.y, self.p1.y, self.p2.y, self.p3.y];
        let (min_x, max_x) = (xs.iter().cloned().fold(f32::MAX, f32::min), xs.iter().cloned().fold(f32::MIN, f32::max));
        let (min_y, max_y) = (ys.iter().cloned().fold(f32::MAX, f32::min), ys.iter().cloned().fold(f32::MIN, f32::max));
        Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    /// 转为多边形路径（用于精确填充）
    pub fn to_path(&self) -> Vec<Vec2> {
        vec![self.p0, self.p1, self.p2, self.p3]
    }
    
    /// 是否包含 2D 点
    pub fn contains(&self, p: Vec2) -> bool {
        // 点在凸四边形内的判断：点在四条边的同侧
        fn sign(p: Vec2, a: Vec2, b: Vec2) -> f32 {
            (p.x - b.x) * (a.y - b.y) - (a.x - b.x) * (p.y - b.y)
        }
        let d1 = sign(p, self.p0, self.p1);
        let d2 = sign(p, self.p1, self.p2);
        let d3 = sign(p, self.p2, self.p3);
        let d4 = sign(p, self.p3, self.p0);
        let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0) || (d4 < 0.0);
        let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0) || (d4 > 0.0);
        !(has_neg && has_pos)
    }
}
```

### 4.5 Ray3D（命中测试用）

```rust
/// 3D 射线，从屏幕点出发进入 3D 场景
#[derive(Debug, Clone, Copy)]
pub struct Ray3D {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray3D {
    pub fn new(origin: Vec3, direction: Vec3) -> Self { Self { origin, direction } }

    /// 射线与 AABB 的相交测试（用于命中测试）
    pub fn intersects_aabb(&self, aabb: &AABB3D) -> bool {
        //  slabs method
        let inv_dir = Vec3::new(1.0/self.direction.x, 1.0/self.direction.y, 1.0/self.direction.z);
        let t1 = (aabb.min - self.origin) * inv_dir;
        let t2 = (aabb.max - self.origin) * inv_dir;
        let tmin = t1.x.min(t2.x).max(t1.y.min(t2.y)).max(t1.z.min(t2.z));
        let tmax = t1.x.max(t2.x).min(t1.y.max(t2.y)).min(t1.z.max(t2.z));
        tmax >= 0.0 && tmax >= tmin
    }

    /// 射线与 z=0 平面的交点（2D UI 命中测试用）
    pub fn intersect_z0(&self) -> Option<Vec3> {
        if self.direction.z.abs() < 1e-10 { return None; }
        let t = -self.origin.z / self.direction.z;
        if t < 0.0 { return None; }
        Some(self.origin + self.direction * t)
    }
}

impl Mul<Vec3> for f32 {
    type Output = Vec3;
    fn mul(self, v: Vec3) -> Vec3 { Vec3::new(self*v.x, self*v.y, self*v.z) }
}
```

---

## 5. 2D 零成本路径

### 5.1 核心设计

`RenderContext` 直接持有 `&mut dyn Canvas2D`，2D 方法直接调用它：

```rust
pub struct RenderContext<'a> {
    // ── 2D 零成本路径（直接持有 Canvas2D）──
    canvas: &'a mut dyn Canvas2D,
    
    // ── 3D 空间路径（惰性使用）──
    spatial: SpatialContext<'a>,
    
    // ── 字体与主题 ──
    font: FontHandle,
    font_service: &'a FontService,
    tokens: &'a dyn TokenProvider,
    max_text_width: Cell<f32>,
    debug_mode: bool,
}

impl<'a> RenderContext<'a> {
    /// 2D 零成本路径：直接调用 Canvas2D
    #[inline(always)]
    pub fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        self.canvas.fill_rect(rect, color, radius);
    }

    #[inline(always)]
    pub fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        self.canvas.fill_circle(cx, cy, r, color);
    }

    #[inline(always)]
    pub fn stroke_rect(&mut self, rect: Rect, color: Color, lw: f32, radius: Option<Radius>) {
        self.canvas.stroke_rect(rect, color, lw, radius);
    }

    #[inline(always)]
    pub fn draw_text(&mut self, text: &str, pos: Point, color: Color, font_size: f32) {
        // 跟现在的实现完全一样，直接调用 font_service + canvas
        self.draw_text_impl(text, pos, color, font_size);
    }

    #[inline(always)]
    pub fn text_center(&mut self, text: &str, rect: Rect, color: Color, font_size: f32) {
        // 跟现在的实现完全一样
        self.text_center_impl(text, rect, color, font_size);
    }

    #[inline(always)]
    pub fn save(&mut self) { self.canvas.save(); }
    #[inline(always)]
    pub fn restore(&mut self) { self.canvas.restore(); }

    /// 3D 空间路径入口
    #[inline(always)]
    pub fn spatial(&mut self) -> &mut SpatialContext<'a> {
        &mut self.spatial
    }
}
```

**2D 零成本证明**：

```rust
// 现有代码编译后的汇编（预期）：
ctx.fill_rect(frame, color, None);
// → mov    [rcx], frame.x        ← 直接传参
// → call   Canvas2D::fill_rect   ← 无中间层
// → 跟现在完全一样

ctx.spatial().fill_rect(box, color, None);
// → mov    [rcx], box.x          ← 物理单位转 dip
// → call   Mat4::transform_point ← 4×4 矩阵乘法
// → call   Canvas2D::fill_rect   ← 有额外计算
```

### 5.2 从 RenderContext 到 SpatialContext

`SpatialContext` 持有 Canvas2D 的引用（跟 RenderContext 是同一个 Canvas2D 实例）：

```rust
pub struct SpatialContext<'a> {
    canvas: &'a mut dyn Canvas2D,
    
    // ── 矩阵状态 ──
    matrix_stack: Vec<Mat4>,
    current_matrix: Mat4,
    view_matrix: Mat4,
    projection_matrix: Mat4,
    
    // ── 单位与平台 ──
    dpi: f32,
    device_pixel_ratio: f32,
    orientation: Orientation,
    surface_size: (i32, i32),
}

impl<'a> SpatialContext<'a> {
    // 填充矩形（3D 空间版本）
    pub fn fill_rect(&mut self, rect: impl IntoAABB3D, color: Color, radius: Option<Radius>) {
        let aabb = rect.into_aabb(self.dpi, self.device_pixel_ratio);
        let mvp = self.mvp_matrix();
        
        if mvp.is_2d_only() && mvp.is_orthographic() {
            // 3D 路径中的 2D 子路径：简化计算
            let (x, y) = self.project_2d_internal(&mvp, aabb.min.x, aabb.min.y);
            let (x2, y2) = self.project_2d_internal(&mvp, aabb.max.x, aabb.max.y);
            let (px, py) = self.apply_orientation(x, y);
            let (px2, py2) = self.apply_orientation(x2, y2);
            let r = Rect::new(px, py, px2-px, py2-py).dpr(self.device_pixel_ratio);
            self.canvas.fill_rect(r, color, radius);
        } else {
            // 完整 3D 路径：投影 8 个顶点 → 四边形 → 可选多边形填充
            let quad = self.project_aabb_internal(&mvp, &aabb);
            if radius.is_some() {
                // 有圆角时只能取外接矩形（近似）
                let r = quad.bounds().dpr(self.device_pixel_ratio);
                self.canvas.fill_rect(r, color, radius);
            } else {
                // 无圆角时精确填充四边形
                let points: Vec<Vec2> = quad.to_path();
                // 转为像素坐标
                let pixel_points: Vec<(i32, i32)> = points.iter()
                    .map(|p| ((p.x * self.device_pixel_ratio) as i32, (p.y * self.device_pixel_ratio) as i32))
                    .collect();
                // 使用多边形填充
                self.fill_polygon_impl(&pixel_points, color);
            }
        }
    }
}
```

---

## 6. 3D 空间路径

### 6.1 SpatialContext 核心方法

```rust
impl<'a> SpatialContext<'a> {
    // ════════════════════════════════════════
    // 变换栈
    // ════════════════════════════════════════

    /// 推入变换矩阵（当前矩阵 × mat）
    pub fn push_matrix(&mut self, mat: Mat4) {
        self.matrix_stack.push(self.current_matrix);
        self.current_matrix = self.current_matrix * mat;
    }

    /// 弹出矩阵
    pub fn pop_matrix(&mut self) {
        if let Some(prev) = self.matrix_stack.pop() {
            self.current_matrix = prev;
        }
    }

    pub fn save(&mut self) { self.push_matrix(Mat4::identity()); }
    pub fn restore(&mut self) { self.pop_matrix(); }

    // ── 便捷变换 ──

    pub fn translate(&mut self, x: f32, y: f32, z: f32) {
        self.push_matrix(Mat4::translate(x, y, z));
    }
    pub fn translate_2d(&mut self, x: f32, y: f32) { self.translate(x, y, 0.0); }

    pub fn rotate(&mut self, axis: Vec3, angle: f32) {
        self.push_matrix(Mat4::rotate_axis(axis, angle));
    }
    pub fn rotate_x(&mut self, angle: f32) { self.push_matrix(Mat4::rotate_x(angle)); }
    pub fn rotate_y(&mut self, angle: f32) { self.push_matrix(Mat4::rotate_y(angle)); }
    pub fn rotate_z(&mut self, angle: f32) { self.push_matrix(Mat4::rotate_z(angle)); }

    pub fn scale(&mut self, x: f32, y: f32, z: f32) { self.push_matrix(Mat4::scale(x, y, z)); }

    // ── 相机与投影 ──

    pub fn set_camera_look_at(&mut self, eye: Vec3, target: Vec3, up: Vec3) {
        self.view_matrix = Mat4::look_at(eye, target, up);
    }

    pub fn set_perspective(&mut self, fov: f32, aspect: f32, near: f32, far: f32) {
        self.projection_matrix = Mat4::perspective(fov, aspect, near, far);
    }

    pub fn set_orthographic(&mut self, left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) {
        self.projection_matrix = Mat4::orthographic(left, right, bottom, top, near, far);
    }

    // ════════════════════════════════════════
    // 投影核心
    // ════════════════════════════════════════

    /// MVP 矩阵
    pub fn mvp_matrix(&self) -> Mat4 {
        self.projection_matrix * self.view_matrix * self.current_matrix
    }

    /// 3D 点 → 屏幕像素坐标
    pub fn project(&self, p: &Vec3) -> (f32, f32) {
        let mvp = self.mvp_matrix();
        let ndc = mvp.transform_point(p);
        self.ndc_to_screen(ndc.x, ndc.y)
    }

    /// NDC → 屏幕坐标
    fn ndc_to_screen(&self, ndc_x: f32, ndc_y: f32) -> (f32, f32) {
        let w = self.surface_size.0 as f32;
        let h = self.surface_size.1 as f32;
        let sx = (ndc_x * 0.5 + 0.5) * w;
        let sy = (1.0 - (ndc_y * 0.5 + 0.5)) * h;
        match self.orientation {
            Orientation::YDown => (sx, sy),
            Orientation::YUp   => (sx, h - sy),
        }
    }

    /// AABB → 屏幕四边形
    pub fn project_aabb(&self, aabb: &AABB3D) -> Quad2D {
        let mvp = self.mvp_matrix();
        self.project_aabb_internal(&mvp, aabb)
    }

    fn project_aabb_internal(&self, mvp: &Mat4, aabb: &AABB3D) -> Quad2D {
        let corners = aabb.corners();
        let pts: Vec<Vec2> = corners.iter()
            .map(|p| {
                let ndc = mvp.transform_point(p);
                let (sx, sy) = self.ndc_to_screen(ndc.x, ndc.y);
                Vec2 { x: sx, y: sy }
            })
            .collect();
        // 取 2D 凸包作为四边形（简化：取 AABB 8 顶点投影后的凸包外接）
        // 精确实现需要 convex hull 算法，这里简化
        let (min_x, max_x) = pts.iter().fold((f32::MAX, f32::MIN), |(mn, mx), p| (mn.min(p.x), mx.max(p.x)));
        let (min_y, max_y) = pts.iter().fold((f32::MAX, f32::MIN), |(mn, mx), p| (mn.min(p.y), mx.max(p.y)));
        Quad2D {
            p0: Vec2 { x: min_x, y: min_y },
            p1: Vec2 { x: max_x, y: min_y },
            p2: Vec2 { x: max_x, y: max_y },
            p3: Vec2 { x: min_x, y: max_y },
        }
    }

    /// 2D 简化投影（仅用于 is_2d_only 的矩阵）
    fn project_2d_internal(&self, mvp: &Mat4, x: f32, y: f32) -> (f32, f32) {
        let m = &mvp.0;
        let px = m[0]*x + m[4]*y + m[12];
        let py = m[1]*x + m[5]*y + m[13];
        (px, py)
    }

    // ════════════════════════════════════════
    // 逆变换（命中测试）
    // ════════════════════════════════════════

    /// 屏幕坐标 → 3D 射线
    pub fn unproject(&self, screen_x: f32, screen_y: f32) -> Ray3D {
        let w = self.surface_size.0 as f32;
        let h = self.surface_size.1 as f32;
        let (ndc_x, ndc_y) = match self.orientation {
            Orientation::YDown => (screen_x / w * 2.0 - 1.0, 1.0 - screen_y / h * 2.0),
            Orientation::YUp   => (screen_x / w * 2.0 - 1.0, screen_y / h * 2.0 - 1.0),
        };
        let inv_mvp = self.mvp_matrix().inverse()
            .expect("MVPMatrix must be invertible for hit-testing");
        
        // 近平面点 (ndc_z = -1) 和远平面点 (ndc_z = 1)
        let near = inv_mvp.transform_point(&Vec3::new(ndc_x, ndc_y, -1.0));
        let far  = inv_mvp.transform_point(&Vec3::new(ndc_x, ndc_y, 1.0));
        let dir = (far - near).normalized();
        Ray3D::new(near, dir)
    }

    /// 获取当前变换矩阵的逆矩阵（widget 用于局部命中测试）
    pub fn inverse_current_matrix(&self) -> Option<Mat4> {
        self.current_matrix.inverse()
    }

    // ════════════════════════════════════════
    // 绘制方法
    // ════════════════════════════════════════

    pub fn fill_rect(&mut self, rect: impl IntoAABB3D, color: Color, radius: Option<Radius>) {
        // ... 见 5.2 节实现
    }

    pub fn fill_circle(&mut self, center: Vec3, r: PhysicalUnit, color: Color) {
        let (sx, sy) = self.project(&center);
        let sr = r.to_dip(self.dpi) * self.device_pixel_ratio;
        self.canvas.fill_circle(sx, sy, sr, color);
    }

    pub fn draw_text(&mut self, text: &str, pos: Vec3, color: Color, font_size: PhysicalUnit) {
        let (sx, sy) = self.project(&pos);
        let fs = font_size.to_dip(self.dpi) * self.device_pixel_ratio;
        // 调用 font_service + canvas.blit_glyph
        self.draw_text_impl(text, sx, sy, color, fs);
    }
}
```

---

## 7. 物理单位

### 7.1 PhysicalUnit

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhysicalUnit {
    Px(f32),
    Mm(f32),
    Cm(f32),
    M(f32),
    Pt(f32),
    Inch(f32),
}

impl PhysicalUnit {
    /// 转换到逻辑像素
    pub fn to_dip(&self, dpi: f32) -> f32 {
        match self {
            PhysicalUnit::Px(v)   => *v,
            PhysicalUnit::Mm(v)   => v * (dpi / 25.4),
            PhysicalUnit::Cm(v)   => v * (dpi / 2.54),
            PhysicalUnit::M(v)    => v * (dpi / 0.0254),
            PhysicalUnit::Pt(v)   => v * (dpi / 72.0),
            PhysicalUnit::Inch(v) => v * dpi,
        }
    }

    pub fn value(&self) -> f32 {
        match self { PhysicalUnit::Px(v)|PhysicalUnit::Mm(v)|PhysicalUnit::Cm(v)|PhysicalUnit::M(v)|PhysicalUnit::Pt(v)|PhysicalUnit::Inch(v) => *v }
    }

    pub fn unit_name(&self) -> &'static str {
        match self { PhysicalUnit::Px(_) => "px", PhysicalUnit::Mm(_) => "mm", PhysicalUnit::Cm(_) => "cm", PhysicalUnit::M(_) => "m", PhysicalUnit::Pt(_) => "pt", PhysicalUnit::Inch(_) => "inch" }
    }
}

pub trait PhysicalUnitExt: Into<f32> {
    fn px(self) -> PhysicalUnit { PhysicalUnit::Px(self.into()) }
    fn mm(self) -> PhysicalUnit { PhysicalUnit::Mm(self.into()) }
    fn cm(self) -> PhysicalUnit { PhysicalUnit::Cm(self.into()) }
    fn m(self)  -> PhysicalUnit { PhysicalUnit::M(self.into()) }
    fn pt(self) -> PhysicalUnit { PhysicalUnit::Pt(self.into()) }
    fn inch(self) -> PhysicalUnit { PhysicalUnit::Inch(self.into()) }
}
impl PhysicalUnitExt for f32 {}
impl PhysicalUnitExt for i32 { fn into(self) -> f32 { self as f32 } }
```

### 7.2 IntoAABB3D trait

```rust
pub trait IntoAABB3D {
    fn into_aabb(&self, dpi: f32, dpr: f32) -> AABB3D;
}

impl IntoAABB3D for Rect {
    fn into_aabb(&self, _dpi: f32, _dpr: f32) -> AABB3D {
        AABB3D::new(Vec3::new(self.x, self.y, 0.0), Vec3::new(self.x+self.w, self.y+self.h, 0.0))
    }
}

impl IntoAABB3D for AABB3D {
    fn into_aabb(&self, _dpi: f32, _dpr: f32) -> AABB3D { *self }
}

/// 物理单位的 3D 盒子
pub struct PhysicalBox {
    pub x: PhysicalUnit, pub y: PhysicalUnit, pub z: PhysicalUnit,
    pub w: PhysicalUnit, pub h: PhysicalUnit, pub d: PhysicalUnit,
}

impl PhysicalBox {
    pub fn new(x: PhysicalUnit, y: PhysicalUnit, z: PhysicalUnit, w: PhysicalUnit, h: PhysicalUnit, d: PhysicalUnit) -> Self { Self { x, y, z, w, h, d } }
    pub fn new_2d(x: PhysicalUnit, y: PhysicalUnit, w: PhysicalUnit, h: PhysicalUnit) -> Self { Self { x, y, z: 0.mm(), w, h, d: 0.mm() } }
}

impl IntoAABB3D for PhysicalBox {
    fn into_aabb(&self, dpi: f32, dpr: f32) -> AABB3D {
        let scale = |u: PhysicalUnit| u.to_dip(dpi) * dpr;
        AABB3D::new(
            Vec3::new(scale(self.x), scale(self.y), scale(self.z)),
            Vec3::new(scale(self.x + self.w), scale(self.y + self.h), scale(self.z + self.d)),
        )
    }
}
```

### 7.3 角度单位

```rust
/// 角度便捷构造
pub trait AngleExt: Into<f32> {
    /// 度 → 弧度
    fn deg(self) -> f32 { self.into() * std::f32::consts::PI / 180.0 }
    /// 弧度
    fn rad(self) -> f32 { self.into() }
}
impl AngleExt for f32 {}
impl AngleExt for i32 { fn into(self) -> f32 { self as f32 } }

// 使用：
// ctx.spatial().rotate_z(45.0.deg());
// ctx.spatial().rotate_x(0.5.rad());
```

---

## 8. 命中测试

### 8.1 整体设计

命中测试需要从屏幕坐标反向变换到 widget 的局部坐标：

```
屏幕点击 (x, y)
    │ unproject()
    ▼
3D 射线 (origin, direction)
    │
    ├─ 与 z=0 平面相交（2D widget）
    │   → 得到 3D 交点
    │   → inverse_current_matrix() → 局部坐标
    │   → widget 用局部坐标做命中测试
    │
    └─ 与 3D AABB 相交（3D 物体）
        → Ray3D::intersects_aabb()
        → 检测命中
```

### 8.2 WidgetTree 的命中测试

```rust
// 修改 WidgetTree::hit_test（当前接收 Point，返回 WidgetId）

impl WidgetTree {
    /// 3D 感知的命中测试
    pub fn hit_test_3d(&self, screen_pos: Point, spatial: &SpatialContext) -> Option<WidgetId> {
        let ray = spatial.unproject(screen_pos.x, screen_pos.y);
        
        // 按 z-index 从高到低遍历
        for &id in self.traverse().iter().rev() {
            let node = self.get(id)?;
            // 每个 widget 提供自己的 3D 空间信息
            if let Some(hit) = node.inner().hit_test_3d(&ray, spatial) {
                if hit { return Some(id); }
            }
        }
        None
    }
}

// Widget trait 新增方法
pub trait Widget {
    // ── 新增：3D 命中测试 ──
    /// 判断射线是否命中本 widget。
    /// 默认实现：将射线经逆变换转到局部空间，检测是否在 frame 内。
    fn hit_test_3d(&self, ray: &Ray3D, spatial: &SpatialContext) -> bool {
        // 默认尝试 z=0 平面相交
        if let Some(hit_point) = ray.intersect_z0() {
            if let Some(inv) = spatial.inverse_current_matrix() {
                let local = inv.transform_point(&hit_point);
                let frame = self.frame();  // WidgetCore::frame
                local.x >= frame.x && local.x <= frame.x + frame.w
                    && local.y >= frame.y && local.y <= frame.y + frame.h
            } else {
                false
            }
        } else {
            false
        }
    }
    
    // ── 保留原有平面命中测试（现有 widget 不改）──
    fn hit_test_frame(&self, actual_frame: Rect) -> Rect {
        actual_frame
    }
}
```

### 8.3 向后兼容

现有 widget 不实现 `hit_test_3d`，走默认实现（z=0 平面相交 + 逆变换），结果跟现在一致。

```rust
// 现有 widget 的 on_event 不改
fn on_event(&mut self, event: &WidgetEvent) -> EventResult {
    match event {
        WidgetEvent::MouseDown { pos, .. } => {
            // pos 已经是局部坐标（WidgetTree 的 hit_test_3d 已做逆变换）
            if self.frame.contains(*pos) {
                // 处理事件
            }
        }
    }
}
```

---

## 9. 裁剪系统

### 9.1 3D 裁剪

```rust
impl SpatialContext {
    /// 推入 3D 裁剪 AABB（投影到屏幕后做裁剪）
    pub fn push_clip_3d(&mut self, aabb: AABB3D) {
        // 投影到屏幕
        let quad = self.project_aabb(&aabb);
        let bounds = quad.bounds().dpr(self.device_pixel_ratio);
        self.canvas.push_clip(bounds);
        self.clip_stack.push(aabb);
    }

    pub fn pop_clip_3d(&mut self) {
        self.clip_stack.pop();
        self.canvas.pop_clip();
    }
}

// ScrollView 的 children_clip 在 3D 下：
impl Widget for ScrollView {
    fn children_clip(&self, frame: Rect) -> Option<Rect> {
        // 2D：返回矩形（不变）
        // 3D：由 SpatialContext 处理
        Some(frame)  // 2D 场景
    }

    // 3D 场景下在 render 中设置裁剪
    fn render(&self, frame: Rect, ctx: &mut RenderContext, tree: &WidgetTree) {
        ctx.spatial().push_clip_3d(AABB3D::from_rect_z(frame, 0.0, 0.0));
        // ... 渲染子节点
        ctx.spatial().pop_clip_3d();
    }
}
```

### 9.2 Canvas2D 的裁剪不变

```rust
// Canvas2D 的裁剪方法不变，始终接收像素 Rect
pub trait Canvas2D {
    fn push_clip(&mut self, rect: Rect);
    fn pop_clip(&mut self);
    fn push_clip_path(&mut self, path: &Path);
}
```

---

## 10. 脏区域与增量更新

### 10.1 3D 脏区域

```rust
/// 3D 脏区域：AABB + 变换上下文
pub struct DirtyRegion3D {
    aabb: AABB3D,
}

impl DirtyRegion3D {
    pub fn new(aabb: AABB3D) -> Self { Self { aabb } }
    
    /// 投影到屏幕空间
    pub fn project(&self, spatial: &SpatialContext) -> Rect {
        let quad = spatial.project_aabb(&self.aabb);
        quad.bounds().dpr(spatial.device_pixel_ratio)
    }
}

// Widget trait 新增
pub trait Widget {
    /// 3D 脏区域（默认等于 frame 转 AABB）
    fn dirty_region_3d(&self, frame: Rect) -> DirtyRegion3D {
        DirtyRegion3D::new(AABB3D::from_rect_z(frame, 0.0, 0.0))
    }
}
```

### 10.2 WidgetTree 脏区域合并

```rust
impl WidgetTree {
    fn collect_dirty_regions(&self, spatial: &SpatialContext) -> Vec<Rect> {
        self.dirty_widgets.iter()
            .filter_map(|&id| {
                let node = self.get(id)?;
                let region = node.inner().dirty_region_3d(node.frame());
                Some(region.project(spatial))
            })
            .collect()
    }
}
```

**2D 场景优化**：当 `is_orthographic_2d()` 时，投影退化为直接的 `Rect` 变换，脏区域计算跟现在一致，无额外开销。

---

## 11. z-index 与 z 坐标

### 11.1 规则

```
正交投影（纯 2D）：
  - 仅 z-index 决定绘制顺序
  - z 坐标无效（所有物体在 z=0 平面）
  - 跟现在完全一致

透视投影（3D）：
  - z 坐标决定深度位置（透视缩放/遮挡）
  - z-index 仅在同 z 值物体间排序
  - 不同 z 的物体按深度排序（z 越小越靠近相机，越后绘制/越上层）
```

### 11.2 实现

```rust
impl WidgetTree {
    /// 3D 感知的遍历顺序
    fn traverse_3d(&self, spatial: &SpatialContext) -> Vec<WidgetId> {
        let mut order = self.traverse();
        if spatial.is_perspective() {
            // 3D 模式：按 z 坐标排序（z 小 = 靠近相机 = 上层）
            // 先画远（z 大）的，再画近（z 小）的
            // 大致的 painters algorithm
            order.sort_by(|&a, &b| {
                let za = self.get(a).map(|n| n.frame().z).unwrap_or(0.0);
                let zb = self.get(b).map(|n| n.frame().z).unwrap_or(0.0);
                zb.partial_cmp(&za).unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        let ia = self.get(a).map(|n| n.z_index()).unwrap_or(0);
                        let ib = self.get(b).map(|n| n.z_index()).unwrap_or(0);
                        ia.cmp(&ib)
                    })
            });
        }
        order
    }
}
```

---

## 12. 与现有系统的集成

### 12.1 RenderContext 完整迁移清单

```
保留的方法（2D 零成本路径，直接委托 Canvas2D）:
  ✅ fill_rect, fill_circle, fill_ellipse, fill_sector, fill_path
  ✅ stroke_rect, stroke_circle, stroke_path, draw_line
  ✅ draw_box_shadow, draw_box_shadow_ambient
  ✅ fill_linear_gradient, fill_radial_gradient
  ✅ draw_text, text_center, draw_text_in_frame, draw_text_wrapped
  ✅ draw_text_baseline, draw_text_with_selection
  ✅ blit_glyph_layout
  ✅ save, restore
  ✅ measure_text, measure_text_wrapped
  ✅ text_hit_test, text_cursor_x
  ✅ selection_rects, visual_center_y
  ✅ apply_style
  ✅ set_font, font, set_max_text_width
  ✅ debug_mode, draw_debug_border, draw_debug_label, draw_debug_frame_info

新增的方法（SpatialContext 路径）:
  ✅ ctx.spatial() → &mut SpatialContext
  ✅ ctx.spatial().fill_rect(PhysicalBox/PhysicalUnit...)
  ✅ ctx.spatial().draw_text(..., PhysicalUnit)
  ✅ ctx.spatial().translate / rotate / scale
  ✅ ctx.spatial().push/pop_matrix
  ✅ ctx.spatial().set_perspective / set_orthographic / set_camera

修改的方法（内部调整，对外不变）:
  ⚠️ 无——所有现有 public API 签名不变
```

### 12.2 Canvas2D 变更

```rust
// 移除的方法：
fn set_transform(&mut self, t: Transform);     // → SpatialContext 接管
fn reset_transform(&mut self);                  // → SpatialContext 接管

// 保留的方法：所有绘制方法、save/restore、clip、opacity、blend、pixels
// 全部签名不变
```

### 12.3 GraphicsEngine 变更

```rust
pub trait GraphicsEngine: 'static {
    fn initialize(&mut self, width: i32, height: i32, dpi: f32, dpr: f32, orientation: Orientation) -> Result<(), Error>;
    // 其余方法不变
}
```

### 12.4 平台坐标适配

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Orientation {
    YDown,  // Windows
    YUp,    // Wayland, macOS Core Graphics
}

pub fn detect_platform_orientation() -> Orientation {
    #[cfg(target_os = "windows")]
    { Orientation::YDown }
    #[cfg(target_os = "linux")]
    { Orientation::YUp }   // Wayland
    #[cfg(target_os = "macos")]
    { Orientation::YUp }
}
```

---

## 13. API 完整清单

### 13.1 模块结构

```
graphics/src/
├── spatial/                        # 空间系统（纯 3D，不耦合渲染）
│   ├── mod.rs                      # 重新导出
│   ├── vec3.rs                     # Vec3, Vec4, Vec2
│   ├── mat4.rs                     # Mat4 (4×4)
│   ├── aabb3d.rs                   # AABB3D
│   ├── quad2d.rs                   # Quad2D
│   ├── ray3d.rs                    # Ray3D
│   ├── unit.rs                     # PhysicalUnit + PhysicalUnitExt + AngleExt
│   ├── box.rs                      # PhysicalBox + IntoAABB3D
│   ├── context.rs                  # SpatialContext
│   ├── platform_adapter.rs         # Orientation detection
│   └── dirty_region.rs             # DirtyRegion3D
│
├── renderer/                       # 渲染器
│   ├── mod.rs
│   └── canvas2d_impl.rs            # Canvas2D trait（从 traits/ 移入）  
│
├── traits/
│   ├── engine.rs                   # GraphicsEngine（微调）
│   ├── rendering_backend.rs        # 不变
│   └── update_strategy.rs          # 不变
│
├── traits/canvas_2d.rs             # ❌ 移出至 renderer/canvas2d_impl.rs
├── types.rs                        # 保留 Transform 等（From<Transform> for Mat4）
└── ... 其余不变
```

### 13.2 现有 Transform 的兼容

```rust
// types.rs 中原有的 Transform (3×2) 保留，但标记为 deprecated
// 新增 From 转换：
impl From<Transform> for Mat4 { /* ... */ }
// 实现 Mat4 到 Transform 的向下转换（需要时）：
impl Mat4 {
    pub fn to_affine_2d(&self) -> Option<Transform> {
        if !self.is_2d_only() { return None; }
        Some(crate::Transform {
            m: [self.0[0], self.0[1], self.0[12], self.0[4], self.0[5], self.0[13]],
        })
    }
}
```

---

## 14. 实施路径

### 阶段 1：基础矩阵 + 向量类型（3-5 天）

**文件**：
- `graphics/src/spatial/mod.rs`
- `graphics/src/spatial/vec3.rs`
- `graphics/src/spatial/mat4.rs`

**完成标志**：
- ✓ Vec3, Vec4 类型 + 基本运算
- ✓ Mat4 4×4 矩阵（identity, translate, rotate_x/y/z, scale）
- ✓ Mat4::perspective, orthographic, look_at
- ✓ 矩阵乘法、transform_point/transform_vec4
- ✓ inverse、is_identity、is_2d_only、is_orthographic
- ✓ From<Transform> for Mat4
- ✓ 单元测试覆盖

### 阶段 2：SpatialContext + 物理单位（3-5 天）

**文件**：
- `graphics/src/spatial/unit.rs`
- `graphics/src/spatial/box.rs`
- `graphics/src/spatial/aabb3d.rs`
- `graphics/src/spatial/quad2d.rs`
- `graphics/src/spatial/ray3d.rs`
- `graphics/src/spatial/context.rs`
- `graphics/src/spatial/platform_adapter.rs`

**完成标志**：
- ✓ PhysicalUnit + 六种单位的 DPI 转换
- ✓ PhysicalBox + IntoAABB3D
- ✓ AABB3D（含 corners/contains/from_rect_z）
- ✓ Quad2D（含 bounds/contains/to_path）
- ✓ Ray3D（含 intersects_aabb/intersect_z0）
- ✓ SpatialContext 变换栈（push/pop/save/restore）
- ✓ setup camera + projection
- ✓ mvp_matrix / project / project_aabb / unproject
- ✓ fill_rect 实现（含 2D 子路径 + 3D 路径）
- ✓ Orientation 检测
- ✓ 单元测试覆盖

### 阶段 3：RenderContext 集成 + 2D 零成本路径（2-3 天）

**文件**：
- `graphics/src/renderer/canvas2d_impl.rs`（从 traits/ 移入并清理）
- `ui/src/render_context.rs`（重构）

**完成标志**：
- ✓ Canvas2D 移除 set_transform/reset_transform
- ✓ RenderContext 直接持有 Canvas2D，2D 方法零成本
- ✓ RenderContext::spatial() 暴露
- ✓ 现有所有 widget 编译通过
- ✓ 示例 demo 运行正常

### 阶段 4：命中测试 + 裁剪 + 脏区域（2-3 天）

**文件**：
- `ui/src/widget/tree_core.rs`（修改 hit_test）
- `ui/src/widget/mod.rs`（Widget trait 增加 hit_test_3d）
- `graphics/src/spatial/dirty_region.rs`

**完成标志**：
- ✓ WidgetTree::hit_test_3d
- ✓ Widget::hit_test_3d 默认实现（z=0 平面）
- ✓ SpatialContext::push_clip_3d / pop_clip_3d
- ✓ DirtyRegion3D + 投影
- ✓ 事件系统在 3D 变换后仍正常工作

### 阶段 5：Demo + 验证（持续）

- 纯 2D UI demo：性能无退化
- 2.5D 翻转卡片 demo
- 3D 场景 demo（数据可视化）
- 高 DPI 验证（96/192/300dpi 输出一致）
- 跨平台验证（Windows/Wayland）

---

## 附录：关键决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 2D/3D API 分离 | `ctx.fill_rect` vs `ctx.spatial().fill_rect` | 2D 零成本，3D 按需；API 自文档化 |
| 矩阵类型 | 统一 4×4 | 2D affine 是子集，一种类型避免转换 |
| 坐标手系 | 右手系，Y-up | 与 OpenGL/WebRender 一致 |
| 角度单位 | 弧度 + `deg()` 便捷 | 内部统一弧度，外部用度方便 |
| 命中测试 | unproject → Ray3D → z=0/intersect | 通用方案，2D/3D 统一 |
| z-index 与 z | 正交 z-index 决定，透视 z 排序 | 符合 2D UI 和 3D 场景的直观预期 |
| 物理单位 | 空间路径专用 | 2D 零成本路径不引入物理单位，避免开销 |
