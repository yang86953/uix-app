//! Macros for declarative widget tree construction and component definition.
//!
//! Provides two macros:
//!
//! - [`tree!`] — Compose widget trees declaratively (like React JSX).
//! - [`define_widget!`] — Define custom widgets with full framework access.
//!
//! # `tree!` — Declarative widget tree
//!
//! ```ignore
//! tree! {
//!     Container::new().bg(bg) => [
//!         Label::new("Hello", color).font_size(16),
//!         tree! { Container::new().size(200.0, GH) => [
//!             Button::new("Click").primary(),
//!         ]},
//!         my_function_component(props),
//!     ]
//! }
//! ```
//!
//! # `define_widget!` — Custom component definition
//!
//! ```ignore
//! define_widget! {
//!     /// Doc comment
//!     pub Counter {
//!         count: u32,
//!     }
//!
//!     // Constructor (optional)
//!     new => || -> Self { Self { count: 0 } }
//!
//!     // Widget trait methods (all optional, only override what you need)
//!     preferred_size => |&self, _engine: Option<&dyn GraphicsEngine>| -> Size {
//!         Size::new(120.0, 36.0)
//!     }
//!
//!     on_event => |&mut self, event: &WidgetEvent| -> EventResult {
//!         match event {
//!             MouseDown { .. } => { self.count += 1; Handled }
//!             _ => NotHandled
//!         }
//!     }
//!
//!     render => |&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree| {
//!         ctx.draw_text(&format!("Count: {}", self.count), ...);
//!     }
//! }
//! ```

// ────────────────────────────────────────────────────────────────────────────
// tree! — Declarative widget tree composition
// ────────────────────────────────────────────────────────────────────────────

/// Build a declarative widget tree.
///
/// Returns a `WidgetNode` that can be consumed by `WidgetTree::build()`.
///
/// # Examples
///
/// ```ignore
/// use uix::ui::tree;
///
/// tree.build(tree! {
///     Container::new().size(800.0, 600.0).bg(bg) => [
///         Label::new("Title", color).font_size(20),
///     ]
/// });
/// ```
#[macro_export]
macro_rules! tree {
    // Parent with children: widget => [child1, child2, ...]
    ($parent:expr => [$($child:expr),+ $(,)?]) => {
        $crate::widget::WidgetNode::new(
            Box::new($parent),
            vec![$($crate::widget::IntoWidgetNode::into_node($child)),+],
        )
    };

    // Leaf widget (no children)
    ($widget:expr) => {
        $crate::widget::WidgetNode::leaf(Box::new($widget))
    };
}

// ────────────────────────────────────────────────────────────────────────────
// define_widget! — Custom widget component definition
// ────────────────────────────────────────────────────────────────────────────

/// Define a custom widget with full framework access.
///
/// Generates a struct and `impl Widget for ...` with only the methods
/// you provide (others get `Widget` trait defaults).
///
/// # Method reference
///
/// | 宏段          | 对应 Widget trait 方法    | 参数                              |
/// |---------------|---------------------------|-----------------------------------|
/// | `@new`        | 无参构造器                | 仅 `-> Self`，不支持参数          |
/// | `preferred_size` | `fn preferred_size`   | `(&self, engine)`                 |
/// | `on_event`    | `fn on_event`             | `(&mut self, event: &WidgetEvent)` |
/// | `render`      | `fn render`              | `(&self, frame, ctx, tree)`       |
/// | `post_render` | `fn post_render`         | `(&self, frame, ctx, tree)`       |
/// | `on_mount`    | `fn on_mount`            | `(&mut self)`                     |
/// | `on_unmount`  | `fn on_unmount`          | `(&mut self)`                     |
/// | `on_update`   | `fn on_update`           | `(&mut self, dt: f32)`            |
/// | `build`       | `fn build`               | `(&self) -> Vec<Box<dyn Widget>>` |
/// | `visible`     | `fn visible`             | `(&self) -> bool`                 |
/// | `needs_continuous_update` | `fn needs_continuous_update` | `(&self) -> bool`     |
/// | `hit_test_frame` | `fn hit_test_frame`   | `(&self, actual_frame) -> Rect`   |
/// | `children_clip` | `fn children_clip`     | `(&self, frame) -> Option<Rect>`  |
/// | `is_repaint_boundary` | `fn is_repaint_boundary` | `(&self) -> bool`         |
///
/// # 构造器说明
///
/// `@new` 仅生成**无参** `pub fn new() -> Self`。
/// 若 widget 需要带参构造器（如 `Button::new("Click")`），
/// 请在宏外手动添加 `impl WidgetName { ... }` 块。
///
/// # 示例
///
/// ```ignore
/// define_widget! {
///     /// 一个点击计数的按钮。
///     pub Counter {
///         count: u32,
///     }
///
///     @new -> Self { Self { count: 0 } }
///
///     on_event => (&mut self, event: &WidgetEvent) -> EventResult {
///         match event {
///             WidgetEvent::MouseDown { .. } => { self.count += 1; EventResult::Handled }
///             _ => EventResult::NotHandled
///         }
///     }
///
///     render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
///         let tokens = ctx.tokens();
///         ctx.fill_rect(frame, tokens.color_primary_bg(), None);
///         ctx.draw_text(
///             &format!("Count: {}", self.count),
///             Point::new(frame.x + 8.0, frame.y + 8.0),
///             tokens.color_text(),
///             14.0,
///         );
///     }
/// }
/// ```
#[macro_export]
macro_rules! define_widget {
    (
        $(#[$struct_meta:meta])*
        $vis:vis struct $name:ident {
            $($field:tt)*
        }

        $(@ new $(-> $new_ret:ty)? $new_body:block)?

        $(
            $method:ident => ( $($params:tt)* ) $(-> $ret:ty)? $body:block
        )*
    ) => {
        $(#[$struct_meta])*
        $vis struct $name {
            $($field)*
        }

        $(
            impl $name {
                pub fn new() $(-> $new_ret)? $new_body
            }
        )?

        impl $crate::widget::Widget for $name {
            fn as_any(&self) -> &dyn std::any::Any { self }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
            $(
                fn $method( $($params)* ) $(-> $ret)? $body
            )*
        }
    };
}
