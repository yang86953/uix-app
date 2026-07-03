# UIX Framework API 简化方案

> 目标：框架使用方只关心"页面上有什么"和"点了之后做什么"，
> 所有内部调度机制（平台、引擎、树管理、增量渲染、事件分发）对用户透明。
> 状态变更自动触发 UI 刷新。

---

## 目录

1. [现状分析](#1-现状分析)
2. [目标架构](#2-目标架构)
3. [具体重构项](#3-具体重构项)
4. [渐进迁移路径](#4-渐进迁移路径)
5. [工作量估算](#5-工作量估算)

---

## 1. 现状分析

### 1.1 用户视角的认知负担

通过对 `developer-guide.md` 中 API 的分析，统计用户上手需要理解的概念：

| 类别 | 数量 | 概念 |
|------|------|------|
| 建树方式 | 3 | `WidgetNode` / `tree!` / `ui!` |
| Widget 能力接口 | 5 | `WidgetComponent`, `Layout`, `Render`, `EventHandler`, `Lifecycle` |
| Widget trait 方法 | 18 | `build`, `visible`, `tab_index`, `preferred_size`, `flex_grow`, `flex_shrink`, `layout_children`, `render`, `post_render`, `dirty_rect`, `children_clip`, `draw_margin`, `on_event`, `needs_continuous_update`, `scroll_delta`, `hit_test_frame`, `hit_test_3d`, `on_update` |
| 渲染上下文方法 | 25+ | `fill_rect`, `stroke_rect`, `draw_text`, `text_center`, `fill_path`... |
| Style 字段 | 27 | `margin`, `padding`, `border_color`, `border_width`, `border_radius`, `width`, `height`, `display`, `flex_direction`, `flex_wrap`, `justify_content`, `align_items`, `gap`, `flex_grow`, `flex_shrink`, `align_self`, `background`, `background_hover`, `background_active`, `color`, `font_size`, `opacity`, `box_shadow`, `visible`... |
| 事件类型 | 19 | `MouseDown`, `MouseUp`, `MouseMove`, `MouseWheel`, `KeyDown`, `KeyUp`, `KeyPress`, `FocusIn`, `FocusOut`, `HoverEnter`, `HoverLeave`, `Resize`, `WindowMaximize`, `WindowMinimize`, `WindowRestore`, `WindowFocus`, `WindowBlur`, `Timer`, `FileDrop`, `DragStart`, `DragMove`, `DragEnd` |
| 状态 API | 3 | `State`, `Computed`, `Effect` |
| 启动流程 | 11 个参数 | platform, window, engine, tree, font_service, theme, debug_mode, cursor_pos, map_event, exit_condition, frame_callback |

**问题根源**：框架把"内部调度机制"当成了"用户的 API"。

- 用户不需要知道 `WidgetTree`、`WidgetNode`、`BoxedWidget`、`WidgetId` 存在
- 用户不需要手动建 `Platform`、`GraphicsEngine`、`FontService`
- 用户不需要理解 `mark_dirty`、`DirtyRegion`、增量渲染
- 用户不需要了解 `run_widget_loop` 的 11 个参数
- 用户不需要在三种建树语法中做选择

### 1.2 具体问题点

| # | 问题 | 后果 |
|---|------|------|
| 1 | 三种建树方式并存 | 决策成本，碎片化 |
| 2 | `define_widget!` 宏暴露内部机制 | 用户必须理解 trait、能力位、上转型 |
| 3 | 启动流程暴露太多内部依赖 | 写一个最小应用需要抄 50 行样板代码 |
| 4 | 状态变更不自动刷新 UI | 用户需要理解 mark_dirty 和调度机制 |
| 5 | `RenderContext`、`Canvas2D` 是底层 API | 新手被 25+ 绘制方法淹没 |
| 6 | `Style` 结构过于庞大 | 用户需要学习 27 个字段来调样式 |
| 7 | 事件系统手动模式较重 | match 19 种事件变体，EventResult 三种返回值 |

---

## 2. 目标架构

```
┌─────────────────────────────────────────────────┐
│                  用户层（简化 API）               │
│                                                 │
│  App::new().root(view).run()                     │
│  fn my_view() -> impl View                       │
│  button("X").on_click(fn)                        │
│  State::new(val) → 自动触发刷新                  │
│                                                 │
├─────────────────────────────────────────────────┤
│                  适配层（桥接）                   │
│                                                 │
│  View → WidgetNode 展开                          │
│  Event → View 回调映射                           │
│  State → 脏标记桥接                              │
│  App → run_widget_loop 封装                      │
│                                                 │
├─────────────────────────────────────────────────┤
│                  现有框架层                       │
│                                                 │
│  WidgetTree / BoxedWidget / WidgetId             │
│  LayoutEngine / Style / LayerTree                │
│  RenderContext / Canvas2D                         │
│  run_widget_loop / map_ui_event                   │
│  platform / engine / font / theme                 │
│  DirtyRegion / 增量渲染 / FrameGraph              │
│  EventBus / 事件分发                              │
│  define_widget! / tree! / ui!                     │
└─────────────────────────────────────────────────┘
```

**分层原则**：

- **用户层**：纯声明式。用户写函数 + 组合子 + 闭包回调。不知道 WidgetTree、RenderContext 等存在。
- **适配层**：将用户层的声明式描述（View trait）展开为框架层数据（WidgetNode、BoxedWidget）。
- **框架层**：现有基础设施不动，仅适配层调用框架层 API。

---

## 3. 具体重构项

### 3.1 入口：`App` 简化

**现状**（用户需要写这么多才能启动）：

```rust
let mut platform = create_platform()?;
let mut window = platform.window_manager().create_window("标题", 800, 600)?;
let mut engine = SoftwareEngine::new();
engine.initialize(800, 600)?;
let mut font_service = FontService::new();
font_service.load_default_system_font(14.0, platform.system_info());
let theme = RefCell::new(Theme::antd_light());
let debug_mode = Cell::new(false);
let cursor_pos = Cell::new(Point::new(0.0, 0.0));
run_widget_loop(&mut *platform, &mut *window, &mut engine,
    &mut tree, &font_service, &theme, &debug_mode,
    &cursor_pos, map_ui_event, |ev| ..., |tree,_,_| {});
```

**目标**：

```rust
// ── 最简 ──
fn main() {
    uix::App::new()
        .root(app_view)
        .run();
}

// ── 带配置 ──
fn main() {
    uix::App::new()
        .title("我的应用")
        .size(1024, 768)
        .theme(Theme::antd_dark())
        .root(app_view)
        .run();
}
```

**适配层实现**：

```rust
// 新增 uix::App（简化版本，非 app crate 的 App）
pub struct App {
    title: String,
    size: (i32, i32),
    theme: Theme,
    root: Option<Box<dyn View>>,
    // 可选注入
    on_exit: Option<Box<dyn Fn(&UiEvent) -> bool>>,
    on_frame: Option<Box<dyn Fn(&mut WidgetProxy)>>,
}

impl App {
    pub fn run(mut self) -> ! {
        // 1. 创建 platform
        // 2. 创建窗口
        // 3. 创建引擎
        // 4. 创建字体服务
        // 5. 通过 ViewAdapter 将 root View 展开为 WidgetTree
        // 6. 调用 run_widget_loop（内部传参，用户看不到）
        // 7. 用户只关心 View trait 和 on_click 回调
        todo!()
    }
}
```

### 3.2 构建 UI：一种 DSL

**现状**：三种方式并行，用户需要自己选。

```
WidgetNode::new(Box::new(...), vec![...])     // 手动，啰嗦
tree! { Parent => [Child] }                    // 宏，箭头语法不直观
ui! { Name(args) { children } }                // proc-macro，JSX-like
```

**目标**：一种方式。推荐函数式组合子。

```rust
// ── 方案 A：函数式组合子（零宏，纯 Rust） ──

fn login_page() -> impl View {
    column(
        label("登录").font_size(24),
        input().placeholder("账号"),
        input().placeholder("密码").password(),
        row(
            button("登录").primary().on_click(do_login),
            button("取消").on_click(go_back),
        ),
        space(16),
        label("忘记密码？").color(Color::blue()).on_click(reset_pwd),
    )
}
```

```rust
// ── 方案 B：view! 宏（JSX 风格，proc-macro） ──

fn login_page() -> impl View {
    view! {
        <Column align="center" gap=16>
            <Label text="登录" font_size=24 />
            <Input placeholder="账号" />
            <Input placeholder="密码" password=true />
            <Row gap=8>
                <Button text="登录" variant="primary" on_click={do_login} />
                <Button text="取消" on_click={go_back} />
            </Row>
        </Column>
    }
}
```

**推荐方案 A**：因为：

- 纯 Rust 语法，IDE 支持最好（补全、重构、跳转）
- 没有 proc-macro 编译开销
- 和组合子函数闭包自然配合
- `if`/`for`/`match` 可以直接写

```rust
// 条件渲染
fn user_card(user: Option<User>) -> impl View {
    column(
        match user {
            Some(u) => view_user_info(u),
            None => view_loading(),
        },
    )
}

// 列表渲染
fn item_list(items: Vec<Item>) -> impl View {
    column(
        items.into_iter().map(|item| item_row(item)),
    )
}
```

**组合子函数签名**：

```rust
// 所有"视图"都返回 impl View
pub trait View: Sized {
    /// 将 View 展开为 WidgetTree（框架内部调用）
    fn build(self) -> ViewNode;
}

// 组合子函数
pub fn column(children: impl IntoIterator<Item = impl View>) -> impl View { ... }
pub fn row(children: impl IntoIterator<Item = impl View>) -> impl View { ... }
pub fn label(text: &str) -> impl View { ... }
pub fn button(text: &str) -> impl View { ... }
pub fn input() -> impl View { ... }
pub fn space(size: f32) -> impl View { ... }

// 属性通过 builder 方法链式设置
pub fn button(text: &str) -> ButtonBuilder {
    ButtonBuilder { text: text.to_string(), variant: Default::default(), on_click: None }
}

impl ButtonBuilder {
    pub fn primary(mut self) -> Self { self.variant = ButtonVariant::Primary; self }
    pub fn on_click<F: FnMut() + 'static>(mut self, f: F) -> Self { self.on_click = Some(Box::new(f)); self }
}
```

### 3.3 View 定义：函数式组件

**现状**：必须用 `define_widget!` 宏定义一个完整的 struct + trait 实现。

```rust
define_widget! {
    pub Counter { count: i32 }
    @new -> Self { Self { count: 0 } }
    render => (&self, frame, ctx, tree) { ... }
    on_event => (&mut self, event) -> EventResult { ... }
}
```

**目标**：

```rust
// 80% 场景：函数就够了
fn counter(initial: i32) -> impl View {
    let count = State::new(initial);
    column(
        label(move || format!("计数: {}", count.get())),
        row(
            button("+").on_click(move || count += 1),
            button("-").on_click(move || count -= 1),
        ),
    )
}

// 需要自定义绘制时：实现 View trait
pub struct Avatar { pub url: String, pub size: f32 }

impl View for Avatar {
    fn render(&self, ui: &mut Ui) {
        ui.draw_image(&self.url, Rect::size(self.size, self.size));
    }
}
```

**关键转变**：

| 现状 | 目标 |
|------|------|
| 用户定义 WidgetComponent + 各 trait | 用户定义 View trait（一个方法）或写函数 |
| `render` 接收 `frame: Rect` + `RenderContext` | `render` 接收 `&mut Ui`（简化渲染上下文） |
| `on_event` 处理 match 19 种事件 | `on_click` / `on_change` 等具名回调 |
| 字段在 struct 中定义 | 字段在闭包中捕获 |
| builder 方法需要手动实现 | builder 由适配层自动生成 |

**`Ui` 结构体**（用户层渲染上下文）：

```rust
pub struct Ui<'a> {
    // 简化版渲染 API，内部委托给 RenderContext
    // 只暴露用户需要的绘制方法
}

impl<'a> Ui<'a> {
    // 少量高频 API
    pub fn fill_rect(&mut self, rect: Rect, color: impl Into<Color>) { ... }
    pub fn stroke_rect(&mut self, rect: Rect, color: impl Into<Color>, width: f32) { ... }
    pub fn text(&mut self, text: &str, pos: Point, color: impl Into<Color>, font_size: f32) { ... }
    pub fn text_center(&mut self, text: &str, rect: Rect, color: impl Into<Color>, font_size: f32) { ... }
    pub fn image(&mut self, url: &str, rect: Rect) { ... }

    // 主题令牌简化
    pub fn color(&self, name: &str) -> Color { ... }  // "primary", "bg", "text"...
}
```

### 3.4 状态管理：自动脏标记

**现状**：

- 组件内改状态 → 靠 `dispatch_to` 自动标记（只在事件分发中生效）
- 组件外改状态 → 需要手动 `mark_dirty`
- `Effect.tick()` 需要每帧调用

**目标**：

```rust
let count = State::new(0);

// 任意位置改状态 → 自动标记所属 View 为脏
count.set(42);
count.update(|n| *n += 1);

// 框架在渲染循环中自动：
// 1. 检测哪些 State 发生了变化
// 2. 找到依赖这些 State 的 View
// 3. 标记 dirty → 触发重绘
```

**实现思路**：

`State<T>` 新增一个 `owner: Option<ViewId>` 字段（在 View 中使用时由适配层自动设置）。

```rust
// 适配层桥接：
// 1. 函数式组件中创建的 State，由 ViewAdapter 绑定到对应 ViewNode
// 2. 当 State.set() 被调用时，自动调用 WidgetTree::mark_dirty(owner)
// 3. 用户完全不知道 DirtyRegion、mark_dirty 这些概念

impl<T> State<T> {
    pub fn set(&self, value: T) {
        // ... 现有逻辑 ...
        // 新增：如果有 owner，自动标记脏
        if let Some(owner) = self.owner.get() {
            unsafe { WIDGET_TREE.with(|wt| wt.mark_dirty(owner)); }
        }
    }
}
```

**全局追踪方式**（替代方案：view_id 注册）：

```rust
// State 创建时自动注册当前 View 上下文
thread_local! {
    static CURRENT_VIEW: RefCell<Option<ViewId>> = RefCell::new(None);
}

impl<T> State<T> {
    pub fn new(value: T) -> Self {
        let view_id = CURRENT_VIEW.with(|cv| cv.borrow().clone());
        Self {
            inner: Arc::new(RwLock::new(...)),
            view_id,  // 记录所属 View
        }
    }

    pub fn set(&self, value: T) {
        // 写入值后，自动触发所属 View 的 dirty 标记
        self.inner.write().value = value;
        if let Some(vid) = self.view_id {
            WidgetTree::mark_dirty(vid);
        }
    }
}
```

### 3.5 内置事件回调：具名回调取代 EventResult

**现状**：

```rust
on_event => (&mut self, event: &WidgetEvent) -> EventResult {
    match event {
        WidgetEvent::MouseUp { .. } => { self.count += 1; EventResult::Handled }
        WidgetEvent::KeyDown { key, .. } if *key == KeyCode::Escape => { ... }
        _ => EventResult::NotHandled,
    }
}
```

**目标**：

```rust
// 常用：具名回调
button("+").on_click(|| count += 1);
input().on_change(|val| println!("输入: {}", val));
checkbox().on_toggle(|checked| ...);

// 不常用的事件：通过 .on() 通用方法
button("X").on(Event::KeyDown, |key| ...);
```

**适配层映射**：

```
on_click     → WidgetEvent::MouseUp
on_change    → WidgetEvent::KeyPress / Change 组合
on_focus     → WidgetEvent::FocusIn
on_blur      → WidgetEvent::FocusOut
on_hover     → WidgetEvent::HoverEnter / HoverLeave
on_key       → WidgetEvent::KeyDown
on_scroll    → WidgetEvent::MouseWheel
```

### 3.6 主题：简化 API

**现状**：`ctx.tokens().color_primary_hover()` — 40+ 个方法名需要记。

**目标**：

```rust
// 在 View 中
fn my_view() -> impl View {
    // 用户层不知道 TokenProvider 的存在
    let token = use_token();
    // token 有简化的 API
    token.color("primary")        // → Color
    token.color("bg")             // → Color
    token.color("text")           // → Color
    token.font_size()             // → f32
    token.radius()                // → f32
    token.spacing()               // → f32
}

// 或者主题值直接从 Ui 获取
fn render(&self, ui: &mut Ui) {
    let bg = ui.color("bg_container");
    let text = ui.color("text");
}
```

### 3.7 组件属性：从 Style 暴露简化

**现状**：用户需要设置 Style 的 27 个字段，或者使用 Container builder 方法。

**目标**：所有 View 组合子都暴露一致的属性设置方式：

```rust
column(
    // 所有 View 都有 .style() 方法设置常用属性
    label("Hello")
        .color("#333")
        .font_size(16)
        .padding(8),

    button("Submit")
        .primary()
        .width(120)
        .on_click(submit),
)
```

```rust
// 适配层提供 StyleExt trait
pub trait StyleExt: View + Sized {
    fn color(mut self, color: impl Into<Color>) -> Self { ... }
    fn font_size(mut self, size: f32) -> Self { ... }
    fn padding(mut self, p: impl Into<EdgeInsets>) -> Self { ... }
    fn bg(mut self, color: impl Into<Color>) -> Self { ... }
    fn width(mut self, w: f32) -> Self { ... }
    fn height(mut self, h: f32) -> Self { ... }
    fn flex_grow(mut self, g: f32) -> Self { ... }
    fn gap(mut self, g: f32) -> Self { ... }
    // ... 高频的 15 个属性
}
```

### 3.8 生命周期：自动管理

**现状**：`on_init`, `on_mount`, `on_unmount`, `on_update` 需要用户显式实现。

**目标**：用户写函数式组件不需要知道生命周期存在。

```rust
// 函数式组件自动获得生命周期：
// - 每次渲染时重建 View（框架内部缓存或对比）
// - 回调闭包在组件销毁时自动 drop
// - on_update 由框架通过帧循环驱动

// 需要显式生命周期时：
fn my_view() -> impl View {
    use_effect(|| {
        // 组件挂载时执行
        subscribe_to_some_event(handler);
        // 返回清理函数
        move || unsubscribe(handler)
    });

    label("Hello")
}
```

---

## 4. 渐进迁移路径

### 阶段一：新增适配层（不破坏现有 API）

```
ui/src/
├── view/                        # 新增：用户层 API
│   ├── mod.rs                   # View trait, ViewNode
│   ├── combinators.rs           # column(), row(), label(), button()...
│   ├── state.rs                 # State 增强（自动脏标记）
│   ├── app.rs                   # 简化 App 入口
│   └── adapter.rs               # View → WidgetTree 展开
├── ...
```

- 新增 `uix::view` 模块（或 `uix::ui::view`）
- 现有 `define_widget!`、`WidgetNode`、`tree!`、`ui!` 全部不动
- 旧用户继续用老 API，新用户走新路径

### 阶段二：View 展开器（适配层的核心）

`ViewAdapter` 负责将用户层的 `View` 树展开为框架层的 `WidgetTree`：

```rust
// 核心适配流程
pub struct ViewAdapter;

impl ViewAdapter {
    pub fn build(view: impl View) -> WidgetTree {
        let node = view.build();  // → ViewNode
        let mut tree = WidgetTree::new();

        // 递归展开 ViewNode → WidgetNode
        fn expand(vn: ViewNode) -> WidgetNode {
            match vn.kind {
                ViewKind::Label { text, style, .. } => {
                    WidgetNode::leaf(Box::new(Label::new(&text).style(style)))
                }
                ViewKind::Button { text, variant, on_click, style, .. } => {
                    let mut btn = Button::new(&text);
                    btn = match variant {
                        ButtonVariant::Primary => btn.primary(),
                        _ => btn,
                    };
                    btn = btn.style(style);
                    if let Some(cb) = on_click {
                        // 将闭包封装为 EventHandler
                    }
                    WidgetNode::leaf(Box::new(btn))
                }
                ViewKind::Column { children, style, .. } => {
                    WidgetNode::new(
                        Box::new(Container::new().dir(Column).style(style)),
                        children.into_iter().map(expand).collect(),
                    )
                }
                // ...
            }
        }

        tree.build(expand(node));
        tree
    }
}
```

### 阶段三：State 自动脏标记

- `State::set()` 检查是否绑定了 `ViewId`
- 绑定了则自动调用 `WidgetTree::mark_dirty(view_id)`
- ViewAdapter 在构建时绑定 State

```rust
pub struct ViewContext {
    // 当前正在构建的 View 的 WidgetId
    current_view_id: Cell<Option<WidgetId>>,
}

// 在 expand 阶段：
fn expand(vn: ViewNode, ctx: &ViewContext) -> WidgetNode {
    // 为当前 View 分配一个 WidgetId
    let view_id = ctx.alloc_id();

    // 递归展开子节点时，在上下文中记录 view_id
    ctx.current_view_id.set(Some(view_id));

    // 当用户代码创建 State 时，State::new 内部读取 current_view_id
    // 从而绑定到该 View

    // ...
}
```

### 阶段四：标记旧 API 为 deprecated

- `define_widget!` → 建议用函数式组件
- `tree!` / `ui!` → 建议用 `column()` / `row()` 组合子
- `WidgetNode` → 内部使用，不公开
- `RenderContext`（直暴露）→ 包装成 `Ui`
- `run_widget_loop` → 被 `App::run()` 替代

---

## 5. 工作量估算

| 模块 | 改动量 | 文件 | 说明 |
|------|--------|------|------|
| `ui/src/view/`（新增） | ~800 行 | 5 文件 | View trait、组合子函数、App、State 增强、Adapter |
| `ui/src/state.rs`（修改） | ~30 行 | 1 文件 | 添加 owner 绑定 + 自动 mark_dirty |
| `ui/src/lib.rs`（修改） | ~5 行 | 1 文件 | 导出 view 模块 |
| `ui/src/widgets/` 各组件 | ~200 行 | 每个组件加 builder | 组件已有一部分 builder |
| `app/src/`（新增简化版） | ~100 行 | 1 文件 | 简化 App |
| demo 迁移示例 | ~150 行 | 3 文件 | 写新的 demo 展示新 API |

**总计**：~1300 行核心代码 + 示例。全部新增或内联，**不影响现有 API 和已有代码**。

---

## 6. 附录：新 API 使用对比

### 当前 v.s. 简化后

| 场景 | 当前 | 简化后 |
|------|------|--------|
| 启动 | 50 行 | `App::new().root(view).run()` |
| 建树 | 三种语法选一 | `column(...)` / `row(...)` |
| 写组件 | define_widget! DSL | `fn my_view() -> impl View { ... }` |
| 绑定状态 | `on_event` + `match` + `mark_dirty` | `on_click(|| ...)` |
| 自定义绘制 | `RenderContext` (25+ 方法) | `Ui` (8 个方法) |
| 布局 | `Container::new().dir().gap()` | `column().gap(8)` |
| 主题 | `ctx.tokens().color_primary_hover()` | `ui.color("primary")` |
| 测试 | `test-harness` feature + fake | `App::test(root).click(btn).assert(...)` |
| 生命周期 | `on_init` / `on_mount` / `on_unmount` / `on_update` | `use_effect(|| { ...; || cleanup })` |
| 样式 | `style: Style` 27 个字段 | `.color("#f50").padding(8).bg("#fff")` |
