# 图形引擎重构方案 v2

> 基于 2025-06 讨论结论，对 graphics crate 进行架构级重构。
> 核心思路：引擎只负责"给我数据，我画出来"——字体/图片/排版归 UI 层。

---

## 一、架构总览

```
┌─────────────────────────────────────────────────────┐
│  UI 层                                              │
│  ├─ Drawable (Widget 实现，描述视觉外观)              │
│  ├─ FontService (字体加载/排版/字形光栅化)            │
│  ├─ ImageManager (图片解码/管理)                     │
│  └─ Layout / LayerTree (编排 Widget)                 │
└────────────┬────────────────────────────────────────┘
             │ 调用 canvas.draw_*()  /  blit_*()
             ▼
┌─────────────────────────────────────────────────────┐
│  Graphics 层                                        │
│  ├─ GraphicsEngine (生命周期 + 帧控制 + 更新策略)     │
│  │    ├─ canvas_2d() → &mut dyn Canvas2D             │
│  │    └─ canvas_3d() → &mut dyn Canvas3D             │
│  ├─ Canvas2D (2D 绘制原语 + 渲染状态)                │
│  ├─ Canvas3D (3D 渲染接口，先留接口)                  │
│  ├─ RenderingBackend (像素存储 + 呈现 + 区域复制)     │
│  └─ Rasterizer (纯函数，Canvas 默认实现底层委托)      │
└─────────────────────────────────────────────────────┘
```

**核心原则：**

- 引擎不管资源从哪来——给定数据就画出来
- UI 层管所有资源获取（字体/图片/字形）
- 2D 和 3D 各走各的 Canvas，不强行统一
- 后端极简——新后端只需实现 6 个方法
- Canvas 全默认实现，GPU 后端按需覆盖

---

## 二、Trait 定义

### 2.1 UpdateStrategy — 帧更新策略

```rust
/// 帧更新策略，决定清除行为和绘制方式。
pub enum UpdateStrategy {
    /// 全屏清除 + 完整重绘
    FullRedraw,
    /// 精确更新：清除指定区域内旧内容，只重绘这些区域
    DirtyRects(Vec<Rect>),
    /// 增量叠加：不清除，在上一帧内容上叠画新内容
    Overlay(Vec<Rect>),
}
```

**节能语义：**

| 策略 | clear_rect 调用 | 典型场景 |
|------|----------------|---------|
| FullRedraw | 整个 surface | 窗口首次绘制、分辨率变更 |
| DirtyRects | 逐区域清除 | Widget 状态变更、动画单帧 |
| Overlay | 不调用 | 光标闪烁、拖拽预览、通知弹出 |

### 2.2 RenderingBackend — 渲染后端

```rust
/// 渲染后端接口——只管像素在哪、怎么存、怎么呈现。
/// 新后端只需实现 6 个方法。
pub trait RenderingBackend {
    /// 表面尺寸（像素）
    fn surface_size(&self) -> Size;

    /// 只读像素缓冲（CPU 后端有效，GPU 返回空切片）
    fn pixels(&self) -> &[u32];

    /// 可变像素缓冲（CPU 后端有效，GPU panic）
    fn pixels_mut(&mut self) -> &mut [u32];

    /// 清除指定矩形区域为 clear_color
    fn clear_rect(&mut self, rect: Rect, color: Color);

    /// 呈现到屏幕（CPU 空操作，GPU swap buffers）
    fn present(&mut self);

    /// 将 src 区域像素复制到 (dst_x, dst_y)。
    /// GPU 后端可选实现（默认空操作）。
    /// 核心节能手段：列表滚动时避免重绘 90% 像素。
    fn copy_region(&mut self, src: Rect, dst_x: i32, dst_y: i32) {}
}
```

### 2.3 Canvas2D — 2D 绘制能力

```rust
/// 2D 绘制上下文——所有绘制原语 + 渲染状态。
/// 全部方法有默认实现，委托 Rasterizer 纯函数。
pub trait Canvas2D {
    // ═══════════════════════════════════════════
    // 矢量填充
    // ═══════════════════════════════════════════
    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) { ... }
    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) { ... }
    fn fill_ellipse(&mut self, rect: Rect, color: Color) { ... }
    fn fill_sector(&mut self, cx: f32, cy: f32, r: f32, start_angle: f32, end_angle: f32, color: Color) { ... }
    fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) { ... }

    // ═══════════════════════════════════════════
    // 矢量描边
    // ═══════════════════════════════════════════
    fn stroke_rect(&mut self, rect: Rect, color: Color, line_width: f32, radius: Option<Radius>) { ... }
    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, line_width: f32) { ... }
    fn stroke_path(&mut self, path: &Path, color: Color, opts: &StrokeOptions) { ... }
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) { ... }

    // ═══════════════════════════════════════════
    // 渐变
    // ═══════════════════════════════════════════
    fn fill_linear_gradient(&mut self, rect: Rect, color_a: Color, color_b: Color, dir: GradientDirection) { ... }
    fn fill_radial_gradient(&mut self, cx: f32, cy: f32, inner_r: f32, outer_r: f32, inner_color: Color, outer_color: Color) { ... }

    // ═══════════════════════════════════════════
    // 阴影
    // ═══════════════════════════════════════════
    fn draw_box_shadow(&mut self, rect: Rect, blur: f32, ox: f32, oy: f32, color: Color, radius: Option<Radius>) { ... }
    fn draw_box_shadow_ambient(&mut self, rect: Rect, blur: f32, ox: f32, oy: f32, color: Color, radius: Option<Radius>) { ... }

    // ═══════════════════════════════════════════
    // 图像/字形混合（数据由 UI 层提供）
    // ═══════════════════════════════════════════
    /// 将 src 矩形区域缩放绘制到 dst 矩形区域
    fn blit_image(&mut self, src: &[u32], src_w: i32, src_rect: Rect, dst_rect: Rect) { ... }

    /// 将 coverage 位图和颜色混合到 (x, y) 位置
    fn blit_glyph(&mut self, x: i32, y: i32, coverage: &[u8], w: usize, h: usize, color: Color) { ... }

    // ═══════════════════════════════════════════
    // 渲染状态栈
    // ═══════════════════════════════════════════
    fn save(&mut self);
    fn restore(&mut self);
    fn push_clip(&mut self, rect: Rect);
    fn pop_clip(&mut self);
    fn set_opacity(&mut self, opacity: f32);
    fn opacity(&self) -> f32;
    fn set_transform(&mut self, t: Transform);
    fn reset_transform(&mut self);

    // ═══════════════════════════════════════════
    // 混合模式
    // ═══════════════════════════════════════════
    /// 设置当前混合模式。默认 SrcOver。
    /// 用于暗色模式发光、Multiply 叠加等效果。
    fn set_blend_mode(&mut self, mode: BlendMode);

    // ═══════════════════════════════════════════
    // 非矩形裁剪
    // ═══════════════════════════════════════════
    /// 推入 Path 裁剪区域。当前 clip 与 path 取交集。
    /// 用于圆形头像等非矩形裁剪场景。
    fn push_clip_path(&mut self, path: &Path);

    // ═══════════════════════════════════════════
    // 像素访问（内部用，默认实现通过 RenderingBackend）
    // ═══════════════════════════════════════════
    fn pixels_mut(&mut self) -> &mut [u32];
    fn surface_size(&self) -> Size;
    fn current_clip(&self) -> Rect;
}
```

**Canvas2D 方法总计：**

| 类别 | 方法数 |
|------|-------|
| 矢量填充 | 5 |
| 矢量描边 | 4 |
| 渐变 | 2 |
| 阴影 | 2 |
| 图像/字形 | 2 |
| 渲染状态 | 8 |
| 混合模式 | 1 |
| 非矩形裁剪 | 1 |
| 像素访问 | 3 |
| **合计** | **28** |

### 2.4 Canvas3D — 3D 渲染接口（先留接口）

```rust
/// 3D 渲染上下文——保留模式，先留接口。
/// 当前 SoftwareEngine 返回 NotSupported 错误。
pub trait Canvas3D {
    // 顶点/索引缓冲
    fn create_vertex_buffer(&mut self, data: &[u8], layout: &VertexLayout) -> Result<BufferHandle, Error>;
    fn update_vertex_buffer(&mut self, handle: BufferHandle, data: &[u8]);
    fn destroy_buffer(&mut self, handle: BufferHandle);

    // 着色器
    fn create_shader(&mut self, src: &ShaderSource) -> Result<ShaderHandle, Error>;
    fn destroy_shader(&mut self, handle: ShaderHandle);

    // 纹理
    fn create_texture_2d(&mut self, w: u32, h: u32, data: &[u8], fmt: TexFormat) -> Result<TextureHandle, Error>;
    fn destroy_texture(&mut self, handle: TextureHandle);

    // 渲染状态
    fn set_camera(&mut self, camera: &Camera);
    fn set_light(&mut self, index: u32, light: &Light);

    // 提交绘制
    fn draw_mesh(&mut self, vb: BufferHandle, ib: Option<BufferHandle>, shader: ShaderHandle, textures: &[TextureHandle]);
    fn clear_depth(&mut self);
}
```

### 2.5 GraphicsEngine — 引擎编排

```rust
/// 图形引擎 trait——只负责生命周期、帧控制、提供绘制能力入口。
pub trait GraphicsEngine: 'static {
    /// 初始化引擎
    fn initialize(&mut self, w: i32, h: i32) -> Result<(), Error>;

    /// 释放引擎资源（可通过 Drop 实现）
    fn shutdown(&mut self);

    /// 尺寸变更
    fn resize(&mut self, w: i32, h: i32);

    /// 开始帧。strategy 决定清除策略。
    /// 返回 Idle 可跳过本帧 rendering。
    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome;

    /// 结束帧。自动检测本帧是否有绘制调用，无调用返回 Idle。
    fn end_frame(&mut self) -> RenderOutcome;

    /// 获取 2D 绘制上下文
    fn canvas_2d(&mut self) -> &mut dyn Canvas2D;

    /// 获取 3D 绘制上下文（SoftwareEngine 返回 NotSupported）
    fn canvas_3d(&mut self) -> &mut dyn Canvas3D;

    // 可选诊断
    fn memory_usage(&self) -> usize { 0 }
    fn diagnose_memory(&self) { }
}
```

---

## 三、UI 层职责

```rust
/// 任何实现了 Drawable 的东西，都能被图形引擎绘制。
/// 定义在 UI 层，不在 graphics crate。
pub trait Drawable {
    fn draw(&self, canvas: &mut dyn Canvas2D);
}
```

UI 层还负责：
- **FontService** — 字体加载、回退链、文本排版、字形光栅化（coverage → canvas.blit_glyph）
- **ImageManager** — 图片解码、缩放、缓存（像素 → canvas.blit_image）
- **Layout** — 测量、布局计算
- **LayerTree** — 组织 Widget 树、遍历 Drawable

---

## 四、废弃内容（与原方案 v1 对比）

### 从 GraphicsEngine trait 删除的方法：

| 删除方法 | 理由 |
|----------|------|
| `load_font` / `font_service` | 字体归 UI 层 |
| `load_image` / `unload_image` / `image_size` / `draw_image` | 图片归 UI 层 |
| `measure_text` / `draw_glyph_raster` | 字形光栅化归 FontService（UI 层） |
| `fill_rect` / `stroke_rect` / ... (20+ 绘制方法) | 挪到 Canvas2D |
| `push_clip` / `pop_clip` / `save` / `restore` / ... (状态方法) | 挪到 Canvas2D |
| `scroll_region` | 挪到 RenderingBackend.copy_region |
| `clear_surface` / `clear_surface_rect` | 挪到 RenderingBackend.clear_rect + begin_frame 内部 |
| `create_offscreen` / `destroy_offscreen` / `begin_offscreen` / `end_offscreen` | SoftwareEngine 内部实现细节，不暴露 |
| `as_any_mut` / `surface` / `surface_mut` | 通过 RenderingBackend 内部使用 |
| `set_blend_mode` / `supersample_level` 等 | Canvas2D / 构造参数 |

### 废弃的架构概念：

| 概念 | 理由 |
|------|------|
| ResourceManager 统一资源层 | 资源全归 UI 层，引擎不管理资源 |
| AssetStore / ImagePool | 移到 UI 层 |
| FontService 在引擎中 | 移到 UI 层 |
| OffscreenPool 暴露接口 | 离屏降为 SoftwareEngine 内部实现 |
| 5 层架构表述 | 简化为 trait 驱动的极简架构 |

---

## 五、渲染后端实现

### 5.1 SoftwareEngine（CPU）

```rust
pub struct SoftwareEngine {
    surface: PixelSurface,          // 实现 RenderingBackend（持有 Vec<u32>）
    canvas_2d: CpuCanvas2D,         // 实现 Canvas2D（组合 RenderingBackend + 状态栈）
    canvas_3d: NoopCanvas3D,        // 返回 NotSupported
    dirty_rects: Vec<Rect>,
}

/// CPU 像素表面——RenderingBackend 的实现体
struct PixelSurface {
    pixels: Vec<u32>,
    width: i32,
    height: i32,
    clear_color: Color,
}

impl RenderingBackend for PixelSurface { ... }

/// CPU Canvas2D 实现
struct CpuCanvas2D {
    backend: PixelSurface,          // 像素存储
    clip_rect: Rect,
    clip_stack: Vec<Rect>,
    opacity: f32,
    transform: Transform,
    blend_mode: BlendMode,
    clip_path: Option<Path>,        // 非矩形裁剪（与 rect clip 取交集）
    state_stack: Vec<StateSnapshot>,
}

impl Canvas2D for CpuCanvas2D {
    // 默认实现委托 Rasterizer::fill_rect(pixels_mut(), surface_size(), clip, ...)
}
```

### 5.2 GpuEngine（GPU）

```rust
pub struct GpuEngine {
    device: WgpuDevice,
    canvas_2d: GpuCanvas2D,     // 覆盖部分 Canvas2D 方法：用 shader 替代 fill_rect 等
    canvas_3d: GpuCanvas3D,     // 正经实现 Canvas3D
}

/// GPU Canvas2D — 继承大部分 Canvas2D 默认实现，只覆盖要加速的方法
struct GpuCanvas2D {
    device: WgpuDevice,
    render_pass: RenderPass,
    clip: ClipState,
    // ... GPU 特定状态
}

impl Canvas2D for GpuCanvas2D {
    // 覆盖：fill_rect / fill_circle / blit_image — 走 shader
    // 不覆盖：fill_path / stroke_path / blit_glyph — 走 CPU 默认实现
}
```

### 5.3 NullEngine（测试用）

```rust
pub struct NullEngine;
impl GraphicsEngine for NullEngine { /* 全空操作 */ }
```

---

## 六、节能机制总结

| 能力 | 实现 | 节能效果 |
|------|------|---------|
| 帧闲置检测 | `end_frame` 检测 Canvas 无调用 → Idle，不调 `present` | CPU/GPU 休眠 |
| 精确脏区 | `begin_frame(DirtyRects)` 只清除变化区域 | 减少像素写入 |
| 增量叠加 | `begin_frame(Overlay)` 不调用 clear | 零清除开销 |
| 区域复制 | `RenderingBackend.copy_region` 列表滚动 90% 像素免重绘 | 极大减少光栅化 |
| 裁剪跳过 | Canvas 内部，在 clip 外的绘制调用 → 空操作 | 零绘制开销 |
| 后台休眠 | 交平台层（无交互时停渲染循环） | 零功耗 |

---

## 七、与原方案 v1 的关系

| 项目 | v1 方案 | v2 方案 |
|------|---------|---------|
| GraphicsEngine trait 方法数 | ~40 | ~8 |
| 资源管理 | 引擎内 ImagePool + FontService | 全部移到 UI 层 |
| 2D 绘制 | 挂在 GraphicsEngine 上 | 独立 Canvas2D trait |
| 3D 支持 | 无 | Canvas3D trait（先留接口） |
| 后端抽象 | 无 | RenderingBackend（6 方法） |
| 更新策略 | DirtyRegion 单一模型 | UpdateStrategy 三种模型 |
| 混合模式 | 内部自动选择 | Canvas2D.set_blend_mode 暴露 |
| 非矩形裁剪 | 无 | Canvas2D.push_clip_path |
| 架构层数 | 5 层 | trait 驱动，无显式分层 |
| 光栅化纯函数 | 保留（rasterizer/） | 保留（Canvas 默认实现委托） |

---

## 八、执行路线

### 阶段 1：定义新 trait（T1）
**目标**：创建全部 trait 定义，旧代码不动。

```
T1: 创建 graphics/src/traits/ 目录
    ├─ update_strategy.rs   (UpdateStrategy 枚举)
    ├─ rendering_backend.rs (RenderingBackend trait)
    ├─ canvas_2d.rs         (Canvas2D trait + 默认实现)
    ├─ canvas_3d.rs         (Canvas3D trait + NoopCanvas3D)
    └─ graphics_engine.rs   (GraphicsEngine trait)
验证: cargo check -p uix-graphics（不影响旧代码）
```

### 阶段 2：光栅化纯函数化（T2-T3）
**目标**：创建 rasterizer/ 模块，纯函数不持状态。

```
T2: 创建 rasterizer/ 模块（从旧 software_engine/ 迁出绘制逻辑）
    ├─ fill.rs      (fill_rect, fill_circle, fill_ellipse, fill_sector, fill_path)
    ├─ stroke.rs    (stroke_rect, stroke_circle, stroke_path, draw_line)
    ├─ gradient.rs  (fill_linear_gradient, fill_radial_gradient)
    ├─ shadow.rs    (draw_box_shadow, draw_box_shadow_ambient)
    ├─ glyph.rs     (blit_glyph — coverage bitmap 混合)
    ├─ image.rs     (blit_image — src→dst 缩放混合)
    └─ polygon.rs   (fill_polygons — 路径多边形填充)

T3: Canvas2D 默认实现委托 rasterizer 纯函数
验证: cargo check -p uix-graphics
```

### 阶段 3：SoftwareEngine 重构（T4-T5）
**目标**：SoftwareEngine 实现新 trait 体系。

```
T4: 实现 PixelSurface (实现 RenderingBackend)
T5: 实现 CpuCanvas2D (实现 Canvas2D，组合 PixelSurface + 状态栈)
T6: SoftwareEngine 实现 GraphicsEngine（组合 PixelSurface + CpuCanvas2D + NoopCanvas3D）
验证: cargo check -p uix-graphics
```

### 阶段 4：GpuEngine 适配（T7）
```
T7: GpuEngine 实现 GraphicsEngine + GpuCanvas2D（覆盖关键方法）+ GpuCanvas3D（留空）
验证: cargo check -p uix-graphics
```

### 阶段 5：UI 层适配 + 清理（T8-T10）
```
T8: 在 UI 层定义 Drawable trait
T9: FontService / ImageManager 移到 UI 层
T10: 适配 LayerTree 使用新 GraphicsEngine + Canvas2D
验证: cargo check -p uix -p uix-ui
```

### 阶段 6：删除旧代码 + 全量验证（T11-T13）
```
T11: 删除旧 GraphicsEngine trait、废弃的 software_engine 子模块（shapes/shadow/sdf/content/rect/pixels等）
T12: 删除旧 frame.rs（帧逻辑并入 SoftwareEngine）
T13: 更新 lib.rs / api.rs 模块注册
验证: cargo check -p uix-graphics -p uix-ui && cargo test -p uix-graphics -p uix-ui
```

---

## 九、不做的改动（明确边界）

- **不改 platform 层**（IGraphicsContext / EGL / window 管理不碰）
- **不引入新外部依赖**
- **不保留向后兼容**——直接重定义 trait，同步改调用方

---

## 十、Drawable 使用示例

```rust
// UI 层的 Button Widget
impl Drawable for Button {
    fn draw(&self, canvas: &mut dyn Canvas2D) {
        canvas.save();

        // 气泡裁剪
        canvas.push_clip(self.bounds);
        canvas.push_clip_path(&self.rounded_rect_path());

        // 背景渐变
        canvas.fill_linear_gradient(self.bounds, self.top_color, self.bottom_color, GradientDirection::Vertical);

        // 阴影
        canvas.draw_box_shadow(self.bounds, 4.0, 0.0, 2.0, Color::rgba(0,0,0,0.3), Some(self.radius));

        // 文字（字形数据由 FontService 提前光栅化）
        for glyph in &self.glyphs {
            canvas.blit_glyph(glyph.x, glyph.y, &glyph.coverage, glyph.w, glyph.h, self.text_color);
        }

        canvas.restore();
    }
}
```
