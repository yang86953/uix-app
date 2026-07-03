//! Macros — 组件式 widget 定义。

#[macro_export]
macro_rules! tree {
    ($parent:expr => [$($child:expr),+ $(,)?]) => {
        $crate::widget::WidgetNode::new(
            Box::new($parent),
            vec![$($crate::widget::IntoWidgetNode::into_node($child)),+],
        )
    };
    ($widget:expr) => {
        $crate::widget::WidgetNode::leaf(Box::new($widget))
    };
}

/// 辅助：能力上转型生成。
#[macro_export]
#[doc(hidden)]
macro_rules! wc_upcast {
    ($T:ty; WidgetRender) => {
        fn as_render(&self) -> Option<&dyn $crate::api::traits::WidgetRender> {
            Some(self)
        }
        fn as_render_mut(&mut self) -> Option<&mut dyn $crate::api::traits::WidgetRender> {
            Some(self)
        }
    };
    ($T:ty; WidgetEventHandler) => {
        fn as_event(&self) -> Option<&dyn $crate::api::traits::WidgetEventHandler> {
            Some(self)
        }
        fn as_event_mut(&mut self) -> Option<&mut dyn $crate::api::traits::WidgetEventHandler> {
            Some(self)
        }
    };
    ($T:ty; WidgetLifecycle) => {
        fn as_lifecycle(&self) -> Option<&dyn $crate::api::traits::WidgetLifecycle> {
            Some(self)
        }
        fn as_lifecycle_mut(&mut self) -> Option<&mut dyn $crate::api::traits::WidgetLifecycle> {
            Some(self)
        }
    };
    ($T:ty; WidgetLayout) => {
        fn as_layout(&self) -> Option<&dyn $crate::api::traits::WidgetLayout> {
            Some(self)
        }
    };
}

// ── 辅助宏：build 方法和 upcast ──

/// 如果方法是 `build`，生成 `fn build(params) -> Ret { body }`。
#[macro_export]
#[doc(hidden)]
macro_rules! __define_widget_build_method {
    (build; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn build($($p)*) -> $ret $body
    };
    (visible; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn visible($($p)*) -> $ret $body
    };
    (tab_index; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn tab_index($($p)*) -> $ret $body
    };
    ($other:ident; $($rest:tt)*) => {};
}

/// 每个 trait 只生成一次 upcast（主方法负责）。
#[macro_export]
#[doc(hidden)]
macro_rules! __define_widget_upcast_method {
    (preferred_size; $T:ty) => { $crate::wc_upcast!($T; WidgetLayout); };
    (flex_grow; $T:ty) => {};
    (flex_shrink; $T:ty) => {};
    (layout_children; $T:ty) => {};
    (render; $T:ty) => { $crate::wc_upcast!($T; WidgetRender); };
    (post_render; $T:ty) => {};
    (dirty_rect; $T:ty) => {};
    (children_clip; $T:ty) => {};
    (draw_margin; $T:ty) => {};
    (on_event; $T:ty) => { $crate::wc_upcast!($T; WidgetEventHandler); };
    (needs_continuous_update; $T:ty) => {};
    (scroll_delta; $T:ty) => {};
    (scroll_delta_for_dirty; $T:ty) => {};
    (hit_test_frame; $T:ty) => {};
    (on_init; $T:ty) => { $crate::wc_upcast!($T; WidgetLifecycle); };
    (on_mount; $T:ty) => {};
    (on_unmount; $T:ty) => {};
    (on_update; $T:ty) => {};
    ($other:ident; $T:ty) => {};
}

/// 定义 widget 组件。
///
/// 支持内联语法（推荐）：
/// ```ignore
/// define_widget! {
///     pub Button { text: String }
///     preferred_size => (&self, _engine) -> Size { ... }
///     render => (&self, frame, ctx, tree) { ... }
///     on_event => (&mut self, event) -> EventResult { ... }
/// }
/// ```
#[macro_export]
macro_rules! define_widget {
    // ═══ 去 struct 关键字转发 ═══
    (
        $(#[$m:meta])* $vis:vis struct $name:ident { $($field:tt)* }
        $($rest:tt)*
    ) => {
        $crate::define_widget! {
            $(#[$m])* $vis $name { $($field)* }
            $($rest)*
        }
    };

    // ═══ 主模式：内联语法 ═══
    // 支持可选的 @new 构造函数
    (
        $(#[$m:meta])*
        $vis:vis $name:ident {
            $($field:tt)*
        }
        $(
            // @new 构造函数（可选）
            @new -> Self $new_body:block
        )?
        $(
            $method:ident => ( $($params:tt)* ) $(-> $ret:ty)? $body:block
        )*
    ) => {
        $(#[$m])* $vis struct $name { $($field)* }

        // @new 构造函数（如果存在）
        $(
            impl $name {
                pub fn new() -> Self $new_body
            }
        )?

        impl $crate::api::traits::WidgetComponent for $name {
            fn as_any(&self) -> &dyn std::any::Any { self }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
            fn capabilities(&self) -> $crate::api::traits::WidgetCapabilities {
                let mut c = $crate::api::traits::WidgetCapabilities::new();
                $(
                    match stringify!($method) {
                        "preferred_size" | "flex_grow" | "flex_shrink" | "layout_children" | "build" =>
                            c.insert($crate::api::traits::WidgetCapabilities::LAYOUT),
                        "render" | "post_render" | "dirty_rect" | "children_clip" | "draw_margin" =>
                            c.insert($crate::api::traits::WidgetCapabilities::RENDER),
                        "on_event" | "needs_continuous_update" | "scroll_delta" | "scroll_delta_for_dirty" | "hit_test_frame" =>
                            c.insert($crate::api::traits::WidgetCapabilities::EVENT),
                        "on_init" | "on_mount" | "on_unmount" | "on_update" =>
                            c.insert($crate::api::traits::WidgetCapabilities::LIFECYCLE),
                        _ => {}
                    }
                )*
                c
            }
            $(
                $crate::__define_widget_build_method!($method; ($($params)*) $(-> $ret)? $body);
            )*

            // ═══ 无条件上转型 — grouped trait impl 始终存在（render/preferred_size 必定义，
            // Lifecycle/EventHandler 各方法均有默认实现），因此所有上转型始终有效 ═══
            $crate::wc_upcast!($name; WidgetLayout);
            $crate::wc_upcast!($name; WidgetRender);
            $crate::wc_upcast!($name; WidgetEventHandler);
            $crate::wc_upcast!($name; WidgetLifecycle);
        }

        // ═══ 生成 grouped trait impl 块 ═══
        // 每个 trait 只生成一个 impl 块
        $crate::__define_widget_grouped_impl! {
            WidgetLayout,
            $name,
            [preferred_size flex_grow flex_shrink layout_children],
            [$(
                ($method, ($($params)*) $(-> $ret)? $body)
            )*]
        }
        $crate::__define_widget_grouped_impl! {
            WidgetRender,
            $name,
            [render post_render dirty_rect children_clip draw_margin],
            [$(
                ($method, ($($params)*) $(-> $ret)? $body)
            )*]
        }
        $crate::__define_widget_grouped_impl! {
            WidgetEventHandler,
            $name,
            [on_event needs_continuous_update scroll_delta scroll_delta_for_dirty hit_test_frame],
            [$(
                ($method, ($($params)*) $(-> $ret)? $body)
            )*]
        }
        $crate::__define_widget_grouped_impl! {
            WidgetLifecycle,
            $name,
            [on_init on_mount on_unmount on_update],
            [$(
                ($method, ($($params)*) $(-> $ret)? $body)
            )*]
        }
    };
}

// ════════════════════════════════════════════════════════════════════════════
// 辅助宏：将方法名 + 参数 + 返回类型 + body 转换为 method 定义（仅当属于指定 trait）
// ════════════════════════════════════════════════════════════════════════════

/// 如果方法属于指定 trait，生成 `fn method(params) -> Ret? { body }`；否则生成空。
#[macro_export]
#[doc(hidden)]
macro_rules! __define_widget_method_builder {
    // ── WidgetLayout ──
    (preferred_size; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn preferred_size($($p)*) -> $ret $body
    };
    (flex_grow; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn flex_grow($($p)*) -> $ret $body
    };
    (flex_shrink; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn flex_shrink($($p)*) -> $ret $body
    };
    (layout_children; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn layout_children($($p)*) -> $ret $body
    };
    // ── WidgetRender ──
    (render; WidgetRender; ($($p:tt)*) $body:block) => {
        fn render($($p)*) $body
    };
    (post_render; WidgetRender; ($($p:tt)*) $body:block) => {
        fn post_render($($p)*) $body
    };
    (dirty_rect; WidgetRender; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn dirty_rect($($p)*) -> $ret $body
    };
    (children_clip; WidgetRender; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn children_clip($($p)*) -> $ret $body
    };
    (draw_margin; WidgetRender; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn draw_margin($($p)*) -> $ret $body
    };
    // ── WidgetEventHandler ──
    (on_event; WidgetEventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn on_event($($p)*) -> $ret $body
    };
    (needs_continuous_update; WidgetEventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn needs_continuous_update($($p)*) -> $ret $body
    };
    (scroll_delta; WidgetEventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn scroll_delta($($p)*) -> $ret $body
    };
    (scroll_delta_for_dirty; WidgetEventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn scroll_delta_for_dirty($($p)*) -> $ret $body
    };
    (hit_test_frame; WidgetEventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn hit_test_frame($($p)*) -> $ret $body
    };
    // ── WidgetLifecycle ──
    (on_init; WidgetLifecycle; ($($p:tt)*) $body:block) => {
        fn on_init($($p)*) $body
    };
    (on_mount; WidgetLifecycle; ($($p:tt)*) $body:block) => {
        fn on_mount($($p)*) $body
    };
    (on_unmount; WidgetLifecycle; ($($p:tt)*) $body:block) => {
        fn on_unmount($($p)*) $body
    };
    (on_update; WidgetLifecycle; ($($p:tt)*) $body:block) => {
        fn on_update($($p)*) $body
    };
    // ── 非此 trait 的方法：跳过 ──
    ($method:ident; $trait:ident; $($rest:tt)*) => {};
}

/// 如果方法属于指定 trait，生成 `fn ...` 定义。
#[macro_export]
#[doc(hidden)]
macro_rules! __match_trait_method {
    // ── WidgetLayout ──
    (WidgetLayout, preferred_size, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn preferred_size($($p)*) -> $ret $body
    };
    (WidgetLayout, flex_grow, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn flex_grow($($p)*) -> $ret $body
    };
    (WidgetLayout, flex_shrink, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn flex_shrink($($p)*) -> $ret $body
    };
    (WidgetLayout, layout_children, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn layout_children($($p)*) -> $ret $body
    };
    // ── WidgetRender ──
    (WidgetRender, render, ($($p:tt)*) $body:block) => {
        fn render($($p)*) $body
    };
    (WidgetRender, post_render, ($($p:tt)*) $body:block) => {
        fn post_render($($p)*) $body
    };
    (WidgetRender, dirty_rect, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn dirty_rect($($p)*) -> $ret $body
    };
    (WidgetRender, children_clip, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn children_clip($($p)*) -> $ret $body
    };
    (WidgetRender, draw_margin, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn draw_margin($($p)*) -> $ret $body
    };
    // ── WidgetEventHandler ──
    (WidgetEventHandler, on_event, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn on_event($($p)*) -> $ret $body
    };
    (WidgetEventHandler, needs_continuous_update, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn needs_continuous_update($($p)*) -> $ret $body
    };
    (WidgetEventHandler, scroll_delta, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn scroll_delta($($p)*) -> $ret $body
    };
    (WidgetEventHandler, scroll_delta_for_dirty, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn scroll_delta_for_dirty($($p)*) -> $ret $body
    };
    (WidgetEventHandler, hit_test_frame, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn hit_test_frame($($p)*) -> $ret $body
    };
    // ── WidgetLifecycle ──
    (WidgetLifecycle, on_init, ($($p:tt)*) $body:block) => {
        fn on_init($($p)*) $body
    };
    (WidgetLifecycle, on_mount, ($($p:tt)*) $body:block) => {
        fn on_mount($($p)*) $body
    };
    (WidgetLifecycle, on_unmount, ($($p:tt)*) $body:block) => {
        fn on_unmount($($p)*) $body
    };
    (WidgetLifecycle, on_update, ($($p:tt)*) $body:block) => {
        fn on_update($($p)*) $body
    };
    // ── 不属于此 trait：跳过 ──
    ($trait:ident, $method:ident, $($rest:tt)*) => {};
}

/// 生成一个 trait 的 impl 块，包含该 trait 所有匹配的方法。
///
/// 格式: `(TraitName, Type, [allowed_method_ids], [(method, (params), -> Ret?, body)])`
#[macro_export]
#[doc(hidden)]
macro_rules! __define_widget_grouped_impl {
    // ── WidgetLayout ──
    (WidgetLayout, $T:ty, [$($allowed:ident)*], [$(($method:ident, ($($p:tt)*) $(-> $ret:ty)? $body:block))*]) => {
        impl $crate::api::traits::WidgetLayout for $T {
            $(
                $crate::__match_trait_method!(WidgetLayout, $method, ($($p)*) $(-> $ret)? $body);
            )*
        }
    };
    // ── WidgetRender ──
    (WidgetRender, $T:ty, [$($allowed:ident)*], [$(($method:ident, ($($p:tt)*) $(-> $ret:ty)? $body:block))*]) => {
        impl $crate::api::traits::WidgetRender for $T {
            $(
                $crate::__match_trait_method!(WidgetRender, $method, ($($p)*) $(-> $ret)? $body);
            )*
        }
    };
    // ── WidgetEventHandler ──
    (WidgetEventHandler, $T:ty, [$($allowed:ident)*], [$(($method:ident, ($($p:tt)*) $(-> $ret:ty)? $body:block))*]) => {
        impl $crate::api::traits::WidgetEventHandler for $T {
            $(
                $crate::__match_trait_method!(WidgetEventHandler, $method, ($($p)*) $(-> $ret)? $body);
            )*
        }
    };
    // ── WidgetLifecycle ──
    (WidgetLifecycle, $T:ty, [$($allowed:ident)*], [$(($method:ident, ($($p:tt)*) $(-> $ret:ty)? $body:block))*]) => {
        impl $crate::api::traits::WidgetLifecycle for $T {
            $(
                $crate::__match_trait_method!(WidgetLifecycle, $method, ($($p)*) $(-> $ret)? $body);
            )*
        }
    };
}
