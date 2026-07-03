# UIX Framework — 开发指南

> 面向应用开发者。你只需要关心"页面上有什么"和"点了之后做什么"。
> 最后更新: 2026-07-03

---

## 快速开始

```bash
# 运行完整演示
cargo run --bin uix-demo

# 简化 API 演示
cargo run --bin uix-demo -- --simple
```

**最小应用（4 行）**：

```rust
use uix::ui::view::*;
use uix::ui::{State, App};

fn main() {
    let count = State::new(0);
    App::new().title("计数器").size(400, 200)
        .root(column([
            dynamic_label(move || format!("点击了 {} 次", count.get())).font_size(24.0),
            button("点我").primary().on_click(move || count += 1),
        ]).gap(16).padding(24.0))
        .run();
}
```

---

## 构建 UI

一切 UI 由**组合子函数** + **链式样式**拼装而成，不需要任何宏。

### 组合子

```rust
use uix::ui::view::*;

fn my_page() -> ViewNode {
    column([              // 列容器
        label("标题").font_size(24.0),
        row([             // 行容器
            button("确定").primary().on_click(|| println!("ok")),
            button("取消").on_click(|| println!("cancel")),
        ]).gap(8),
        input().placeholder("请输入"),
        space(16),        // 空白占位
    ]).gap(12).padding(24.0)
}
```

| 函数 | 说明 |
|------|------|
| `column([...])` | 垂直堆叠容器 |
| `row([...])` | 水平排列容器 |
| `label("文本")` | 静态文本 |
| `dynamic_label(move \|\| format!(...))` | 响应式文本（State 变自动更新） |
| `button("文案")` | 按钮（链式 `.primary() .on_click(fn)`） |
| `input()` | 输入框（链式 `.placeholder() .on_change(fn)`） |
| `space(h)` | 空白间距 |

### 链式样式

所有组件都支持以下样式方法，顺序任意：

```rust
label("Hello")
    .color("#333")           // 文字颜色（支持 hex）
    .font_size(16.0)         // 字号
    .bg("#ffffff")           // 背景色
    .padding(12.0)           // 内边距（均匀）
    .padding((上, 右, 下, 左)) // 四元组
    .margin(8.0)             // 外边距
    .radius(6.0)             // 圆角
    .border(1.0, "#d9d9d9")  // 边框
    .width(200) .height(40)  // 固定尺寸
    .flex_grow(1.0)          // 弹性增长
    .gap(8.0)                // 子项间距
    .opacity(0.8)            // 透明度
```

### 条件与列表

纯 Rust 语法，没有特殊模板：

```rust
fn user_card(user: Option<User>) -> ViewNode {
    column([match user {
        Some(u) => label(u.name).font_size(18.0),
        None => label("加载中...").color("#999"),
    }])
}

fn item_list(items: Vec<Item>) -> ViewNode {
    column(items.into_iter().map(|item| {
        label(item.title).padding(8.0)
    })).gap(4.0)
}
```

---

## 状态管理

`State::new(val)` 创建响应式状态，调用 `.set()` / `.update()` 自动触发 UI 刷新。

```rust
let count = State::new(0);

count.get();                    // 读值
count.set(42);                  // 写值（自动触发重绘）
count.update(|n| *n += 1);      // 原地修改（自动触发重绘）
count.watch(|v| println!("{}", v)); // 订阅变化
```

与 `dynamic_label` 配合实现响应式文本：

```rust
let name = State::new(String::new());

dynamic_label(move || format!("你好，{}", name.get()))
    .font_size(20.0)
    .color("#1677ff")
```

**不需要手动 mark_dirty，不需要了解 DirtyRegion、增量渲染等概念**。

---

## 事件

具名回调，不用 match 19 种事件变体：

```rust
button("提交").on_click(move || submit());
input().on_change(move |val| println!("输入: {}", val));
```

按钮支持的链式方法：

| 方法 | 说明 |
|------|------|
| `.primary()` | 主按钮样式 |
| `.danger()` | 红色危险按钮 |
| `.on_click(fn)` | 点击回调（`FnMut() + 'static`） |

输入框支持的链式方法：

| 方法 | 说明 |
|------|------|
| `.placeholder("提示")` | 占位文本 |
| `.on_change(fn)` | 输入变化回调（`FnMut(&str) + 'static`） |

---

## 启动应用

```rust
App::new()
    .title("我的应用")        // 窗口标题
    .size(1024, 768)          // 窗口尺寸
    .theme(Theme::antd_dark()) // 可选：设置主题
    .root(my_view)            // 设置根视图
    .run();                   // 启动（阻塞）
```

内部自动完成：创建平台 → 创建窗口 → 初始化渲染引擎 → 加载字体 → 展开 View 树 → 进入事件循环。

---

## 内置组件

所有组件在 `uix::ui::*` 下直接使用：

**通用**：`Button` `Icon` `Label` `Space` `Typography`

**布局**：`Container` `Layout` `Grid` `Divider` `Splitter` `ScrollView` `VirtualScroll`

**导航**：`Menu` `Navigation` `Tabs` `Breadcrumb` `Dropdown` `Anchor` `Pagination` `Steps`

**输入**：`Input` `InputNumber` `Select` `Checkbox` `Radio` `Switch` `Slider` `DatePicker` `TimePicker` `ColorPicker` `Rate`

**展示**：`Table` `List` `Tree` `Calendar` `Carousel` `Image` `Badge` `Avatar` `Tag` `Collapse` `Timeline`

**反馈**：`Alert` `Modal` `Drawer` `Message` `Notification` `Popconfirm` `Popover` `Tooltip` `Spin` `Skeleton`

**图表**：`BarChart` `LineChart` `PieChart`

```rust
use uix::ui::*;

Button::new("确定").primary().size(ButtonSize::Large);
Input::new().placeholder("请输入");
Modal::new().title("提示").visible(is_open).on_ok(confirm);
ScrollView::new(ScrollDirection::Vertical).flex_grow(1.0);
```

---

## 布局

- **容器属性**：`.dir(FlexDirection::Row)` `.justify(JustifyContent::SpaceBetween)` `.align(AlignItems::Center)` `.gap(8.0)` `.wrap(true)`
- **子项属性**：`.flex_grow(1.0)` `.flex_shrink(0.0)` `.align_self(AlignItems::Center)`

```rust
// 等宽三列
row([
    label("A").flex_grow(1.0).bg("#1677ff"),
    label("B").flex_grow(1.0).bg("#52c41a"),
    label("C").flex_grow(1.0).bg("#ff4d4f"),
]).gap(8.0)
```

---

## 常用模式

**带 State 的计数器**：

```rust
fn counter() -> ViewNode {
    let count = State::new(0);
    column([
        dynamic_label(move || format!("值: {}", count.get())).font_size(28.0),
        row([
            button("+1").primary().on_click(move || count += 1),
            button("-1").on_click(move || { if count.get() > 0 { count -= 1; } }),
            button("归零").on_click(move || count.set(0)),
        ]).gap(8),
    ]).gap(16).padding(24.0)
}
```

**Todo 列表**：

```rust
fn todo_list() -> ViewNode {
    let items = State::new(vec!["学习 UIX"]);
    let text = State::new(String::new());
    column([
        row([
            input().placeholder("新任务").on_change(move |v| text.set(v.to_string())),
            button("添加").primary().on_click(move || {
                let t = text.get();
                if !t.is_empty() { items.update(|list| list.push(t)); text.set(String::new()); }
            }),
        ]).gap(8),
        column(items.get().into_iter().enumerate().map(|(i, item)| {
            let idx = i; let text = item;
            row([label(text).flex_grow(1.0), button("删除").danger().on_click(move || {
                items.update(|list| { list.remove(idx); });
            })]).gap(8)
        })).gap(4),
    ]).gap(12).padding(24.0)
}
```

---

## 旧 API（define_widget!）

如果内置组件不满足需求，可以使用 `define_widget!` 宏定义自定义组件：

```rust
use uix::ui::*;
use uix_platform::{Rect, Size};
use uix_graphics::{Color, GraphicsEngine};

define_widget! {
    pub MyButton { count: i32 }
    @new -> Self { Self { count: 0 } }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        ctx.fill_rect(frame, Color::from_rgb(24, 144, 255), Some((6.0).into()));
        ctx.text_center(&self.count.to_string(), frame, Color::white(), 14.0);
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseUp { .. } => { self.count += 1; EventResult::Handled }
            _ => EventResult::NotHandled,
        }
    }
}
```

旧 API 与简化 API 可以在同一个项目中混合使用。

---

## 测试

```bash
cargo test                    # 全部测试
cargo test -p uix-ui          # UI 层测试
cargo test -p uix-platform    # 平台层测试
```

---

> 更多组件用法参见 `ui/src/widgets/` 下的源码，完整示例见 `demo/src/demos/`。
