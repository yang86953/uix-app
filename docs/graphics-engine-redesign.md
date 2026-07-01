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

    // ═══════════════════════════════════════════
    // 像素移动（滚动优化）
    // ═══════════════════════════════════════════

    /// 在画布上移动一个矩形区域内的像素（scroll/pan 优化）。
    /// 将 viewport 区域内的像素从 `(viewport.x-dx, viewport.y-dy)` 复制到
    /// `(viewport.x, viewport.y)`，避免全帧重绘。
    /// 调用者只需重绘新暴露的 strip 区域（与滚动方向相反的一侧）。
    /// 默认空操作，CPU 后端通过 RenderingBackend::copy_region 实现。
    fn scroll_region(&mut self, viewport: Rect, dx: f32, dy: f32) {}
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
| 像素移动（滚动优化） | 1 |
| **合计** | **29** |

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
## 六、节能机制总结

| 能力 | 实现 | 节能效果 |
|------|------|---------|
| 帧闲置检测 | `end_frame` 检测 Canvas 无调用 → Idle，不调 `present` | CPU/GPU 休眠 |
| 精确脏区 | `begin_frame(DirtyRects)` 只清除变化区域 | 减少像素写入 |
| 增量叠加 | `begin_frame(Overlay)` 不调用 clear | 零清除开销 |
| 区域复制 | `Canvas2D.scroll_region` → `RenderingBackend.copy_region` | 极大减少光栅化 |
| 管线剪枝 | `FrameGraph` 输入未变+输出无消费→跳过 Pass | 零 Pass 开销 |
| 动画快照 | `old_dirty_rect` 记录动画前后变化→脏区域精确 | 减少残留像素清除 |
| 裁剪跳过 | Canvas 内部，在 clip 外的绘制调用 → 空操作 | 零绘制开销 |
| 后台休眠 | 交平台层（无交互时停渲染循环） | 零功耗 |
