# UI 创建范式重构方案

> 日期：2026-06-28
> 目标：统一视觉属性到 Style 系统，让 margin 真正参与布局，简化组件创建体验
> 范围：`ui/` crate（widgets/style/layout/macros）
> 策略：**一次性改造，不做向后兼容**

---

## 一、总览

### 要解决的问题

| # | 问题 | 现状 |
|---|------|------|
| 1 | **视觉属性分散** | Container 有 `bg_color/border_color/padding`，Button 有 `custom_style`，各组件各自为政 |
| 2 | **Margin 不参与布局** | Style 和 Container 都有 margin 字段，但 FlexLayout 布局时不读子节点 margin |
| 3 | **无声明式 Style** | 没有 style 构建宏，写 Style 字面量冗长 |
| 4 | **创建方式略繁琐** | 各组件 builder 方法各自定义，视觉属性没统一入口 |

### 核心设计原则

1. **Style 是视觉的唯一来源** — 所有组件不再有 `bg_color/border_color/padding/direction` 等独立字段
2. **简化优先** — 减少用户的代码量，不增加不必要的抽象层
3. **Builder 是默认入口** — Props 结构体作为内部细节，用户通过 builder 链使用
4. **宏减少样板** — style 快捷方法由宏自动生成

### 目标对比

```
当前:
  Container::new()
      .bg_color(Some(BLUE))
      .padding(EdgeInsets::uniform(16))
      .border_radius(6.0)
      .direction(FlexDirection::Row)
      .child(Button::new("Click").primary())

目标:
  div()
      .bg(BLUE)
      .p(16)
      .rounded(6)
      .display(Flex)
      .direction(Row)
      .child(btn("Click").primary())
```

---

## 二、Style 系统 — 统一的视觉契约

### 2.1 Style struct

```rust
/// 类 CSS 样式 — 所有组件的通用视觉契约。
///
/// 每个组件只持有 `style: Style`，不再各自定义视觉字段。
/// 布局引擎从 style.margin 读取外边距参与盒模型计算。
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    // ── 盒模型 ──
    pub margin: EdgeInsets,
    pub padding: EdgeInsets,
    pub border: Option<BorderLine>,
    pub border_radius: f32,

    // ── 尺寸 ──
    pub width: Option<f32>,
    pub height: Option<f32>,

    // ── 弹性布局（容器属性）──
    pub display: DisplayMode,
    pub flex_direction: FlexDirection,
    pub flex_wrap: bool,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub gap: f32,

    // ── 弹性布局（子项属性）──
    pub flex_grow: f32,
    pub flex_shrink: f32,

    // ── 视觉 ──
    pub background: Option<Color>,
    pub background_hover: Option<Color>,
    pub background_active: Option<Color>,
    pub color: Color,
    pub font_size: f32,
    pub opacity: f32,
    pub box_shadow: Option<BoxShadowDef>,

    // ── 显示 ──
    pub visible: bool,
}

/// 单侧边框
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderLine {
    pub color: Color,
    pub width: f32,
}

/// 盒阴影
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxShadowDef {
    pub color: Color,
    pub blur: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}

/// 显示模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayMode {
    #[default]
    None,
    Flex,
    Grid,
}
```

### 2.2 `style!` 宏

```rust
/// 声明式 Style 构建宏。
///
/// # 语法
/// ```ignore
/// style! {
///     color: RED,                    // Color 值
///     margin: 8,                     // → EdgeInsets::uniform(8)
///     margin: [0, 8],                // → EdgeInsets::new(0, 8, 0, 8)
///     margin: [0, 8, 0, 8],          // → EdgeInsets::new(0, 8, 0, 8)
///     padding: 16,
///     bg: BLUE,                      // background 的别名
///     border: [RED, 1],              // → BorderLine
///     rounded: 6,                    // border_radius 的别名
///     fs: 14,                        // font_size 的别名
///     display: Flex,
///     direction: Row,                // flex_direction 的别名
///     gap: 8,
///     shadow: [BLACK, 4, 0, 2],      // box_shadow 的别名
///     grow: 1,                       // flex_grow 的别名
/// }
/// ```
macro_rules! style {
    // ...解析 key: value 对，生成 Style { ..Style::default() }
}
```

支持快捷别名：

| 完整名 | 别名 | 原因 |
|--------|------|------|
| `background` | `bg` | CSS 习惯 |
| `border_radius` | `rounded` | CSS 习惯 |
| `font_size` | `fs` | 高频使用 |
| `flex_direction` | `direction` | CSS 简写 |
| `flex_grow` | `grow` | CSS 简写 |
| `flex_shrink` | `shrink` | CSS 简写 |
| `justify_content` | `justify` | CSS 简写 |
| `box_shadow` | `shadow` | CSS 习惯 |

---

## 三、布局引擎 — 真正的盒模型

### 3.1 问题

当前 FlexLayout 布局时不读子节点 margin，margin 无法推开兄弟节点。

### 3.2 方案

`LayoutChild` 增加 `margin` 字段：

```rust
/// 统一子节点布局信息
#[derive(Debug, Clone)]
pub struct LayoutChild {
    pub id: WidgetId,
    pub preferred_size: Size,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub margin: EdgeInsets,        // ← 新增
    pub grid_cell: usize,
    pub grid_col_span: u32,
    pub grid_row_span: u32,
}
```

`EdgeInsets` 新增辅助方法：

```rust
impl EdgeInsets {
    /// 主轴方向的 margin 总和
    pub fn main_axis(&self, dir: FlexDirection) -> f32 {
        match dir {
            Row | RowReverse => self.left + self.right,
            Column | ColumnReverse => self.top + self.bottom,
        }
    }

    /// 起始边 margin
    pub fn start(&self, dir: FlexDirection) -> f32 {
        match dir {
            Row => self.left, RowReverse => self.right,
            Column => self.top, ColumnReverse => self.bottom,
        }
    }

    /// 交叉边起始 margin
    pub fn cross_start(&self, dir: FlexDirection) -> f32 {
        match dir {
            Row | RowReverse => self.top,
            Column | ColumnReverse => self.left,
        }
    }
}
```

FlexLayout 核心改动：

```rust
// 子节点占位 = preferred_size + margin（主轴方向）
let occupied_main = child.preferred_size.main(direction)
    + child.margin.main_axis(direction);

// 分配 frame 时偏移 margin
child_frame = Rect::new(
    cursor + child.margin.start(direction),
    cross_offset + child.margin.cross_start(direction),
    child.preferred_size.main(direction),
    child.preferred_size.cross(direction),
);
```

`child_from_tree` 从 Style 读取 margin：

```rust
pub fn child_from_tree(id: WidgetId, tree: &WidgetTree) -> LayoutChild {
    let node = tree.get(id);
    LayoutChild {
        id,
        preferred_size: node.map(|n| n.preferred_size(None)).unwrap_or_default(),
        flex_grow: node.and_then(|n| n.as_layout().map(|l| l.flex_grow())).unwrap_or(0.0),
        flex_shrink: node.and_then(|n| n.as_layout().map(|l| l.flex_shrink())).unwrap_or(0.0),
        margin: node.map(|n| n.style().margin).unwrap_or_default(),  // ← 从 Style 读
        ..LayoutChild::default()
    }
}
```

---

## 四、组件创建 — Builder 模式 + 统一 Style

### 4.1 组件分层

```
Builder（用户入口）
    │ 持有组件的配置，提供链式方法
    ↓
Impl struct（宏生成）
    │ 实现 WidgetComponent trait，持有 style + 内部状态
    ↓
WidgetNode（树节点）
    │ 加入 widget tree 参与布局和渲染
```

用户只接触 Builder，Impl 和 Props 是内部细节。

### 4.2 Button 示例

```rust
// ═══════════════════════════════════════
// 底层：Props（内部，用户不直接接触）
// ═══════════════════════════════════════

struct ButtonProps {
    text: String,
    variant: ButtonVariant,
    size: ButtonSize,
    on_click: RefCell<Option<Box<dyn FnMut()>>>,
    disabled: bool,
    loading: bool,
    danger: bool,
    block: bool,
    ghost: bool,
    icon: String,
    style: Style,
}

// ═══════════════════════════════════════
// 内部实现（define_widget! 宏生成）
// ═══════════════════════════════════════

define_widget! {
    pub(crate) Button {
        // ── Props 字段（来自构造参数）──
        text: String,
        variant: ButtonVariant,
        size: ButtonSize,
        on_click: RefCell<Option<Box<dyn FnMut()>>>,
        disabled: bool,
        loading: bool,
        danger: bool,
        block: bool,
        ghost: bool,
        icon: String,
        style: Style,

        // ── 内部状态 ──
        hovered: bool,
        pressed: bool,
        anim_progress: f32,
        click_pos: Option<Point>,
    }

    preferred_size => |self| {
        let h = button_height(self.size);
        let text_w = self.text.len() as f32 * 7.0;
        let icon_w = if self.icon.is_empty() { 0.0 } else { 20.0 };
        let w = text_w + button_padding_h(self.size) * 2.0 + icon_w;
        Size::new(if self.block { f32::MAX } else { w.max(32.0) }, h)
    }

    render => |self, frame, ctx| {
        ctx.apply_style(frame, &self.style);
        // 从 style 读颜色和字号
        let color = self.style.color;
        let font_size = self.style.font_size;
        ctx.text_center(&self.text, frame, color, font_size);
    }

    on_event => |self, event| -> EventResult {
        match event {
            MouseDown { .. } => { self.pressed = true; Handled }
            MouseUp { .. } => {
                self.pressed = false;
                if let Some(cb) = self.on_click.borrow_mut().as_mut() { cb(); }
                Handled
            }
            HoverEnter => { self.hovered = true; Handled }
            HoverLeave => { self.hovered = false; self.pressed = false; Handled }
            _ => NotHandled,
        }
    }

    on_update => |self, dt| {
        if self.anim_progress > 0.0 {
            self.anim_progress += dt / 0.4;
            if self.anim_progress >= 1.0 { self.anim_progress = 0.0; }
        }
    }

    needs_continuous_update => |self| self.anim_progress > 0.0
}

// ═══════════════════════════════════════
// Builder（用户入口）
// ═══════════════════════════════════════

/// 创建 Button 组件。
///
/// # 例子
/// ```ignore
/// btn("提交")
///     .primary()
///     .w(120)
///     .margin([0, 8])
///     .on_click(handle)
/// ```
pub fn btn(text: impl Into<String>) -> ButtonBuilder {
    ButtonBuilder {
        text: text.into(),
        variant: Default::default(),
        size: Default::default(),
        on_click: RefCell::new(None),
        disabled: false,
        loading: false,
        danger: false,
        block: false,
        ghost: false,
        icon: String::new(),
        style: Style::default(),
    }
}

/// Button 的构建器（宏生成大多数方法）。
pub struct ButtonBuilder {
    // ── 字段与 ButtonProps 一一对应 ──
    text: String,
    variant: ButtonVariant,
    size: ButtonSize,
    on_click: RefCell<Option<Box<dyn FnMut()>>>,
    disabled: bool,
    loading: bool,
    danger: bool,
    block: bool,
    ghost: bool,
    icon: String,
    style: Style,
}

// ── 行为属性 builder 方法 ──
impl ButtonBuilder {
    pub fn variant(mut self, v: ButtonVariant) -> Self { self.variant = v; self }
    pub fn size(mut self, s: ButtonSize) -> Self { self.size = s; self }
    pub fn on_click<F: FnMut() + 'static>(mut self, f: F) -> Self {
        self.on_click = RefCell::new(Some(Box::new(f))); self
    }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn loading(mut self, v: bool) -> Self { self.loading = v; self }
    pub fn icon(mut self, i: impl Into<String>) -> Self { self.icon = i.into(); self }
    pub fn danger(mut self, v: bool) -> Self { self.danger = v; self }
    pub fn block(mut self, v: bool) -> Self { self.block = v; self }
    pub fn ghost(mut self, v: bool) -> Self { self.ghost = v; self }

    // ── 预设快捷方法 ──
    pub fn primary(mut self) -> Self {
        self.variant = ButtonVariant::Primary;
        self.style.background = Some(Color::from_rgb(22, 119, 255));
        self.style.color = Color::white();
        self
    }
}

// ── 视觉属性 builder 方法（宏自动生成）──
// 以下方法由 define_widget! 的 #[auto_style] 自动生成：
//   .s(Style)       → 批量设置 style
//   .w(f32)         → style.width = Some(v)
//   .h(f32)         → style.height = Some(v)
//   .bg(Color)      → style.background = Some(c)
//   .color(Color)   → style.color = c
//   .fs(f32)        → style.font_size = v
//   .margin(impl Into<EdgeInsets>) → style.margin = v.into()
//   .padding(impl Into<EdgeInsets>) → style.padding = v.into()
//   .border(BorderLine) → style.border = Some(b)
//   .rounded(f32)   → style.border_radius = v
//   .opacity(f32)   → style.opacity = v
//   .display(DisplayMode) → style.display = v
//   .direction(FlexDirection) → style.flex_direction = v
//   .gap(f32)       → style.gap = v
//   .grow(f32)      → style.flex_grow = v
//   .shrink(f32)    → style.flex_shrink = v
//   .shadow(BoxShadowDef) → style.box_shadow = Some(v)
//   .visible(bool)  → style.visible = v

impl IntoWidgetNode for ButtonBuilder {
    fn into_node(self) -> WidgetNode {
        WidgetNode::leaf(Box::new(Button {
            text: self.text,
            variant: self.variant,
            size: self.size,
            on_click: self.on_click,
            disabled: self.disabled,
            loading: self.loading,
            danger: self.danger,
            block: self.block,
            ghost: self.ghost,
            icon: self.icon,
            style: self.style,
            hovered: false,
            pressed: false,
            anim_progress: 0.0,
            click_pos: None,
        }))
    }
}
```

**使用方式**：

```rust
// 风格 A：纯 builder 链（最常用）
btn("提交")
    .primary()
    .w(120)
    .margin([0, 8])
    .on_click(handle_submit)

// 风格 B：style! 宏批量设置视觉属性
btn("提交")
    .s(style! { bg: BLUE, color: WHITE, w: 120, margin: [0, 8] })
    .on_click(handle_submit)

// 风格 C：混合
btn("提交")
    .primary()
    .s(style! { w: 120, margin: [0, 8] })
    .on_click(handle_submit)
```

### 4.3 Container 示例

```rust
/// 创建容器组件。
///
/// # 例子
/// ```ignore
/// div()
///     .display(Flex).direction(Row).gap(8).p(16)
///     .child(btn("确定"))
///     .child(btn("取消"))
///
/// // 或带初始 style
/// div(style! { display: Flex, direction: Row, gap: 8, p: 16 })
///     .child(btn("确定"))
/// ```
pub fn div() -> ContainerBuilder { ContainerBuilder { style: Style::default(), overflow_content: false, children: Vec::new() } }
pub fn div(style: Style) -> ContainerBuilder { ContainerBuilder { style, overflow_content: false, children: Vec::new() } }

pub struct ContainerBuilder {
    style: Style,
    overflow_content: bool,
    children: Vec<WidgetNode>,
}

impl ContainerBuilder {
    // ── 视觉属性快捷方法（宏自动生成）──
    // .s(), .w(), .h(), .bg(), .margin(), .padding(), .border(), .rounded(),
    // .display(), .direction(), .gap(), .grow(), .shrink(), .shadow()...

    // ── 行为属性 ──
    pub fn overflow_content(mut self) -> Self {
        self.overflow_content = true; self
    }

    // ── 子节点 ──
    pub fn child(mut self, node: impl IntoWidgetNode) -> Self {
        self.children.push(node.into_node()); self
    }

    pub fn children(mut self, nodes: impl IntoIterator<Item = impl IntoWidgetNode>) -> Self {
        for node in nodes { self.children.push(node.into_node()); }
        self
    }
}

impl IntoWidgetNode for ContainerBuilder {
    fn into_node(self) -> WidgetNode {
        let style = self.style;
        WidgetNode::new(
            Box::new(ContainerImpl { style, overflow_content: self.overflow_content }),
            self.children,
        )
    }
}
```

使用：

```rust
// 纯 builder
div()
    .display(Flex)
    .direction(Row)
    .gap(8)
    .p(16)
    .bg(BG_GRAY)
    .rounded(6)
    .shadow([BLACK, 4, 0, 2]))
    .child(btn("确定").primary())
    .child(btn("取消"))

// 带 style! 宏
div(style! {
    display: Flex, direction: Row, gap: 8,
    p: 16, bg: BG_GRAY, rounded: 6,
})
    .child(btn("确定").primary())
    .child(btn("取消"))
```

### 4.4 Label 示例（最简单的组件）

```rust
pub fn label(text: impl Into<String>) -> LabelBuilder {
    LabelBuilder { text: text.into(), style: Style::default() }
}

pub struct LabelBuilder {
    text: String,
    style: Style,
}

// 视觉方法由宏自动生成
// 没有行为方法（Label 没有交互）

impl IntoWidgetNode for LabelBuilder {
    fn into_node(self) -> WidgetNode {
        WidgetNode::leaf(Box::new(LabelImpl { text: self.text, style: self.style }))
    }
}
```

使用：

```rust
label("Hello World").fs(14).color(TEXT_PRIMARY)
label("小标题").s(style! { fs: 12, color: TEXT_SECONDARY, margin: [0, 0, 8, 0] })
```

### 4.5 函数组件（组合模式）

```rust
/// 函数组件——纯组合，返回 impl IntoWidgetNode。
fn tool_bar() -> impl IntoWidgetNode {
    div()
        .display(Flex).direction(Row).gap(8)
        .p([8, 16]).bg(WHITE).border([BORDER_GRAY, 1]).rounded(6)
        .child(btn("新建").primary().on_click(|| println!("new")))
        .child(btn("编辑").on_click(|| println!("edit")))
        .child(btn("删除").danger(true))
}

fn card(title: &str, style: Style) -> impl IntoWidgetNode {
    div()
        .s(style! {
            display: Flex, direction: Column, gap: 12,
            p: 16, bg: WHITE, rounded: 8,
        })
        .s(style)  // 传入了 style 则覆盖
        .child(label(title).fs(16).color(TEXT_DARK))
}

fn my_page() -> impl IntoWidgetNode {
    div()
        .p(24)
        .display(Flex).direction(Column).gap(16)
        .child(tool_bar())
        .child(card("统计概览", style! {}))
        .child(card("最近订单", style! { margin: [16, 0, 0, 0] }))
}
```

### 4.6 关键设计决策

| 决策 | 选择 | 原因 |
|------|------|------|
| Builder vs Props 结构体 | **Builder**（Props 内部） | Builder 链更符合 Rust 习惯，Props 作为底层实现细节 |
| 快捷方法 vs 只用 style! 宏 | **两者都支持** | 高频属性（w/h/bg/color/margin/padding）用快捷方法更简洁，批量设置用 style! 宏更方便 |
| 宏生成 builder 方法 | **是** | 避免每个组件手写几十个 style 快捷方法 |
| `style!` 宏名 | **`style!`** | 与 CSS 同名，自文档化 |
| `primary()` 等预设 | **保留** | 预设方法同时设 variant + style，比纯 style! 宏更简洁 |

### 4.7 `define_widget!` 宏的增强

```rust
/// 增强版组件定义宏。
///
/// 新特性：
/// - 自动生成 Builder struct + 所有 style 快捷方法
/// - `#[auto_style]` 标记开启 style 快捷方法生成
/// - 支持闭包语法 `|self|` / `|self, frame, ctx|`
///
/// ```ignore
/// define_widget! {
///     pub Button {
///         text: String,
///         variant: ButtonVariant,
///         style: Style,
///         hovered: bool,
///     }
///
///     render => |self, frame, ctx| { ... }
///     on_event => |self, event| -> EventResult { ... }
/// }
/// ```
#[macro_export]
macro_rules! define_widget {
    // 新语法：闭包式方法定义
    (
        $(#[$m:meta])*
        $vis:vis $name:ident {
            $($field:ident : $ty:ty),* $(,)?
        }

        $( $method_name:ident => |$self:ident $(, $($p:ident),*)?| $(-> $ret:ty)? $body:block )*
    ) => {
        // 生成 struct
        $(#[$m])* struct $name {
            $($field : $ty),*
        }

        // 生成 WidgetComponent + 各能力 trait impl
        // ...

        // 自动生成 Builder struct + IntoWidgetNode
        // ...
    };
}
```

---

## 五、总体架构

### 5.1 完整示例

```rust
use uix::ui::prelude::*;

fn dashboard() -> impl IntoWidgetNode {
    div().p(24).display(Flex).direction(Column).gap(16)
        .child(header())
        .child(stat_row())
}

fn stat_row() -> impl IntoWidgetNode {
    div().display(Flex).direction(Row).gap(16)
        .child(stat_card("订单", "1,284", style! { bg: BLUE }))
        .child(stat_card("营收", "¥48k", style! { bg: GREEN }))
        .child(stat_card("用户", "3,421", style! { bg: PURPLE }))
}

fn stat_card(title: &str, value: &str, bg: Style) -> impl IntoWidgetNode {
    div()
        .s(bg).color(WHITE).rounded(8).p(16)
        .display(Flex).direction(Column).gap(8).grow(1)
        .child(label(title).fs(12).opacity(0.8))
        .child(label(value).fs(24))
}

fn main() {
    let mut tree = WidgetTree::new();
    tree.set_root(dashboard());
    run_widget_loop(tree);
}
```

### 5.2 组件映射表

| 组件 | 函数 | 行为方法 | 视觉方法 |
|------|------|---------|---------|
| Container | `div()` / `div(style)` | `.overflow_content()` | 快捷方法 + `.s()` |
| Button | `btn(text)` | `.variant()/.size()/.on_click()/.disabled()/.loading()/.icon()/.danger()/.block()/.ghost()` `.primary()` | 快捷方法 + `.s()` |
| Label | `label(text)` | 无 | 快捷方法 + `.s()` |
| Input | `input()` | `.value()/.placeholder()/.on_change()/.on_submit()/.size()` | 快捷方法 + `.s()` |
| ScrollView | `scroll_view(dir)` | `.direction()` | 快捷方法 + `.s()` |
| Space | `row()` / `col()` | 无 | 快捷方法 + `.s()` |
| Card | `card()` | 无 | 快捷方法 + `.s()` |

### 5.3 新旧对照

| 旧 | 新 | 状态 |
|----|----|------|
| `Container::new().bg(blue).pad(16).dir(Row)` | `div().bg(BLUE).p(16).direction(Row)` | **替换** |
| `Button::new("x").primary().size(Large)` | `btn("x").primary().size(Large)` | **替换**（`.primary()` 保留） |
| `Label::new("x").color(c).font_size(14)` | `label("x").color(c).fs(14)` | **替换** |
| `tree! { Container => [ Child ] }` | `div().child(child)` | **废弃** |
| `define_widget!` 旧语法 | `define_widget!` 闭包语法 | **不兼容** |

---

## 六、实施计划

### Phase 1：基建（预估 2-3 天）

1. 扩展 `Style` struct（新增全部字段）
2. 实现 `style!` 宏（macro_rules! 或 proc-macro）
3. `EdgeInsets` 新增 `main_axis()` / `start()` / `cross_start()`
4. `LayoutChild` 增加 `margin`，改造 `FlexLayout`
5. 增强 `RenderContext::apply_style`
6. 增强 `define_widget!` 宏：闭包语法 + 自动生成 Builder + 快捷方法
7. 单元测试

### Phase 2：组件改造（预估 4-6 天）

| 批次 | 组件 | 要点 |
|------|------|------|
| 1 | `Container` + `Space` + `ScrollView` | 容器类，含 layout_children |
| 2 | `Label` + `Typography` + `Icon` | 纯展示组件 |
| 3 | `Button` + `Input` + `InputNumber` | 交互组件 |
| 4 | `Card` + `Collapse` + `Tabs` + `Modal` + `Drawer` | 复合容器 |
| 5 | `Table` + `Form` + `Menu` + `Select` + `Tree` | 复杂组件 |
| 6 | 其余 40+ 组件 | 批量 |

### Phase 3：Demo + 文档（预估 1 天）

---

**总预估：7-10 天**

---

## 七、开放问题

1. **`style!` 宏用 macro_rules! 还是 proc-macro？**
   - 倾向 macro_rules!（无需单独 crate，解析足够简单）
   - 如果需要复杂表达式支持再升级 proc-macro

2. **快捷方法生成粒度？**
   - 所有 Style 字段都生成快捷方法（约 20 个）
   - 高频的给短名（w/h/bg/fs/p/m），低频的用全名（visible/opacity）

3. **`primary()` 预设方法如何统一？**
   - 每个组件自己定义预设快捷方法（如 Button 的 `primary()`）
   - 也可以定义 `style! { variant: Primary }` 直接设 variant 字段

4. **style 快捷方法命名风格？**
   - 短名更方便：`w/h/bg/fs/p/m/rounded/gap/grow/shrink`
   - 参考 Tailwind 的命名，但用 Rust 链式调用而非 class 字符串
