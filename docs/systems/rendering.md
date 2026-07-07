# 渲染系统

← [Main](../Main.md) · 系统 **#7** · 功能域：`draw` · `app`（ScenePaint 桥接）

> ScenePaint 契约绘制；局部重绘上屏。不知具体组件类型。**须符合** [按需零闲置](demand-driven.md) L0/L1/L2（#105、#107、#122、#129）。

## 索引

| 主题 | 章节 | 决策 |
|------|------|------|
| 引擎 | [引擎](#引擎) | #59 #70 |
| 场景与合成 | [场景与合成](#场景与合成) | #82 #122 #129 |
| 失效队列 | [管线与失效](#管线与失效) | #86 #87 #122 #129 |
| 帧渲染 | [FrameRenderer](#framerenderer) | #59 |
| 资源服务 | [字体与图像](#字体与图像) | #42 #65 |
| 空间几何 | [空间几何](#空间几何) | #70 |
| 动画帧 | [动画帧](#动画帧) | #83 |

**关联**：[application](application.md) · [component](component.md) · [theme-style](theme-style.md) · [foundation](foundation.md) · [platform](platform.md) · [demand-driven](demand-driven.md)

---

## 引擎

### 选择与回退（#59）

```text
create_gpu_context → GpuEngine::new
    失败 → gpu.shutdown → SoftwareEngine
```

| 引擎 | 后端 | 特点 |
|------|------|------|
| GpuEngine | GPU swapchain + damage | 默认 |
| SoftwareEngine | CPU 像素缓冲 | 回退 |
| NullEngine | 无操作 | 测试 |

### GraphicsEngine 契约

`begin_frame(UpdateStrategy)` → 绘制 → `end_frame(&DamageRegion)` → `RenderOutcome`。

| UpdateStrategy | 含义 |
|----------------|------|
| `FullRedraw` | 全帧清除重绘 |
| `DirtyRects(rects)` | 局部 clip + 重绘 |

后端不支持 partial → `normalize_strategy` 降级 Full（`pipeline/frame.rs`）。

### Present 规则

| 条件 | 行为 |
|------|------|
| Paint/Composite 脏 + 非 Idle | `Present(damage)` → platform presenter |
| 仅 Layout 脏 | 不 render、不 present |
| 无脏区 | `RenderOutcome::Idle`，跳过 present |

---

## 场景与合成

### ScenePaint

draw 层 trait（`compositor/scene_paint.rs`）；**不依赖 ui**。app 层 `WidgetTree` 实现。

Compositor 只通过 ScenePaint 读树：frame、children、clip、scroll、dirty、paint callback。

### LayerTree

结构同步（`tree_version` 变化时 rebuild）：

| LayerNode | 条件 | 渲染 |
|-----------|------|------|
| **Picture** | `PicturePolicy::Eligible` **且** `node_count≥8` **且** `est_pixels≥65536`（#122、#129） | 离屏光栅化 → blit |
| **ClipRect** | `children_clip` 存在 | Content → clip+scroll → children → AfterChildren |
| **Direct** | 不满足 Picture 条件 | Content → children |

Picture 由框架 **自动推断** Policy（metadata + runtime 合并 #136）+ 自适应阈值；**无**调用方黑名单。详见 [demand-driven · PicturePolicy](demand-driven.md#picturepolicy-自动推断122129) · [component · PicturePolicy 元数据](component.md#picturepolicy-元数据122)。

Build 时子节点按 `z_index` 排序。旧 Picture offscreen 按 widget_id+bounds 复用。

### RenderObjectTree

并行 DisplayList 缓存（`render_object/tree.rs`）：

- 首帧后 Content pass 可 record+replay DisplayList
- `sync(scene)` 刷新 dirty 节点
- 与 LayerTree 协同：`paint_content` 走 cache，AfterChildren 仍 live paint

### Picture 离屏（#82）

```text
ensure_offscreen(w,h)
→ record DisplayList (或 replay cache)
→ render Direct/ClipRect 子树进 offscreen
→ blit 到屏幕 bounds
```

`MAX_OFFSCREEN_RETRY = 8`；失败回退 Direct 绘制。

### 视口裁剪

`viewport_transform.rs`：累积 scroll offset → content rect 投影到屏幕 → 与 dirty_region 求交决定是否绘制。

---

## 管线与失效

### Invalidation 类型

```rust
enum Invalidation {
    Layout(NodeId),
    Paint { id: NodeId, rect: Option<Rect> },  // None = full frame
    Composite { rect: Rect, scroll: Option<ScrollDelta> },
}
```

### InvalidationQueue 行为

| 操作 | 效果 |
|------|------|
| `push(Paint)` | 同 node 矩形 merge |
| `has_layout()` | 是否需 layout  pass |
| `has_paint_or_composite()` | 是否需 render pass |
| `dirty_region()` | → `DirtyRegion`（Paint + Composite rects） |
| `clear()` | 帧末 `reset_dirty` |

WidgetTree（`tree_dirty.rs`）共享 `InvalidationQueueHandle`；State 绑定通过 `invalidate_paint_handle` 跨线程安全入口（仍主线程消费）。

### Damage 流转

```text
InvalidationQueue.dirty_region()
    → FrameRenderer.compute_damage()
    → DamageRegion (padded partial rects)
    → begin_frame(D DirtyRects)
    → end_frame → PresentDamage
    → IPresenter / IGraphicsContext
```

几何类型定义在 `core::damage`（见 [foundation](foundation.md)）。

> **实现注记**：`Invalidation::Composite` 与 `FrameRenderer` 的 scroll_region **memmove** API（`tree.drain_scroll_region_move`、`Canvas2D::scroll_region`）已存在；Wheel、滚动条拖拽与程序化 ScrollView 滚动已写入 Composite exposed strip，并避免整 viewport Paint invalidate。其他滚动来源仍需逐项接入。

---

## FrameRenderer

`render_frame(engine, scene: &ScenePaint, input)` 顺序：

```text
1. 归一化 dirty（首帧 / full_frame / 无 partial 能力 → full；首帧后空 dirty 且无 Composite → Idle）
2. tree_version 变 → LayerTree.build + sweep_orphaned_offscreens
3. LayerTree.update_dirty(scene)
4. RenderObjectTree.sync(scene)
5. scroll_region memmove（如有 Composite scroll）
6. compute_damage → UpdateStrategy
7. engine.begin_frame → Idle? early exit
8. LayerTree.render (Content + focus ring + debug HUD)
9. engine.end_frame(damage)
10. FrameRenderOutput { outcome: Present(damage), inv_source, tree_version }
```

Debug：F12 切换 debug_mode；hover 链边框 + 帧指标 HUD。

> **实现注记**：`FrameRenderer` 已在 `rendered_first && dirty_region.is_empty() && scroll_move.is_none()` 时直接返回 `RenderOutcome::Idle`，避免空 dirty 被提升为全帧 present；首帧仍强制 full redraw。

---

## 字体与图像

| 服务 | 职责 |
|------|------|
| **FontService** | 系统字体加载（#65）、glyph 光栅化、TextLayout |
| **ImageService** | 解码、BitmapHandle / ImageSlot 管理 |

文本绘制：`TextBackend` trait；ColorValue + TypographyToken 在 render 时 resolve。

Bitmap 字体（内置）用于 debug / 回退；正常路径走系统字体栈。

---

## 空间几何

| 模块 | 用途 |
|------|------|
| `core::{Point, Size, Rect, EdgeInsets}` | 2D 逻辑坐标 |
| `draw::spatial` | PhysicalBox、Mat4、AABB3D、Ray3D — 3D 命中与变换 |
| `scale_factor` | HiDPI（#70）：布局/绘制逻辑 px；present 前 scale 到物理像素 |

禁止在 draw/ui 重复定义几何类型（AGENTS.md）。

---

## 动画帧

设计（#83）：`AnimationRegistry` 持有 active node 集合；主循环在 `tick_effects` **之前**调用 `tree.update(dt)`，由各 widget 推进 `Animatable`（见 [component · 动画](component.md#动画)、[application · 主循环](application.md#主循环)）。

> **实现注记**：`WidgetAnimation` 能力与 `tree.update(dt)` 已接入；动画更新会按 widget 的 `animation_dirty_rect` 做窄 Paint 标脏，单窗 event loop 在仍有动画时登记下一帧 Registry deadline。`Spin` / `ProgressBar` indeterminate / Dropdown fade / Tooltip fade / Popover fade / Popconfirm fade / Modal / Drawer 内置动画源与多窗运行期 frame drain 已接；其他过渡动画源仍待接。

---

## draw 域模块图

```text
draw/
├── traits/          GraphicsEngine, Canvas2D, TextBackend
├── engine/          SoftwareEngine (cpu)
├── gpu_engine/      GpuEngine
├── pipeline/        InvalidationQueue, FrameRenderer, AnimationRegistry
├── compositor/      ScenePaint, LayerTree, Picture, viewport_transform
├── render_object/   DisplayList cache
├── font/            FontService, text backends
├── image/           ImageService
├── painting/        PaintContext, ThemeTokens
├── spatial/         Mat4, PhysicalBox
└── backend/         RenderBackend 抽象
```

`draw` **不依赖** `ui` 或 `app`；ScenePaint 是唯一向上暴露的树只读接口。
