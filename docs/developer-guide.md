# UIX Framework — 开发指南

> 本文档面向 UIX 框架的使用方（应用开发者），提供完整的框架使用说明。
> 最后更新: 2026-07-03

---

## 目录

1. [框架概览](#1-框架概览)
2. [快速开始](#2-快速开始)
3. [Workspace 结构](#3-workspace-结构)
4. [核心概念](#4-核心概念)
5. [定义 Widget 组件](#5-定义-widget-组件)
6. [组合 Widget 树](#6-组合-widget-树)
7. [响应式状态管理](#7-响应式状态管理)
8. [渲染与绘图](#8-渲染与绘图)
9. [样式系统](#9-样式系统)
10. [布局系统](#10-布局系统)
11. [事件处理](#11-事件处理)
12. [动画与过渡](#12-动画与过渡)
13. [主题体系](#13-主题体系)
14. [应用入口与运行](#14-应用入口与运行)
15. [内置组件库](#15-内置组件库)
16. [测试](#16-测试)
17. [常用模式速查](#17-常用模式速查)
18. [API 索引](#18-api-索引)

---

## 1. 框架概览

### 1.1 这是什么

UIX 是一个 **Rust 原生 UI 框架**，提供完整的前端开发体验：

- **一套代码，多平台运行** — Windows (Win32) + Linux (Wayland)，行为完全一致
- **60+ 开箱即用组件** — 基于 Ant Design 5 设计规范
- **纯 CPU 渲染 + 可选 GPU 后端** — 无浏览器/WebView 依赖
- **响应式状态管理** — `State<T>` / `Computed<T>` 自动追踪依赖
- **增量渲染** — 脏矩形追踪，只重绘变化区域

### 1.2 适用场景

- 桌面端原生 GUI 应用
- 需要跨平台（Windows/Linux）一致体验的工具软件
- 对渲染性能有要求的自定义 UI
- 需要 2D/3D 混合绘制的专业应用

### 1.3 技术栈

| 层次 | 技术选型 |
|------|---------|
| 语言 | Rust（稳定版，2024 edition） |
| 窗口 | Win32 API / Wayland |
| 渲染 | 软件光栅化（默认） + Vulkan/Metal GPU 后端（可选） |
| 布局 | 自研 Flexbox + Grid 引擎 |
| 字体 | ab_glyph + Lucide 图标字体 |
| 构建 | Cargo workspace |

---

## 2. 快速开始

### 2.1 环境要求

- Rust 稳定版（1.80+）
- Windows: 无需额外依赖
- Linux: Wayland 开发库（`libwayland-dev`）

### 2.2 运行演示

```bash
# 克隆仓库
git clone <repo-url>
cd uix-app

# 运行 GUI 演示（默认包含 60+ 组件展示）
cargo run --bin uix-demo

# GPU 模式（如果支持）
cargo run --bin uix-demo -- --gpu

# CLI 演示
cargo run --bin uix-demo -- --cli

# 运行全部测试（330+ 个）
cargo test
```

### 2.3 最小应用

```rust
// src/main.rs
use uix::platform::create_platform;
use uix::graphics::SoftwareEngine;
use uix_graphics::font_service::FontService;
use uix::ui::{Theme, WidgetTree, Container};
use uix::ui::render_loop::run_widget_loop;
use uix::ui::widget::WidgetNode;
use uix::app::map_ui_event;
use std::cell::{Cell, RefCell};
use uix_platform::{Point, Rect};

fn main() {
    // 1. 构建组件树
    let root = WidgetNode::leaf(Box::new(Container::new().flex_grow(1.0)));
    let mut tree = WidgetTree::new();
    tree.build(root);
    tree.layout();
    tree.mark_full_frame_dirty();

    // 2. 创建平台和窗口
    let mut platform = create_platform().expect("平台初始化失败");
    let mut window = platform.window_manager()
        .create_window("我的应用", 800, 600)
        .expect("创建窗口失败");
    window.center_on_screen();
    window.show();

    // 3. 初始化渲染引擎
    let mut engine = SoftwareEngine::new();
    engine.initialize(800, 600).expect("引擎初始化失败");

    // 4. 字体服务
    let mut font_service = FontService::new();
    font_service.load_default_system_font(14.0, platform.system_info());

    // 5. 运行事件循环
    let theme = RefCell::new(Theme::antd_light());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::new(0.0, 0.0));

    run_widget_loop(
        &mut *platform, &mut *window, &mut engine,
        &mut tree, &font_service, &theme,
        &debug_mode, &cursor_pos,
        map_ui_event,
        |ev| matches!(ev.type_, uix_platform::event::UiEventType::WindowClose),
        |_, _, _| {},
    );

    engine.shutdown();
}
```

---

## 3. Workspace 结构

```
uix workspace             Cargo workspace 根
├── platform/              OS 抽象层 — Win32 / Wayland / 文件 / 日志 / 通知
│   └── src/
│       ├── api/           平台接口定义（traits.rs + types.rs）
│       ├── windows/       Win32 实现
│       ├── linux/         Linux/Wayland 实现
│       ├── test_harness/  测试桩（fake 实现）
│       └── event_bus.rs   事件总线
├── graphics/              2D 渲染引擎
│   └── src/
│       ├── api/           Canvas2D trait 定义
│       ├── engine/        软件渲染引擎
│       ├── gpu_engine/    GPU 后端
│       ├── rasterizer/    CPU 光栅化（填色/描边/渐变/阴影）
│       ├── spatial/       空间坐标系统（3D 变换、物理单位）
│       ├── frame_graph/   帧图 Pass 裁剪优化
│       └── traits/        Canvas2D + 渲染后端 trait
├── ui/                    Widget 框架
│   ├── src/
│   │   ├── api/           外部契约（WidgetComponent 等 trait 定义）
│   │   ├── widget/        WidgetTree + BoxedWidget + 事件系统
│   │   ├── widgets/       60+ 内置组件
│   │   ├── layout/        Flexbox + Grid 布局引擎
│   │   ├── state.rs       响应式状态（State / Computed / Effect）
│   │   ├── render_context.rs  渲染上下文入口
│   │   ├── render_loop.rs 渲染事件循环
│   │   ├── style.rs       CSS 式样式系统
│   │   ├── theme/         主题令牌系统
│   │   ├── animation/     动画与过渡
│   │   └── macros.rs      define_widget! / tree! 宏
│   └── macros/            ui! proc-macro
├── app/                   应用入口
│   └── src/
│       ├── application.rs App 构建器
│       ├── window.rs      窗口生命周期
│       ├── cli.rs         CLI 支持
│       └── di.rs          依赖注入容器
├── demo/                  演示应用
│   └── src/demos/dashboard/  GUI 完整演示
└── docs/                  开发文档
```

**开发时最常用的 import 路径**：

```rust
use uix::ui::*;              // Widget 框架全部公开 API
use uix::platform::*;        // 平台 API
use uix::graphics::*;        // 图形 API
use uix::app::*;             // 应用入口 API
```

---

## 4. 核心概念

### 4.1 组件（Widget）

UIX 中的组件是**按能力接口划分**的，没有上帝接口。每个组件可以选实现以下能力：

| 接口 | 能力 | 主要方法 |
|------|------|---------|
| `WidgetComponent` | 基础标识 | `as_any()`, `capabilities()`, `build()`, `visible()`, `tab_index()` |
| `WidgetLayout` | 布局 | `preferred_size()`, `flex_grow()`, `flex_shrink()`, `layout_children()` |
| `WidgetRender` | 渲染 | `render()`, `post_render()`, `dirty_rect()`, `is_repaint_boundary()` |
| `WidgetEventHandler` | 事件 | `on_event()`, `needs_continuous_update()`, `scroll_delta()`, `hit_test_frame()` |
| `WidgetLifecycle` | 生命周期 | `on_init()`, `on_mount()`, `on_unmount()`, `on_update()` |

### 4.2 组件树

- 父节点持有子节点，负责子节点生命周期
- 子节点不反向感知父节点内部实现
- 子树可整体替换、复用
- 树构建后通过 `WidgetId`（`usize`）访问各节点

### 4.3 数据流

**渲染管线**（每帧）：
```
WidgetTree.update(dt)    → 推进动画、计算脏矩形
WidgetTree.layout()      → 仅布局脏子树
LayerTree.render()       → 几何 Pass（清除+绘制脏区域）
LayerTree.render_overlays() → 叠加 Pass（焦点环、Tooltip 等）
IPresenter.present()     → 提交到屏幕
```

**事件流**：
```
OS 事件 → IEventLoop → 事件队列 → WidgetTree.dispatch_event()
    → 命中测试 → on_event() → EventBus.publish()
```

### 4.4 增量渲染

UIX 的核心优势之一：只重绘变化部分。

- 脏矩形追踪 + BitSet 跨帧累加
- 滚动时通过 `scroll_region()` memmove 已有像素，只绘制新暴露区域
- FrameGraph Pass 自动裁剪（输入未变化且无消费时跳过整 Pass）

---

## 5. 定义 Widget 组件

### 5.1 使用 define_widget! 宏（推荐）

宏自动生成 `WidgetComponent` 实现、能力位标记、各 trait 的上转型方法。

**基本结构**：

```rust
use uix::ui::*;
use uix_platform::{Point, Rect, Size};
use uix_graphics::{Color, GraphicsEngine};

define_widget! {
    /// 文档注释
    pub MyWidget {
        // 字段
        count: i32,
        label: String,
    }

    // 可选：构造函数
    @new -> Self { Self { count: 0, label: "默认".into() } }

    // 能力方法（按需定义）
    tab_index => (&self) -> i32 { 1 }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(120.0, 36.0)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        // 绘制逻辑
        ctx.fill_rect(frame, Color::rgb(24, 144, 255), Some((6.0).into()));
        let text = format!("{}: {}", self.label, self.count);
        ctx.text_center(&text, frame, Color::white(), 14.0);
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseUp { .. } => {
                self.count += 1;
                EventResult::Handled
            }
            _ => EventResult::NotHandled
        }
    }
}
```

### 5.2 各能力方法签名一览

| 方法 | 签名 | 所属 trait |
|------|------|-----------|
| `render` | `(&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree)` | WidgetRender |
| `post_render` | 同上 | WidgetRender |
| `preferred_size` | `(&self, _engine: Option<&dyn GraphicsEngine>) -> Size` | WidgetLayout |
| `flex_grow` | `(&self) -> f32` | WidgetLayout |
| `flex_shrink` | `(&self) -> f32` | WidgetLayout |
| `layout_children` | `(&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree) -> Vec<(WidgetId, Rect)>` | WidgetLayout |
| `on_event` | `(&mut self, event: &WidgetEvent) -> EventResult` | WidgetEventHandler |
| `needs_continuous_update` | `(&self) -> bool` | WidgetEventHandler |
| `scroll_delta` | `(&self, frame: Rect) -> Option<(f32, f32)>` | WidgetEventHandler |
| `hit_test_frame` | `(&self, actual_frame: Rect) -> Rect` | WidgetEventHandler |
| `on_init` | `(&mut self)` | WidgetLifecycle |
| `on_mount` | `(&mut self)` | WidgetLifecycle |
| `on_unmount` | `(&mut self)` | WidgetLifecycle |
| `on_update` | `(&mut self, dt: f64)` | WidgetLifecycle |
| `build` | `(&self) -> Vec<Box<dyn WidgetComponent>>` | WidgetComponent |
| `visible` | `(&self) -> bool` | WidgetComponent |
| `tab_index` | `(&self) -> i32` | WidgetComponent |

### 5.3 子组件（build 方法）

如果组件包含子组件，用 `build` 返回：

```rust
define_widget! {
    pub MyContainer {
        title: String,
    }

    build => (&self) -> Vec<Box<dyn WidgetComponent>> {
        vec![
            Box::new(Label::new(&self.title)),
            Box::new(Button::new("确定")),
        ]
    }
}
```

子组件的布局和可见性由父组件控制。

### 5.4 手动实现 Trait（代替宏）

当宏不满足需求时，可以直接手动实现：

```rust
use std::any::Any;
use uix::ui::api::traits::*;
use uix::ui::widget::{WidgetCapabilities, WidgetEvent, EventResult, WidgetTree};

struct MyCustomWidget { /* 字段 */ }

impl WidgetComponent for MyCustomWidget {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    fn capabilities(&self) -> WidgetCapabilities {
        let mut c = WidgetCapabilities::new();
        c.insert(WidgetCapabilities::RENDER);
        c.insert(WidgetCapabilities::EVENT);
        c
    }
    // 上转型方法（必须正确实现，否则能力不生效）
    fn as_render(&self) -> Option<&dyn WidgetRender> { Some(self) }
    fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> { Some(self) }
    fn as_event(&self) -> Option<&dyn WidgetEventHandler> { Some(self) }
    fn as_event_mut(&mut self) -> Option<&mut dyn WidgetEventHandler> { Some(self) }
    fn visible(&self) -> bool { true }
    fn build(&self) -> Vec<Box<dyn WidgetComponent>> { vec![] }
}

impl WidgetRender for MyCustomWidget {
    fn render(&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        ctx.fill_rect(frame, Color::red(), None);
    }
}

impl WidgetEventHandler for MyCustomWidget {
    fn on_event(&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseUp { .. } => EventResult::Handled,
            _ => EventResult::NotHandled,
        }
    }
}
```

### 5.5 组件最佳实践

- 所有视觉属性统一使用 `style: Style` 字段
- 状态变体（hover/active）颜色由 Style 定义，组件内部通过 `on_event` 切换状态
- 不要从组件内部直接访问父组件或其他兄弟组件
- 组件应该自包含：自己能完成渲染和事件处理

---

## 6. 组合 Widget 树

### 6.1 WidgetNode 构建

所有组件通过 `WidgetNode` 组装成树：

```rust
use uix::ui::widget::{WidgetNode, IntoWidgetNode};

// 叶子节点
let leaf = WidgetNode::leaf(Box::new(Label::new("你好")));

// 父节点
let parent = WidgetNode::new(
    Box::new(Container::new().dir(FlexDirection::Column)),
    vec![
        WidgetNode::leaf(Box::new(Label::new("标题"))),
        WidgetNode::leaf(Box::new(Button::new("点击"))),
    ],
);

// 链式附加属性
let node = WidgetNode::leaf(Box::new(Button::new("OK")))
    .key("btn-ok")        // 可选 key，用于调试
    .z_index(10)          // 叠加顺序
    .tab_index(1);         // Tab 导航顺序
```

任何实现了 `WidgetComponent` 的类型都自动实现了 `IntoWidgetNode`，可以直接 `.into_node()`：

```rust
let node = Button::new("Click").primary().into_node();
```

### 6.2 tree! 宏

适合静态层级清晰的场景：

```rust
use uix::tree;

let root = tree! {
    Container::new().dir(FlexDirection::Row).gap(8.0) => [
        Label::new("用户名:"),
        Input::new().placeholder("请输入"),
        Button::new("提交").primary(),
    ]
};
// 展开等价于 WidgetNode::new(...)
```

语法：`Parent(args) => [Child1, Child2, ...]`

### 6.3 ui! 宏（声明式 JSX 风格）

最接近 HTML/JSX 的写法，由 proc-macro 实现：

```rust
use uix::ui::ui;

let node = ui! {
    Container(bg: Color::white(), dir: Column, pad: EdgeInsets::all(16)) {
        Label("登录", font_size: 20, color: Color::black()),
        Input(placeholder: "账号"),
        Input(placeholder: "密码", type: "password"),
        Button("登录", variant: ButtonVariant::Primary, block: true),
    }
};
```

**语法规则**：
- `Name(pos_arg)` — 位置参数传给 `new()`
- `prop: value` — 调用 `.prop(value)` builder 方法
- `{ child1, child2 }` — 子节点列表（逗号可选）

### 6.4 三种建树方式对比

| 方式 | 优点 | 适用场景 |
|------|------|---------|
| `WidgetNode` | 最灵活，可加条件逻辑 | 动态构建、循环生成 |
| `tree!` | 零开销宏，层级清晰 | 静态结构 |
| `ui!` | 最贴近声明式 UI 直觉 | 复杂嵌套、属性多的场景 |

三种方式可以混合使用。

### 6.5 WidgetTree — 运行时操作

```rust
let mut tree = WidgetTree::new();

// 构建
tree.build(root_node);

// 获取根节点
let root_id = tree.root_id();
let root = tree.get(root_id);          // → Option<&BoxedWidget>
let root_mut = tree.get_mut(root_id);  // → Option<&mut BoxedWidget>

// 导航
let children = root.children().to_vec();
let parent_id = tree.parent_of(child_id);

// 修改可见性（不重建树）
tree.set_visible(widget_id, false);
tree.set_visible(widget_id, true);

// 标记脏区域（触发重绘）
tree.mark_dirty(widget_id);
tree.mark_full_frame_dirty();  // 全场重绘

// 布局
tree.layout();

// 通过类型查找并修改
tree.find_by_type_and_modify::<Counter>(|c| c.count += 1);

// 设置焦点
tree.set_focus(widget_id);
```

### 6.6 WidgetId 注意事项

`WidgetId` 是 `usize` 索引，每次 `build()` 后可能变化。**不要在 build 之后持久化 WidgetId**，需要时通过遍历或 `find_by_type_and_modify` 动态获取。

---

## 7. 响应式状态管理

UIX 提供轻量的响应式状态系统，基于 thread-local 依赖追踪。

### 7.1 State<T>

```rust
use uix::ui::State;

// 创建
let count = State::new(0);

// 读取（在 Computed 计算期间会自动注册依赖）
let val = count.get();

// 写入（触发 watcher + generation 递增）
count.set(42);
count.update(|n| *n += 1);

// 监听变化
count.watch(|new_val| {
    println!("值变为: {}", new_val);
});
```

**特点**：
- `T` 需满足 `Clone + Send + Sync + 'static`
- 内部使用 `Arc<RwLock<T>>`，线程安全
- `get()` 在普通环境下只是读取值，在 Computed/Effect 的闭包中会自动注册依赖

### 7.2 Computed<T> — 自动追踪的派生状态

```rust
use uix::ui::{State, Computed};

let a = State::new(1);
let b = State::new(2);

let sum = Computed::new(|| a.get() + b.get());

assert_eq!(sum.get(), 3);

a.set(10);
assert_eq!(sum.get(), 12);  // 自动重新计算！
```

**工作原理**：
1. 构造时执行闭包，自动记录所有读取过的 State
2. 存储依赖的 generation 快照
3. 每次 `get()` 比对 generation，有变化则重算

**何时重算**：懒加载——只在 `get()` 时检查依赖，不是 push 模式。

### 7.3 Effect — 自动追踪的副作用

```rust
use uix::ui::Effect;

let eff = Effect::new(|| {
    println!("count = {}", count.get());
});

// 在渲染循环中手动 tick
if eff.tick() {
    // 副作用已重新执行
}
```

**注意**：Effect 不会自动触发，需要每帧调用 `tick()` 检查依赖变化并重新执行。

### 7.4 在组件中使用状态

```rust
define_widget! {
    pub Counter {
        state: State<i32>,
    }

    @new -> Self { Self { state: State::new(0) } }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let val = self.state.get();
        ctx.text_center(&val.to_string(), frame, Color::white(), 32.0);
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseUp { .. } = event {
            self.state.update(|n| *n += 1);
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }
}
```

### 7.5 外部触发 UI 更新

State 在组件外部修改时，需要在 `on_frame` 回调中手动标记脏区域：

```rust
let count = State::new(0);
let count_clone = count.clone();

// 在外部修改
count_clone.set(42);

// on_frame 回调中标记重绘
move |tree, _, _| {
    tree.find_by_type_and_modify::<CounterDisplay>(|w| {
        // 状态已在外部修改，这里只需确保 UI 重绘
    });
    tree.mark_full_frame_dirty();
}
```

实际上，UIX 的 `dispatch_to` 在事件分发时会自动调用 `mark_dirty(target)`，所以组件内部的事件处理不需要手动标记脏区域。

---

## 8. 渲染与绘图

### 8.1 RenderContext

`RenderContext` 是组件渲染的唯一入口，在 `render` 方法中提供：

```rust
render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
    // ctx 提供所有绘制能力
}
```

### 8.2 2D 图形绘制

```rust
// ── 填充 ──
ctx.fill_rect(rect, color, radius);              // 填充矩形（可选圆角）
ctx.fill_circle(cx, cy, r, color);               // 填充圆
ctx.fill_ellipse(rect, color);                    // 填充椭圆
ctx.fill_sector(cx, cy, r, start_angle, end_angle, color); // 扇形
ctx.fill_path(&path, color, fill_rule);           // 填充路径

// ── 描边 ──
ctx.stroke_rect(rect, color, line_width, radius); // 描边矩形
ctx.stroke_circle(cx, cy, r, color, line_width);  // 描边圆
ctx.stroke_path(&path, color, &stroke_options);    // 描边路径

// ── 线条 ──
ctx.draw_line(x1, y1, x2, y2, color, width);

// ── 渐变 ──
ctx.fill_linear_gradient(rect, color_a, color_b, dir);
ctx.fill_radial_gradient(cx, cy, inner_r, outer_r, inner_c, outer_c);

// ── 阴影 ──
ctx.draw_box_shadow(rect, blur, offset_x, offset_y, color, corner_radius);
ctx.draw_box_shadow_ambient(rect, blur, offset_x, offset_y, color, corner_radius);

// ── 渲染状态 ──
ctx.save();     // 保存当前状态（裁剪区域等）
// 裁剪、变换操作...
ctx.restore();  // 恢复
```

### 8.3 文本绘制

```rust
// 左对齐顶部对齐
ctx.draw_text("你好", Point::new(10.0, 10.0), Color::black(), 14.0);

// 基于基线对齐
ctx.draw_text_baseline("Hello", x, baseline_y, color, font_size);

// 矩形内居中
ctx.text_center("居中文本", rect, Color::black(), 14.0);

// 左对齐垂直居中
ctx.draw_text_in_frame("左对齐", rect, Color::black(), 14.0);

// 自动换行
ctx.draw_text_wrapped("长文本内容...", rect, Color::black(), 14.0);

// 带选中高亮
ctx.draw_text_with_selection("Hello", pos, color, font_size,
    Some((1, 3)), Color::rgba(24, 144, 255, 60));

// 文本测量
let size: Size = ctx.measure_text("Hello", 14.0);
let size: Size = ctx.measure_text_wrapped("Hello", 14.0, 100.0);

// 文本命中测试（返回字符索引）
let idx: Option<usize> = ctx.text_hit_test("Hello", 14.0, point);

// 获取字符的 x 坐标
let x: f32 = ctx.text_cursor_x("Hello", 14.0, 3);
```

### 8.4 Style 应用

```rust
// 一键应用样式（绘制背景/边框/阴影）
ctx.apply_style(frame, &self.style);

// 然后在内容区域绘制文字
let content = frame.shrink(self.style.padding);
ctx.draw_text_in_frame("内容", content, self.style.color, self.style.font_size);
```

### 8.5 3D 空间绘制

UIX 支持统一的 2D/3D 空间绘制：

```rust
// 通过 spatial() 进入空间上下文
let spatial = ctx.spatial();

// 设置透视投影
spatial.set_perspective(60.0_f32.to_radians(), 1.333, 0.1, 100.0);

// 设置相机
spatial.set_camera_look_at(
    Vec3::new(0.0, 0.0, 5.0),   // 相机位置
    Vec3::zero(),                 // 观察目标
    Vec3::new(0.0, 1.0, 0.0),   // 上方向
);

// 应用变换
spatial.translate(1.0, 0.0, 0.0);
spatial.rotate_z(0.5);
spatial.scale(1.0, 1.0, 1.0);

// 使用物理单位（dpi-aware）
use uix_graphics::spatial::PhysicalUnit;
let size = 10.0.mm();    // 毫米
let size = 5.0.cm();     // 厘米
let size = 12.0.pt();    // 点（印刷）

// 3D 文本
ctx.draw_text_spatial("3D文本", Vec3::new(0.0, 0.0, 0.0), Color::white(), 12.0.pt());
ctx.text_center_spatial("居中", aabb_3d, Color::white(), 12.0.pt());
```

---

## 9. 样式系统

### 9.1 Style 结构

`Style` 是组件的**唯一视觉契约**——所有视觉属性统一通过 `style: Style` 配置：

```rust
pub struct Style {
    // 盒模型
    pub margin: EdgeInsets,         // 外边距
    pub padding: EdgeInsets,        // 内边距
    pub border_color: Option<Color>,
    pub border_width: f32,
    pub border_radius: f32,

    // 尺寸
    pub width: Option<f32>,
    pub height: Option<f32>,

    // 弹性布局（容器属性）
    pub display: DisplayMode,       // None / Flex / Grid
    pub flex_direction: FlexDirection,
    pub flex_wrap: bool,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub gap: f32,

    // 弹性布局（子项属性）
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub align_self: Option<AlignItems>,

    // 视觉
    pub background: Option<Color>,
    pub background_hover: Option<Color>,
    pub background_active: Option<Color>,
    pub color: Color,                // 文字颜色
    pub font_size: f32,
    pub opacity: f32,
    pub box_shadow: Option<BoxShadowDef>,

    pub visible: bool,
}
```

### 9.2 便捷构造

```rust
// 预设布局方向
let row = Style::row();       // display: Flex, direction: Row
let col = Style::column();    // display: Flex, direction: Column

// Builder 模式（通过组件方法设置 style）
Container::new()
    .bg(Color::white())            // background
    .color(Color::red())           // 文字颜色
    .font_size(16.0)               // 字号
    .border(1.0, Color::gray())    // 边框
    .radius(8.0)                   // 圆角
    .pad(EdgeInsets::all(16.0))    // padding
    .dir(FlexDirection::Row)       // 主轴方向
    .gap(8.0)                      // 间距
    .flex_grow(1.0)                // 弹性增长
    .flex_shrink(0.0)              // 不收缩
    .wrap(true)                    // 换行
    .align(AlignItems::Center)     // 交叉轴对齐
    .justify(JustifyContent::SpaceBetween);  // 主轴分布
```

### 9.3 优先级规则

```
用户自定义 style > 状态变体（hover/active） > widget 默认值 > 主题 tokens 默认值
```

### 9.4 在自定义组件中使用

```rust
define_widget! {
    pub MyStyledWidget {
        style: Style,
    }

    // 提供 builder 方法让用户方便设置样式
    // Container 等内置组件已提供全套 builder 方法

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        // apply_style 自动处理背景/边框/阴影
        ctx.apply_style(frame, &self.style);

        // 在内容区域绘制
        let content = frame.shrink(self.style.padding);
        ctx.draw_text_in_frame("内容", content, self.style.color, self.style.font_size);
    }
}
```

---

## 10. 布局系统

### 10.1 Flexbox 布局（默认）

默认布局引擎是 Flexbox（CSS Flexbox 子集）。

**容器属性**：

```rust
Container::new()
    .dir(FlexDirection::Row)              // Row（默认）或 Column
    .justify(JustifyContent::FlexStart)   // 主轴对齐
    .align(AlignItems::Stretch)           // 交叉轴对齐
    .wrap(true)                           // 是否换行
    .gap(8.0);                            // 子项间距
```

**子项属性**：

```rust
// 弹性增长（填满剩余空间）
Container::new().flex_grow(1.0);

// 弹性收缩（默认允许收缩）
Container::new().flex_shrink(0.0);  // 不收缩

// 单个子项覆盖 align-items
Container::new().align_self(AlignItems::Center);
```

**完整盒模型**（与 CSS 一致）：

```
┌─────────────────────────┐
│        margin            │
│  ┌───────────────────┐   │
│  │      border        │   │
│  │  ┌─────────────┐   │   │
│  │  │   padding    │   │   │
│  │  │ ┌─────────┐  │   │   │
│  │  │ │ content  │  │   │   │
│  │  │ │ (子节点)  │  │   │   │
│  │  │ └─────────┘  │   │   │
│  │  └─────────────┘   │   │
│  └────────────────────┘   │
└───────────────────────────┘
```

### 10.2 Grid 布局

```rust
use uix::ui::layout::grid::GridLayout;
use uix::ui::layout::engine::LayoutEngine;
```

### 10.3 布局引擎使用

```rust
use uix::ui::layout::engine::{FlexLayout, LayoutEngine, LayoutChild, child_from_tree};

let flex = FlexLayout {
    direction: FlexDirection::Row,
    gap: 8.0,
    ..Default::default()
};

let result = flex.layout(content_rect, &children);
// result: LayoutOutput { positions: Vec<(usize, Rect)>, total_size }
```

### 10.4 ScrollView

```rust
use uix::ui::{ScrollView, ScrollDirection};

// 垂直滚动
ScrollView::new(ScrollDirection::Vertical).flex_grow(1.0);

// 水平滚动
ScrollView::new(ScrollDirection::Horizontal);

// 双向滚动
ScrollView::new(ScrollDirection::Both);
```

滚动优化：ScrollView 通过像素移动（scroll_region）避免全帧重绘，只重绘新暴露的 strip 区域。

---

## 11. 事件处理

### 11.1 事件类型

```rust
pub enum WidgetEvent {
    MouseDown { pos: Point, button: MouseButton, mods: KeyMod },
    MouseUp { pos: Point, button: MouseButton, mods: KeyMod },
    MouseMove { pos: Point, mods: KeyMod },
    MouseWheel { pos: Point, delta: Point },
    KeyDown { key: KeyCode, mods: KeyMod },
    KeyUp { key: KeyCode, mods: KeyMod },
    KeyPress { text: String },
    FocusIn,
    FocusOut,
    HoverEnter,
    HoverLeave,
    Resize { width: f32, height: f32 },
    WindowMaximize, WindowMinimize, WindowRestore,
    WindowFocus, WindowBlur,
    Timer { id: u32 },
    FileDrop { files: Vec<String>, position: Point },
    // 组合拖拽事件（系统自动产生）
    DragStart { pos: Point, button: MouseButton, mods: KeyMod },
    DragMove { pos: Point, delta: Point, mods: KeyMod },
    DragEnd { pos: Point, button: MouseButton, mods: KeyMod },
}
```

**鼠标按钮**：`MouseButton::Left | Right | Middle | Back | Forward | None`

**修饰键**：`KeyMod::NONE | SHIFT | CTRL | ALT | META`（位标记）

### 11.2 事件响应

```rust
on_event => (&mut self, event: &WidgetEvent) -> EventResult {
    match event {
        WidgetEvent::MouseDown { pos, button, mods } => {
            if *button == MouseButton::Left {
                // 处理左键按下
                EventResult::Handled   // 阻止继续传播
            } else {
                EventResult::NotHandled  // 继续冒泡
            }
        }
        WidgetEvent::KeyDown { key, mods } if *key == KeyCode::Escape => {
            // ESC 键按下
            EventResult::Handled
        }
        WidgetEvent::HoverEnter => {
            // 鼠标进入
            EventResult::Handled
        }
        _ => EventResult::NotHandled
    }
}
```

**EventResult 三种返回值**：

| 值 | 含义 |
|-----|------|
| `Handled` | 已处理，停止传播 |
| `NotHandled` | 未处理，继续冒泡到父节点 |
| `Bubbled` | 已处理，但仍继续冒泡 |

### 11.3 拖拽手势

系统自动将鼠标事件组合为拖拽事件：

```rust
on_event => (&mut self, event: &WidgetEvent) -> EventResult {
    match event {
        WidgetEvent::DragStart { pos, button, .. } => {
            // 拖拽开始（MouseDown + 移动超过 5px 阈值后触发）
            EventResult::Handled
        }
        WidgetEvent::DragMove { pos, delta, .. } => {
            // 拖拽中（每帧触发）
            println!("移动偏移: ({}, {})", delta.x, delta.y);
            EventResult::Handled
        }
        WidgetEvent::DragEnd { pos, .. } => {
            // 拖拽结束（MouseUp）
            EventResult::Handled
        }
        _ => EventResult::NotHandled
    }
}
```

### 11.4 焦点管理

```rust
// 组件设置 tab_index 让 Widget 可聚焦
define_widget! {
    pub MyButton { }
    tab_index => (&self) -> i32 { 1 }
    // 接收 FocusIn / FocusOut 事件
}

// 在树上操作焦点
tree.set_focus(widget_id);         // 设置焦点
tree.focused_widget();             // 获取当前焦点 WidgetId

// 焦点环自动由 LayerTree 绘制（2px primary 色描边）
```

**Tab 键遍历**：系统自动收集 `tab_index > 0` 的可见组件，按 tab_index 排序。

### 11.5 事件冒泡路径

```
dispatch_to(target) → target.on_event()
    → 如果返回 NotHandled → parent.on_event()
    → 如果返回 NotHandled → parent's parent...
    → 直到根节点或 Handled
```

---

## 12. 动画与过渡

### 12.1 Animation<T>

```rust
use uix::ui::{Animation, Easing};

// 创建动画：从 0 到 1，持续 0.3 秒
let mut anim = Animation::new(0.0, 1.0, 0.3)
    .with_easing(Easing::ease_out());

// 在 on_update 中推进
let t = anim.update(dt);  // dt: 帧间隔秒数，返回当前进度 [0.0, 1.0]

// 检查状态
let done = anim.is_finished();
let progress = anim.progress();  // [0.0, 1.0]

// 重置
anim.reset();

// 支持任何 Animatable 类型
// Animatable trait:
pub trait Animatable: Clone + Copy + Send + 'static {
    fn lerp(from: Self, to: Self, t: f64) -> Self;
    fn delta(from: Self, to: Self) -> f64;
}
```

内置 `Animatable` 实现：`f32`, `f64`, `Color` 等。

### 12.2 在组件中使用

```rust
define_widget! {
    pub FadeInCard {
        title: String,
        opacity: f32,
        anim: Option<Animation<f32>>,
    }

    @new -> Self {
        Self {
            title: "卡片".into(),
            opacity: 0.0,
            anim: Some(Animation::new(0.0, 1.0, 0.4).with_easing(Easing::ease_out())),
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        // 应用透明度
        ctx.save();
        // 实际渲染...

        ctx.fill_rect(frame, Color::white().with_alpha((self.opacity * 255.0) as u8), None);
    }

    on_update => (&mut self, dt: f64) {
        if let Some(ref mut anim) = self.anim {
            self.opacity = anim.update(dt);
            // opacity 变化后，组件的 render 会自动响应（因为动画推进发生在布局之前）
        }
    }
}
```

### 12.3 缓动函数

```rust
Easing::linear()
Easing::ease_in()
Easing::ease_out()
Easing::ease_in_out()
Easing::antd_default()    // Ant Design 默认缓动

// 自定义三次贝塞尔
Easing::cubic_bezier(0.25, 0.1, 0.25, 1.0)
```

### 12.4 过渡系统

```rust
use uix::ui::transition::{Transition, TransitionPlayer, SlideDirection, presets};

// 使用预设
let config = presets::fade_in(0.3);

// 滑入动画
Transition::new(0.3).slide(SlideDirection::FromRight);
```

---

## 13. 主题体系

### 13.1 主题令牌（Token）

主题由 `TokenProvider` supertrait 定义，由四组子 trait 组成：

| 子 trait | 方法数 | 功能 |
|----------|--------|------|
| `IColorTokens` | ~40 | 颜色令牌（primary/bg/border/text/semantic 等） |
| `ITypographyTokens` | ~15 | 排版令牌（字号/字重/行高） |
| `ISpacingTokens` | ~20 | 间距令牌（padding/radius/control-height/screen-breakpoints） |
| `IBoxShadowTokens` | 2 | 阴影令牌 |

### 13.2 使用内置主题

```rust
use uix::ui::Theme;

// 亮色主题
let light = Theme::antd_light();

// 暗色主题
let dark = Theme::antd_dark();

// 在渲染中获取令牌
render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
    let tokens = ctx.tokens();
    let primary = tokens.color_primary();
    let bg = tokens.color_bg_container();
    let text = tokens.color_text();
    let default_font_size = tokens.font_size();
    let padding = tokens.padding();
    let radius = tokens.border_radius();
}
```

### 13.3 自定义主题

```rust
use uix::ui::api::traits::*;
use uix_graphics::Color;
use uix_platform::EdgeInsets;

struct MyTheme;

impl IColorTokens for MyTheme {
    fn color_primary(&self) -> Color { Color::rgb(0, 128, 255) }
    fn color_primary_hover(&self) -> Color { Color::rgb(64, 169, 255) }
    // ... 实现所有方法
}

impl ITypographyTokens for MyTheme { /* ... */ }
impl ISpacingTokens for MyTheme { /* ... */ }
impl IBoxShadowTokens for MyTheme { /* ... */ }

impl TokenProvider for MyTheme {
    fn is_dark(&self) -> bool { false }
}

// 使用
let theme = Theme::new(MyTheme);
```

### 13.4 DesignTokens

`DesignTokens` 是内置的 Ant Design 5 设计令牌实现：

```rust
use uix::ui::theme::DesignTokens;

let light = DesignTokens::antd_light();
let dark = DesignTokens::antd_dark();

// 运行时切换主题
// 在 on_frame 回调中监听 ThemeToggle 状态，重建树
```

### 13.5 内置颜色令牌速查

```rust
// 主色
tokens.color_primary();
tokens.color_primary_hover();
tokens.color_primary_active();

// 背景
tokens.color_bg_container();     // 容器背景
tokens.color_bg_elevated();      // 浮层面板
tokens.color_bg_layout();        // 页面背景

// 文字
tokens.color_text();             // 主要文字
tokens.color_text_secondary();   // 次要文字
tokens.color_text_tertiary();    // 第三级文字
tokens.color_text_quaternary();  // 第四级文字

// 边框
tokens.color_border();
tokens.color_border_secondary();

// 语义
tokens.color_success();
tokens.color_warning();
tokens.color_error();
tokens.color_info();
```

---

## 14. 应用入口与运行

### 14.1 App 构建器（简单方式）

```rust
use uix::app::api::App;

let mut app = App::new();
app.title("我的应用")
   .size(1000, 700)
   .create_window().unwrap()
   .run();
```

### 14.2 完整启动流程（灵活方式）

参考 `demo/src/demos/dashboard/mod.rs`，标准步骤：

```rust
fn main() {
    // 1. 构建组件树
    let root = build_my_app_tree();
    let mut tree = WidgetTree::new();
    tree.build(root);
    tree.layout();
    tree.mark_full_frame_dirty();

    // 2. 创建平台
    let mut platform = create_platform().expect("平台初始化失败");

    // 3. 创建窗口
    let mut window = platform.window_manager()
        .create_window("应用标题", 1024, 768)
        .expect("创建窗口失败");
    window.center_on_screen();
    window.show();

    // 4. 初始化渲染引擎
    let mut engine = SoftwareEngine::new();
    engine.initialize(1024, 768).expect("引擎初始化失败");

    // 5. 字体服务
    let mut font_service = FontService::new();
    font_service.load_default_system_font(14.0, platform.system_info());
    // 可选：加载图标字体
    if let Ok(ttf) = std::fs::read("assets/fonts/lucide.ttf") {
        uix::ui::widgets::icon::init_lucide_font(&ttf, &mut font_service);
    }

    // 6. 运行时状态
    let theme = RefCell::new(Theme::antd_light());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::new(0.0, 0.0));

    // 7. 运行事件循环
    let exit_code = run_widget_loop(
        &mut *platform,
        &mut *window,
        &mut engine,
        &mut tree,
        &font_service,
        &theme,
        &debug_mode,
        &cursor_pos,
        map_ui_event,                // UiEvent → WidgetEvent 映射
        |ev| matches!(ev.type_, UiEventType::WindowClose),  // 退出条件
        |tree, _, _| {               // 每帧回调（应用业务逻辑）
            // 导航切换、状态更新、主题切换等
        },
    );

    engine.shutdown();
    std::process::exit(exit_code);
}
```

### 14.3 事件映射函数

`map_ui_event` 将平台事件转为 Widget 事件：

```rust
use uix::app::map_ui_event;  // 使用默认映射

// 或自定义：
fn custom_event_mapper(ev: &UiEvent) -> Option<WidgetEvent> {
    match ev.type_ {
        UiEventType::MouseDown => {
            if let UiEventPayload::MouseButton(ref d) = ev.payload {
                Some(WidgetEvent::MouseDown { pos: d.pos, button: d.btn, mods: d.mods })
            } else { None }
        }
        UiEventType::KeyDown => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(WidgetEvent::KeyDown { key: d.key, mods: d.mods })
            } else { None }
        }
        _ => None,
    }
}
```

### 14.4 平台 API

```rust
// 平台
platform.event_loop().wait_event(&|e| ...);      // 事件循环
platform.event_bus().subscribe(|e| ...);          // 事件总线订阅
platform.event_bus().publish(event);              // 发布事件
platform.system_info().dpi();                     // 屏幕 DPI
platform.clipboard().set_text("...");             // 剪贴板

// 窗口
window.set_title("新标题");
window.set_size(800, 600);
window.minimize();
window.maximize();
window.close();
window.is_dirty(true);       // 触发重绘
window.native_surface_ptr(); // GPU 渲染用

// 文件服务
platform.file_service().open_file_dialog(...);

// 通知
platform.notification_service().show("标题", "内容", ...);

// 设置
platform.settings().get("key");
platform.settings().set("key", "value");
```

### 14.5 图标字体

```rust
// 使用 Lucide 图标字体
use uix::ui::widgets::icon::{Icon, init_lucide_font};

// 初始化（在创建字体服务后调用）
if let Ok(ttf) = std::fs::read("assets/fonts/lucide.ttf") {
    init_lucide_font(&ttf, &mut font_service);
}

// 使用图标
Icon::new("check").size(16.0);
Icon::new("user").color(Color::blue());

// 图标名参考：https://lucide.dev/icons
```

---

## 15. 内置组件库

所有组件在 `uix::ui::*` 下可直接使用。以下按功能分类：

### 通用
`Button`、`Icon`、`Typography`、`Label`、`Space`

### 布局
`Container`、`Layout（含 Header/Sider/Content/Footer）`、`Grid`、`Divider`、`Splitter`

### 导航
`Menu`、`Navigation`、`Tabs`、`Breadcrumb`、`Dropdown`、`Anchor`、`Affix`、`Pagination`、`Steps`

### 数据输入
`Input`、`InputNumber`、`Select`、`Checkbox`、`Radio`、`Switch`、`Slider`、`DatePicker`、`TimePicker`、`ColorPicker`、`Cascader`、`TreeSelect`、`Mentions`、`Rate`、`Upload`

### 数据展示
`Table`、`List`、`Tree`、`Calendar`、`Carousel`、`Image`、`Badge`、`Avatar`、`Tag`、`Descriptions`、`Collapse`、`Timeline`、`ProgressBar`、`QRCode`

### 反馈
`Alert`、`Modal`、`Drawer`、`Message`、`Notification`、`Popconfirm`、`Popover`、`Tooltip`、`Spin`、`Skeleton`、`Result`、`Empty`

### 图表
`BarChart`、`LineChart`、`PieChart`

### 富文本
`RichText`、`RichTextSegment`

### 其他
`ScrollView`、`VirtualScroll`、`BackTop`、`FloatButton`、`ThemeToggle`、`FocusTrap`、`Watermark`

**组件使用示例**：

```rust
use uix::ui::*;

// Button
Button::new("确定").primary().size(ButtonSize::Large)
Button::new("取消").danger().loading(false)
Button::new("链接").variant(ButtonVariant::Link)

// Input
Input::new().placeholder("请输入").prefix_icon("search")
Input::new().default_value("初始值").disabled(true)

// Select
Select::new()
    .placeholder("请选择")
    .options(vec![
        ("option1", "选项一"),
        ("option2", "选项二"),
    ])

// Table
Table::new()
    .columns(vec![
        TableColumn::new("姓名", "name").width(100.0),
        TableColumn::new("年龄", "age").width(60.0),
    ])
    .rows(data)

// Modal
Modal::new()
    .title("提示")
    .visible(is_open)
    .content(Label::new("确定要删除吗？"))
    .on_ok(|| println!("确认"))
    .on_cancel(|| println!("取消"))

// ScrollView
ScrollView::new(ScrollDirection::Vertical)
    .flex_grow(1.0)

// 图表
BarChart::new().data(vec![BarData::new("A", 30.0), BarData::new("B", 50.0)])
LineChart::new().data(vec![LineData::new(0.0, 10.0), LineData::new(1.0, 20.0)])
PieChart::new().data(vec![PieData::new("A", 30.0), PieData::new("B", 50.0)])
```

---

## 16. 测试

### 16.1 运行测试

```bash
# 全部测试
cargo test

# 某个 crate
cargo test -p uix-ui
cargo test -p uix-platform
cargo test -p uix-graphics

# 包含平台桩的测试
cargo test --features test-harness -p uix-ui
```

### 16.2 测试模式

框架提供两种测试方式：

**方式一：纯 widget 渲染测试**（无需平台）

```rust
#[test]
fn test_button_creation() {
    let btn = Button::new("测试").primary();
    // 验证 preferred_size、render 等
    assert_eq!(btn.text, "测试");
}

#[test]
fn test_widget_tree_layout() {
    let root = tree! {
        Container::new().size(200.0, 100.0) => [
            Label::new("Hello"),
        ]
    };
    let mut tree = WidgetTree::new();
    tree.build(root);
    tree.layout();
    // 验证子节点 frame 位置
}
```

**方式二：平台集成测试**（需要 test-harness）

```rust
#[cfg(feature = "test-harness")]
#[test]
fn test_with_fake_platform() {
    // 使用 fake platform 模拟事件分发
    // 验证事件处理、状态变更等
}
```

### 16.3 测试桩（Test Harness）

`platform/src/test_harness/` 提供了完整的 fake 实现：

```
fake_clipboard, fake_console, fake_cursor, fake_display,
fake_event_source, fake_file_dialog, fake_file_system,
fake_graphics_context, fake_keyboard, fake_notification,
fake_presenter, fake_system_info, fake_text_input, fake_timer, fake_window
```

通过 `--features test-harness` 启用。

---

## 17. 常用模式速查

### 17.1 完整自定义组件

```rust
use uix::ui::*;
use uix_platform::{Point, Rect, Size, EdgeInsets};
use uix_graphics::{Color, GraphicsEngine};

define_widget! {
    /// 自定义卡片组件
    pub MyCard {
        title: String,
        count: i32,
        style: Style,
    }

    @new -> Self {
        Self {
            title: "卡片".into(),
            count: 0,
            style: Style::default(),
        }
    }

    // builder 方法
    // Container/Button 等内置组件已有全套 builder 方法

    tab_index => (&self) -> i32 { 1 }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(200.0, 120.0)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        // 背景
        ctx.apply_style(frame, &self.style);
        // 标题
        ctx.draw_text(&self.title,
            Point::new(frame.x + 12.0, frame.y + 12.0),
            ctx.tokens().color_text(), 16.0);
        // 计数
        ctx.text_center(&self.count.to_string(),
            Rect::new(frame.x, frame.y + 50.0, frame.w, 40.0),
            ctx.tokens().color_primary(), 28.0);
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseUp { .. } => {
                self.count += 1;
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }
}
```

### 17.2 组合页面 + 导航切换

```rust
fn build_app_pages(tk: &DesignTokens) -> (WidgetNode, SharedActive) {
    // 使用 Navigation 组件
    let mut nav = Navigation::new("主菜单")
        .item("首页", "home")
        .item("设置", "settings")
        .active_index(0);
    let nav_active = nav.active().clone();
    let nav_node = nav.build(tk);

    // 各页面内容（全部加入同一棵树）
    let pages = vec![
        build_home_page(tk),
        build_settings_page(tk),
    ];

    let page_panel = WidgetNode::new(
        Box::new(Container::new().flex_grow(1.0)),
        pages,
    );

    let root = tree! {
        Container::new().flex_grow(1.0).dir(FlexDirection::Row) => [
            nav_node,
            page_panel,
        ]
    };
    (root, nav_active)
}

// 在 on_frame 回调中切换可见性
let prev_active = Cell::new(0);
let on_frame = move |tree: &mut WidgetTree, _, _| {
    let active = nav_active.get();
    if active != prev_active.get() {
        // 隐藏上一个页面，显示当前页面
        switch_page(tree, &page_ids, prev_active.get(), active);
        prev_active.set(active);
    }
};
```

### 17.3 主题切换

```rust
// 使用 ThemeToggle 组件
let toggle = ThemeToggle::new();

// 在 on_frame 中检测变化
let dark_mode = Cell::new(false);
let on_frame = move |tree: &mut WidgetTree, _, _| {
    let mut new_dark = false;
    tree.find_by_type_and_modify::<ThemeToggle>(|w| new_dark = w.dark.get());

    if new_dark != dark_mode.get() {
        dark_mode.set(new_dark);
        // 切换主题并重建树（因为 token 全局变化）
        rebuild_for_theme(tree, ...);
    }
};
```

### 17.4 响应式栅格

使用 `Style` 中的 flex_grow 让容器自适应：

```rust
tree! {
    Container::new().dir(FlexDirection::Row).gap(16.0) => [
        Container::new().flex_grow(1.0).bg(Color::red()),   // 左列 1/3
        Container::new().flex_grow(2.0).bg(Color::blue()),  // 右列 2/3
    ]
}
```

### 17.5 EventBus 通信

组件之间通过平台层 EventBus 实现松耦合通信：

```rust
// 发布事件
platform.event_bus().publish(UiEvent {
    type_: UiEventType::Custom("my_event"),
    payload: UiEventPayload::None,
    // ...
});

// 订阅事件
platform.event_bus().subscribe(|event: &UiEvent| {
    match event.type_ {
        UiEventType::Custom("my_event") => { /* 处理 */ }
        _ => {}
    }
});
```

---

## 18. API 索引

### 入口模块

| Crate | Import 路径 |
|-------|-------------|
| uix（聚合） | `use uix::*` |
| ui 框架 | `use uix::ui::*` |
| 平台 API | `use uix::platform::*` |
| 图形 API | `use uix::graphics::*` |
| 应用入口 | `use uix::app::*` |

### 关键 Trait 定义

| Trait | 路径 | 文件 |
|-------|------|------|
| `WidgetComponent` | `uix::ui::api::traits::WidgetComponent` | `ui/src/api/traits.rs` |
| `WidgetLayout` | 同上 | 同上 |
| `WidgetRender` | 同上 | 同上 |
| `WidgetEventHandler` | 同上 | 同上 |
| `WidgetLifecycle` | 同上 | 同上 |
| `TokenProvider` | 同上 | 同上 |
| `Canvas2D` | `uix::graphics::traits::Canvas2D` | `graphics/src/traits/canvas_2d.rs` |
| `GraphicsEngine` | `uix::graphics::GraphicsEngine` | `graphics/src/api/traits.rs` |

### 关键结构体

| 结构体 | 路径 | 说明 |
|--------|------|------|
| `WidgetTree` | `uix::ui::widget::WidgetTree` | 组件树 |
| `WidgetNode` | `uix::ui::widget::WidgetNode` | 树节点构建器 |
| `BoxedWidget` | `uix::ui::widget::BoxedWidget` | 运行时节点 |
| `RenderContext` | `uix::ui::render_context::RenderContext` | 渲染上下文 |
| `Style` | `uix::ui::style::Style` | 样式 |
| `State<T>` | `uix::ui::state::State` | 响应式状态 |
| `Computed<T>` | `uix::ui::state::Computed` | 派生状态 |
| `Effect` | `uix::ui::state::Effect` | 副作用 |
| `Animation<T>` | `uix::ui::animation::core::Animation` | 动画 |
| `Theme` | `uix::ui::theme::Theme` | 主题 |
| `App` | `uix::app::api::App` | 应用构建器 |

### 关键宏

| 宏 | 路径 | 说明 |
|----|------|------|
| `define_widget!` | `uix::ui::define_widget!` | 定义组件 |
| `tree!` | `uix::tree!` | 构建组件树 |
| `ui!` | `uix::ui::ui!` | 声明式 UI（proc-macro） |

---

> 本指南覆盖 UIX 框架的全部核心使用场景。实际开发中遇到具体组件用法，可直接查阅源码 `ui/src/widgets/` 下的对应文件，或参考 `demo/src/demos/dashboard/` 中的完整示例。
