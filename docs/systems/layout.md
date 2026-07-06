# 布局系统

← [Main](../Main.md) · 系统 **#6** · 功能域：`ui`

> measure 定尺寸；Flex + Grid 排布；Scroll 消化 Wheel。Scroll 失效优先 Composite memmove（#107，见 [demand-driven](demand-driven.md#失效与窄标脏)）。

## 索引

| 主题 | 章节 | 决策 |
|------|------|------|
| 测量 | [测量](#测量) | #29 #38 #103 |
| 盒模型 | [盒模型](#盒模型) | #38 #56 |
| Flex | [Flex 布局](#flex-布局) | #53 #81 |
| Grid | [Grid 布局](#grid-布局) | #53 #67 #84 |
| Scroll | [Scroll](#scroll) | #45 #73 #107 |
| 布局管线 | [布局管线](#布局管线) | #19 |
| Active | [Active 判定](#active-判定) | #19 |

**关联**：[component](component.md) · [theme-style](theme-style.md) · [view-reactive](view-reactive.md) · [rendering](rendering.md) · [demand-driven](demand-driven.md)

---

## 测量

### measure 契约（#29）

每个实现 `WidgetLayout` 的组件提供：

```rust
fn preferred_size(&self, engine: Option<&dyn GraphicsEngine>) -> Size;
```

在父级分配的 **content rect** 约束下测量 intrinsic size。复杂组件可读 engine 做文本测量。

### measure 与 preferred_size（#103）

| | 设计（#29） | 当前实现 |
|---|------------|----------|
| API | `measure(constraints) -> Size` | `preferred_size(engine) -> Size` |
| 约束 | `Constraints { min, max, definite }`（#38） | 父级 content rect + 可选 engine 文本测量 |
| 语义 | 在约束下计算 intrinsic size | 等同 |

重构时将 `preferred_size` 重命名为 `measure` 并显式传入 Constraints（[#103](../decisions.md#d103)）。

### Constraints（#38）

设计规格 `{ min, max, definite }`；当前布局管线通过父级 `Rect` 传递可用空间：

| 概念 | 含义 |
|------|------|
| min | 最小尺寸（flex shrink 下限） |
| max | 最大尺寸（overflow 前 clamp） |
| definite | 主轴有确定长度（如 stretch 填充） |

### LayoutChild

布局引擎输入单元（`layout/engine.rs`）：

```rust
LayoutChild {
    id: WidgetId,
    preferred_size: Size,
    flex_grow, flex_shrink,
    margin: EdgeInsets,
    grid_cell, grid_column_span, grid_row_span,
}
```

由 `child_from_tree(cid, tree)` 从 WidgetTree 构建。

---

## 盒模型

```text
┌─ margin ─────────────────────────────┐
│ ┌─ border ─────────────────────────┐ │
│ │ ┌─ padding ────────────────────┐ │ │
│ │ │         content              │ │ │
│ │ └──────────────────────────────┘ │ │
│ └──────────────────────────────────┘ │
└──────────────────────────────────────┘
```

- `Style.margin` — 布局引擎读取，参与 flex/grid 分配
- `Style.padding` / `border_width: EdgeInsets`（#56）— `BoxModel::content_rect()` 计算内容区
- 四边 border 独立（#43）

几何类型一律来自 `core`（见 [foundation](foundation.md)）。

---

## Flex 布局

v1 支持（#53）。配置来自容器 **Style**（#81）：

| Style 字段 | 对应 |
|------------|------|
| `flex_direction` | Row / Column |
| `flex_wrap` | 换行 |
| `justify_content` | 主轴对齐 |
| `align_items` | 交叉轴对齐 |
| `gap` | 间距 |
| `overflow_content` | 溢出堆叠（不 shrink） |

子项：`flex_grow` / `flex_shrink`（WidgetLayout 默认 0/1）；`align_self` 覆盖容器 align。

### 算法概要（`layout/flex.rs`）

1. 主轴分配 flex-basis（preferred_size）
2. 剩余空间 flex-grow 分配 / 超出 flex-shrink（最多 3 轮 redistribution）
3. 交叉轴 align（Stretch 拉伸至容器高/宽）
4. Wrap 模式多行：每行独立 justify

引擎：`FlexLayout::layout(content_rect, children) -> LayoutOutput`。

---

## Grid 布局

View DSL：`grid([...]).columns([GridTrack::Fr(2.0), GridTrack::Px(120.0)])`（#67）。

### GridTrack（#84）

| 变体 | 行为 |
|------|------|
| `Px(f32)` | 固定像素 |
| `Fr(f32)` | 剩余空间比例 |
| `Auto` | 由内容决定 |

Style 字段：`grid_template_columns/rows`、`grid_gap`。

### 算法概要（`layout/grid.rs`）

1. Auto-place 未指定 cell 的子项（隐式增行）
2. 解析 track 尺寸（Px 固定；Fr/Auto 分剩余空间）
3. 构建 cell 网格坐标
4. 放置子项（支持 span）+ `justify_items` / `align_items`

---

## Scroll

| 机制 | 说明 |
|------|------|
| ScrollView（#45） | 消化 Wheel SystemEvent，更新 scroll offset |
| View `scroll(...).vertical().horizontal()` | #73 DSL |
| `children_clip` | 裁剪子树；参与 ScenePaint viewport |
| `scroll_offset` | 绘制与命中测试坐标变换 |

Scroll 内容区在 `layout_viewports` 阶段单独处理；viewport 祖先不参与 shrink 循环。

Wheel 未被子 Scroll 消费时可 bubble 至父级 Scroll。

> Scroll offset 变更优先标 **`Invalidation::Composite`** + `scroll_region` memmove（#107）；框架在 ScrollView 内 **自动** 写入，App 不介入。

> **实现注记**：Wheel → ScrollView 已写入 `Invalidation::Composite` 并接 `scroll_region` memmove；其他非 Wheel 滚动来源仍待逐项接入。

---

## 布局管线

`WidgetTree::layout()`（`tree_layout.rs`）— 最多 **10** 轮收敛：

```text
1. layout_children        // 自顶向下分配 frame
2. expand/shrink 内循环   // 容器随子项 grow/shrink
3. layout_viewports       // ScrollView content bounds
4. bind_reactive_widget_states
5. rebuild_widget_overlays
6. reconcile_lifecycle_after_layout
```

结构变更（Reconciler / add_child / remove）→ `push_layout_invalidation` + 向上 `propagate_layout_invalidation`。

**Layout 失效不 present**；仅 Paint/Composite 触发上屏（见 [rendering](rendering.md)）。

---

## Active 判定

组件 **Active**（#19）当：

```text
与祖先 clip/scroll 视口求交后仍有可见像素
OR 在焦点链上（focused 或 focus 祖先）
```

Inactive → Lifecycle inactive；跳过大部分输入语义。

---

## 源码模块

```text
ui/layout/
├── engine.rs      LayoutEngine, LayoutChild, layout 管线入口
├── flex.rs        FlexLayout
├── grid.rs        Grid 轨道与 auto-place
└── box_model.rs   margin / padding / content rect
ui/core/widget/tree_layout.rs   WidgetTree::layout, viewports, overlay rebuild
```

详见 [Main · 源码目录详表](../Main.md#源码目录详表)。
