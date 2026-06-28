# UI 层面向组件解耦方案

> 日期：2026-06-28
> 基础：docs/api-restructure-report.md 中的 API 重构已完成 platform/graphics 层
> 目标：用面向组件思想彻底解耦 ui 层

---

## 〇、当前架构诊断

### 现状拓扑

```
lib.rs (pub mod 所有模块 + pub use api::*)
  │
  ├── api/            ← traits.rs 空占位，types.rs 仅 re-export
  │
  ├── widget/         ← Widget trait (20+ 方法) + BoxedWidget + WidgetTree
  ├── widgets/        ← 60+ 具体组件（Button/Input/Modal 等）
  │
  ├── layout/         ← LayoutEngine trait + FlexLayout/GridLayout 实现
  ├── animation/      ← Animatable trait + Easing + Animation + Driver
  ├── theme/          ← 5 个 token trait + TokenProvider supertrait
  │
  ├── state/          ← 响应式状态（State/Computed/Effect）
  ├── style/          ← Style 结构体
  │
  ├── render_context/ ← RenderContext（持有 TokenProvider + SpatialContext + FontService）
  ├── managers/       ← 10 个管理器组合为 WidgetManagers
  │
  ├── macros/         ← define_widget! + tree!
  └── 其他辅助        ← children/clipboard/context/focus_trap/layer/locale/virtual_scroll
```

### 核心问题

| # | 问题 | 违反的组件原则 |
|---|------|--------------|
| 1 | **Widget trait 是上帝接口**：20+ 方法塞在一个 trait 里，render/on_event/preferred_size/hit_test_3d 等不同维度的方法强耦合 | 单一职责 |
| 2 | **接口契约未集中**：LayoutEngine / Animatable / Token 系列 trait 散落在各自内部模块，api/traits.rs 空占位 | 接口清晰 |
| 3 | **define_widget! 硬编码依赖 Widget trait**：宏生成 `impl Widget for ...`，Widget trait 的任何变化都要改宏 | 依赖倒置 |
| 4 | **RenderContext 面向具体实现**：直接持有 `&dyn TokenProvider`、`FontService`、`SpatialContext`，没有按接口隔离 | 隐藏内部实现 |
| 5 | **api/types.rs 是大杂烩 re-export**：没有按组件维度分组，外部使用者无法区分"这是哪个组件的能力" | 组合清晰 |

---

## 一、组件领域划分

将 UI 层划分为 8 个组件领域，每个领域有清晰的限界上下文：

```
┌─────────────────────────────────────────────────────┐
│                    UI 层（uix-ui）                    │
│                                                     │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────┐  │
│  │ Widget   │  │ Widgets  │  │ RenderContext    │  │
│  │ 组件系统  │  │ 组件库    │  │ 渲染上下文组件    │  │
│  └────┬─────┘  └────┬─────┘  └────────┬─────────┘  │
│       │              │                 │            │
│  ┌────▼─────┐  ┌────▼─────┐  ┌────────▼─────────┐  │
│  │ Layout   │  │Animation │  │ State            │  │
│  │ 布局组件  │  │ 动画组件  │  │ 状态管理组件      │  │
│  └────┬─────┘  └────┬─────┘  └────────┬─────────┘  │
│       │              │                 │            │
│  ┌────▼──────────────▼─────────────────▼─────────┐  │
│  │ Theme（设计令牌系统 / TokenProvider 系列接口）  │  │
│  └───────────────────────────────────────────────┘  │
│                                                     │
│  ┌───────────────────────────────────────────────┐  │
│  │ Managers（10 个管理器组合，跨组件提供服务）     │  │
│  └───────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

每个组件领域通过 **api/traits.rs** 暴露接口契约，通过 **api/types.rs** 暴露数据契约，内部实现不对外泄漏。

---

## 二、解耦策略（三阶段）

### 阶段一：接口契约集中化（短期，可并行推进）

#### 1.1 迁移纯 trait 到 api/traits.rs

以下 trait 不依赖内部实现细节，应优先迁移：

| Trait | 当前路径 | 目标 | 说明 |
|-------|---------|------|------|
| `LayoutEngine` | `layout/engine.rs` | `api/traits.rs` | 纯 trait，3 个关联类型，无默认方法 |
| `Animatable` | `animation/easing.rs` | `api/traits.rs` | 纯 trait，2 个方法，无内部依赖 |
| `IColorTokens` | `theme/color_tokens.rs` | `api/traits.rs` | 纯 trait，仅依赖 `Color`（graphics 层类型） |
| `ISpacingTokens` | `theme/spacing_tokens.rs` | `api/traits.rs` | 纯 trait，仅依赖 `ShadowToken` |
| `IBoxShadowTokens` | `theme/spacing_tokens.rs` | `api/traits.rs` | 同上 |
| `ITypographyTokens` | `theme/typography_tokens.rs` | `api/traits.rs` | 纯 trait |
| `TokenProvider` | `theme/wrapper.rs` | `api/traits.rs` | supertrait，聚合以上所有 |

#### 1.2 迁移后结构

```
api/
├── mod.rs        ← pub use traits::*; pub use types::*;
├── traits.rs     ← LayoutEngine, Animatable, IColorTokens,
│                    ISpacingTokens, IBoxShadowTokens,
│                    ITypographyTokens, TokenProvider
└── types.rs      ← 按组件维度分组的 re-export
```

内部模块改为：
```rust
// layout/engine.rs 中
// 移除 trait 定义，改用 use crate::api::traits::LayoutEngine;
// 保留 FlexLayout、GridLayout 等实现

// theme/color_tokens.rs 中
// 移除 trait 定义，保留 ShadowToken、Color 等值类型（types.rs 负责 re-export）
```

#### 1.3 ShadowToken 的归属决策

**重要设计决策**：`ShadowToken` 是值类型（struct），不是 trait。当前在 `theme/color_tokens.rs` 中定义，但它是 `ISpacingTokens` 的方法返回类型。

**方案**：ShadowToken 留在 `theme/color_tokens.rs` 中作为值类型，通过 `api/types.rs` re-export 到 crate 根。`api/traits.rs` 中 `ISpacingTokens` 的 method 签名通过 `use super::types::ShadowToken` 引用。

---

### 阶段二：Widget trait 组件化解耦（中期）

#### 2.1 当前 Widget trait 的方法分类

```
┌──────────────────────────────────────────────┐
│                Widget trait                  │
├──────────────────────────────────────────────┤
│  ① 类型系统：as_any, as_any_mut             │
│  ② 布局：preferred_size, flex_grow,         │
│     flex_shrink, layout_children            │
│  ③ 渲染：render, post_render, dirty_rect,   │
│     is_repaint_boundary, children_clip       │
│  ④ 事件：on_event, hit_test_frame,          │
│     hit_test_3d, scroll_delta               │
│  ⑤ 生命周期：on_init, on_mount, on_unmount, │
│     on_update                               │
│  ⑥ 状态：visible, needs_continuous_update   │
│  ⑦ 构建：build                              │
└──────────────────────────────────────────────┘
```

当前有 4 个子 trait（WidgetLayout/WidgetRender/WidgetEventHandler/WidgetLifecycle）通过 blanket impl 自动实现，但这些子 trait 仍然是**从 Widget trait 衍生**的，即所有 widget 仍然自动拥有全部能力。

#### 2.2 组件式 Widget 接口设计

解耦为**可选组件接口**，每个接口代表一种能力：

```rust
/// 组件核心标识 — 所有 widget 必须实现。
/// 仅包含类型系统支持。
pub trait WidgetComponent: AsAny + Send + 'static {}

/// 布局能力 — widget 可选实现。
pub trait WidgetLayout: WidgetComponent {
    fn preferred_size(&self, engine: Option<&dyn GraphicsEngine>) -> Size { Size::zero() }
    fn flex_grow(&self) -> f32 { 0.0 }
    fn flex_shrink(&self) -> f32 { 0.0 }
    fn layout_children(&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree) -> Vec<(WidgetId, Rect)> { vec![] }
}

/// 渲染能力 — widget 可选实现。
pub trait WidgetRender: WidgetComponent {
    fn render(&self, frame: Rect, ctx: &mut RenderContext, tree: &WidgetTree);
    fn post_render(&self, frame: Rect, ctx: &mut RenderContext, tree: &WidgetTree) {}
    fn dirty_rect(&self, frame: Rect) -> Rect { frame }
    fn is_repaint_boundary(&self) -> bool { false }
    fn children_clip(&self, frame: Rect) -> Option<Rect> { None }
}

/// 事件处理能力 — widget 可选实现。
pub trait WidgetEventHandler: WidgetComponent {
    fn on_event(&mut self, event: &WidgetEvent) -> EventResult { EventResult::NotHandled }
    fn scroll_delta(&self, frame: Rect) -> Option<(f32, f32)> { None }
    fn hit_test_frame(&self, actual_frame: Rect) -> Rect { actual_frame }
    fn hit_test_3d(&self, ray: &Ray3D, spatial: &SpatialContext, frame: Rect) -> bool { /* 默认实现 */ }
}

/// 生命周期能力 — widget 可选实现。
pub trait WidgetLifecycle: WidgetComponent {
    fn on_init(&mut self) {}
    fn on_mount(&mut self) {}
    fn on_unmount(&mut self) {}
    fn on_update(&mut self, dt: f32) {}
}

/// 动画能力 — widget 可选实现。
pub trait WidgetAnimated: WidgetComponent {
    fn needs_continuous_update(&self) -> bool { false }
}
```

**关键变化**：
- 不再是 "所有 widget 自动拥有全部能力"，而是 "widget 声明自己有什么能力"
- `WidgetTree` 在遍历时通过**能力查询**发现 widget 支持哪些行为
- `define_widget!` 宏改为生成对应接口的实现

#### 2.3 BoxedWidget 作为组件容器

```rust
/// 组件容器 — 持有可选的多组件实现。
pub struct BoxedWidget {
    // 核心标识（必须）
    component: Box<dyn WidgetComponent>,
    
    // 可选能力（按需持有）
    layout: Option<Box<dyn WidgetLayout>>,
    render: Option<Box<dyn WidgetRender>>,
    event_handler: Option<Box<dyn WidgetEventHandler>>,
    lifecycle: Option<Box<dyn WidgetLifecycle>>,
    animated: Option<Box<dyn WidgetAnimated>>,
    
    // 树元数据（不变）
    id: WidgetId,
    parent: Option<WidgetId>,
    children: Vec<WidgetId>,
    frame: Rect,
    visible: bool,
    is_dirty: bool,
    widget_opacity: f32,
    z: i32,
}
```

**与现有代码的兼容性**：
- `define_widget!` 宏增强为同时生成多个接口的实现
- `WidgetTree` 内部通过 `as_any` 安全下转型来查询 widget 具有哪些能力
- 对已有 widget 零改动：宏自动推导哪些能力被实现

#### 2.4 宏的适配

```rust
// 当前：
define_widget! {
    pub Button { ... }
    render => (&self, frame, ctx, tree) { ... }
    on_event => (&mut self, event) -> EventResult { ... }
}

// 生成：impl Widget for Button { ... }
//       → 自动获得 WidgetRender + WidgetEventHandler blanket impl
```

```rust
// 解耦后：
define_widget! {
    pub Button { ... }
    
    // 显式声明实现的能力
    impl WidgetRender {
        render => (&self, frame, ctx, tree) { ... }
    }
    impl WidgetEventHandler {
        on_event => (&mut self, event) -> EventResult { ... }
    }
}

// 生成：
// impl WidgetComponent for Button {}
// impl WidgetRender for Button { ... }
// impl WidgetEventHandler for Button { ... }
```

---

### 阶段三：RenderContext 组件化（远期）

#### 3.1 当前问题

```rust
pub struct RenderContext<'a> {
    spatial: SpatialContext<'a>,      // 3D 空间
    font: FontHandle,                  // 字体句柄
    font_service: &'a FontService,     // 字体服务
    max_text_width: f32,
    tokens: &'a dyn TokenProvider,     // 主题令牌
    debug_mode: bool,
}
```

`RenderContext` 同时承担了：
- 2D 绘制委托（`fill_rect`, `draw_text` 等）
- 3D 空间上下文（`spatial()`）
- 主题令牌查询（`tokens()`）
- 字体服务
- 调试模式

#### 3.2 组件化设计

```rust
/// 渲染上下文 — 由多个渲染服务组件组合而成。
pub struct RenderContext<'a> {
    /// 2D 绘制组件
    pub canvas: &'a dyn Canvas2D,
    /// 3D 空间组件
    pub spatial: SpatialContext<'a>,
    /// 文本渲染组件
    pub text: TextRenderService<'a>,
    /// 主题令牌组件
    pub tokens: &'a dyn TokenProvider,
    /// 调试组件
    pub debug: DebugService,
}
```

每个子组件都有明确的接口定义在 `api/traits.rs` 中，可独立替换和测试。

---

## 三、组件依赖关系图（目标状态）

```
                        api/traits.rs
                     (接口契约中心)
                     │
        ┌────────────┼────────────┐
        │            │            │
   ┌────▼───┐  ┌────▼───┐  ┌────▼───┐
   │Widget  │  │Layout  │  │Render  │
   │组件系统 │  │组件    │  │上下文  │
   └────┬───┘  └────┬───┘  └────┬───┘
        │            │            │
   ┌────▼────────────▼────────────▼───┐
   │        Theme (TokenProvider)      │
   │        只依赖 api/traits          │
   └──────────────────────────────────┘
        │
   ┌────▼───────────────────────────┐
   │    Managers (跨组件服务)       │
   │    依赖 Widget 接口 + 自身实现  │
   └────────────────────────────────┘
        │
   ┌────▼───────────────────────────┐
   │    State (响应式状态)          │
   │    零 UI 依赖，纯数据层         │
   └────────────────────────────────┘
```

**依赖规则**：
- 所有组件只依赖 `api/` 模块（接口契约），不依赖其他组件的内部实现
- `WidgetTree` 通过能力查询发现 widget 支持的接口，不假设所有 widget 有相同能力
- 同一领域的实现模块（如 `widget/tree_core.rs`）可以依赖同领域的其他实现模块
- 跨领域依赖必须通过 `api/traits.rs` 中的接口

---

## 四、实施路线图

### Sprint 1：接口契约集中化（1-2 天）

| 任务 | 文件 | 动作 |
|------|------|------|
| 1 | `ui/src/api/traits.rs` | 从空占位改为定义 `LayoutEngine`、`Animatable`、5 个 Token trait、`TokenProvider` |
| 2 | `ui/src/api/types.rs` | 按组件领域分组 re-export（布局/动画/主题/widget/状态） |
| 3 | `ui/src/layout/engine.rs` | 移除 `LayoutEngine` trait 定义，改为 `use crate::api::traits::LayoutEngine` |
| 4 | `ui/src/animation/easing.rs` | 移除 `Animatable` trait 定义，改为 `use crate::api::traits::Animatable` |
| 5 | `ui/src/theme/color_tokens.rs` | 移除 5 个 trait 定义，保留 `ShadowToken` 值类型 |
| 6 | `ui/src/theme/wrapper.rs` | 移除 `TokenProvider` 定义，改为 `use crate::api::traits::TokenProvider` |
| 7 | 编译验证 | `cargo check` 确认所有引用路径正确 |

### Sprint 2：Widget trait 组件化（2-3 天）

| 任务 | 文件 | 动作 |
|------|------|------|
| 1 | `ui/src/widget/mod.rs` | 重写 Widget trait 为 5 个独立接口 + `WidgetComponent` 核心标识 |
| 2 | `ui/src/widget/mod.rs` | 重写 `BoxedWidget` 为组件容器（持有可选的能力指针） |
| 3 | `ui/src/macros.rs` | 重写 `define_widget!` 支持按接口分段的声明 |
| 4 | `ui/src/widget/tree_core.rs` | 修改 `WidgetTree` 遍历逻辑：通过能力查询而非 trait 方法调用 |
| 5 | `ui/src/widget/tree_events.rs` | 事件分发改为通过 `WidgetEventHandler` 能力查询 |
| 6 | `ui/src/widget/tree_dirty.rs` | 脏区域追踪适配新的组件结构 |
| 7 | 编译验证 | 修改所有直接调用 Widget trait 的地方 |

### Sprint 3：WidgetNode 与 WidgetTree 解耦（1 天）

| 任务 | 动作 |
|------|------|
| 1 | `WidgetNode` 改为持有组件列表而非单个 `Box<dyn Widget>` |
| 2 | `tree!` 宏适配新节点结构 |
| 3 | 树重建（`build_node`）的 key 匹配逻辑保持向后兼容 |

### Sprint 4（远期）：RenderContext 组件化 + 全面测试

---

## 五、风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| Widget trait 拆解后 `define_widget!` 宏兼容性 | 所有 widget 需重写声明 | 宏保持向后兼容：不声明 impl 段的自动按旧方式生成 |
| BoxedWidget 从单指针变多指针 | 内存占用增加 | 用 `Option<NonNull<dyn T>>` 或枚举节省空间 |
| WidgetTree 能力查询性能 | 遍历变慢 | 用 bitset 标记能力，O(1) 查询 |
| 60+ widget 组件库适配工作量大 | 迁移成本高 | 宏自动适配，手动修改 0 个 widget 文件 |
| 外部使用者 (`uix-app`) 的导入路径 | 破坏性变更 | lib.rs 保持 `pub use api::*`，快捷路径不变 |

---

## 六、验证标准

每个 Sprint 完成后验证：

1. **编译通过**：`cargo check` 无错误
2. **测试通过**：`cargo test` 无失败
3. **零功能损失**：运行 `/g/code/uix-app` 的所有已有测试和示例
4. **兼容性**：`uix-app` 层无需修改导入路径即可正常编译
5. **接口完整性**：`api/traits.rs` 覆盖所有公开接口契约，`api/types.rs` 覆盖所有值类型
