//! Macros — 组件式 widget 定义。

/// 为组件生成 `WidgetComponent` 胶水代码（能力位 + 上转型）。
///
/// 组件本身是数据 struct；按需 `impl WidgetLayout / WidgetRender / …`，
/// 未覆盖的方法使用 trait 默认实现。
///
/// ```ignore
/// pub struct Button { text: String, ... }
///
/// impl_widget_component!(Button; Layout, Render, Event, Lifecycle; tab_index => 1);
///
/// impl WidgetLayout for Button {
///     fn measure(&self, constraints: Constraints) -> Size { ... }
/// }
/// impl WidgetRender for Button {
///     fn render(&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) { ... }
/// }
/// ```
#[macro_export]
macro_rules! impl_widget_component {
    (
        $T:ty;
        $($cap:ident),+ $(,)?
        $(; tab_index => $tab:expr)?
    ) => {
        impl $crate::ui::traits::WidgetComponent for $T {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
            fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
                self
            }
            fn capabilities(&self) -> $crate::ui::traits::WidgetCapabilities {
                let mut caps = $crate::ui::traits::WidgetCapabilities::new();
                $(
                    impl_widget_component!(@insert_cap caps $cap);
                )+
                caps
            }
            $(
                fn tab_index(&self) -> i32 {
                    $tab
                }
            )?
            $(
                impl_widget_component!(@upcast $cap);
            )+
        }
    };
    (@insert_cap $caps:ident Layout) => {
        $caps.insert($crate::ui::traits::WidgetCapabilities::LAYOUT);
    };
    (@insert_cap $caps:ident Render) => {
        $caps.insert($crate::ui::traits::WidgetCapabilities::RENDER);
    };
    (@insert_cap $caps:ident Event) => {
        $caps.insert($crate::ui::traits::WidgetCapabilities::EVENT);
    };
    (@insert_cap $caps:ident Lifecycle) => {
        $caps.insert($crate::ui::traits::WidgetCapabilities::LIFECYCLE);
    };
    (@insert_cap $caps:ident Animation) => {
        $caps.insert($crate::ui::traits::WidgetCapabilities::ANIMATION);
    };
    (@upcast Layout) => {
        fn as_layout(&self) -> Option<&dyn $crate::ui::traits::WidgetLayout> {
            Some(self)
        }
    };
    (@upcast Render) => {
        fn as_render(&self) -> Option<&dyn $crate::ui::traits::WidgetRender> {
            Some(self)
        }
        fn as_render_mut(&mut self) -> Option<&mut dyn $crate::ui::traits::WidgetRender> {
            Some(self)
        }
    };
    (@upcast Event) => {
        fn as_event(&self) -> Option<&dyn $crate::ui::traits::EventHandler> {
            Some(self)
        }
        fn as_event_mut(&mut self) -> Option<&mut dyn $crate::ui::traits::EventHandler> {
            Some(self)
        }
    };
    (@upcast Lifecycle) => {
        fn as_lifecycle(&self) -> Option<&dyn $crate::ui::traits::WidgetLifecycle> {
            Some(self)
        }
        fn as_lifecycle_mut(&mut self) -> Option<&mut dyn $crate::ui::traits::WidgetLifecycle> {
            Some(self)
        }
    };
    (@upcast Animation) => {
        fn as_animation(&self) -> Option<&dyn $crate::ui::traits::WidgetAnimation> {
            Some(self)
        }
        fn as_animation_mut(&mut self) -> Option<&mut dyn $crate::ui::traits::WidgetAnimation> {
            Some(self)
        }
    };
}

#[macro_export]
macro_rules! tree {
    ($parent:expr => [$($child:expr),+ $(,)?]) => {
        $crate::ui::core::widget::WidgetNode::new(
            Box::new($parent),
            vec![$($crate::ui::IntoWidgetNode::into_node($child)),+],
        )
    };
    ($widget:expr) => {
        $crate::ui::core::widget::WidgetNode::leaf(Box::new($widget))
    };
}

/// 辅助：能力上转型生成。
#[macro_export]
#[doc(hidden)]
macro_rules! wc_upcast {
    ($T:ty; WidgetRender) => {
        fn as_render(&self) -> Option<&dyn $crate::ui::traits::WidgetRender> {
            Some(self)
        }
        fn as_render_mut(&mut self) -> Option<&mut dyn $crate::ui::traits::WidgetRender> {
            Some(self)
        }
    };
    ($T:ty; EventHandler) => {
        fn as_event(&self) -> Option<&dyn $crate::ui::traits::EventHandler> {
            Some(self)
        }
        fn as_event_mut(&mut self) -> Option<&mut dyn $crate::ui::traits::EventHandler> {
            Some(self)
        }
    };
    ($T:ty; WidgetLifecycle) => {
        fn as_lifecycle(&self) -> Option<&dyn $crate::ui::traits::WidgetLifecycle> {
            Some(self)
        }
        fn as_lifecycle_mut(&mut self) -> Option<&mut dyn $crate::ui::traits::WidgetLifecycle> {
            Some(self)
        }
    };
    ($T:ty; WidgetAnimation) => {
        fn as_animation(&self) -> Option<&dyn $crate::ui::traits::WidgetAnimation> {
            Some(self)
        }
        fn as_animation_mut(&mut self) -> Option<&mut dyn $crate::ui::traits::WidgetAnimation> {
            Some(self)
        }
    };
    ($T:ty; WidgetLayout) => {
        fn as_layout(&self) -> Option<&dyn $crate::ui::traits::WidgetLayout> {
            Some(self)
        }
    };
}

// ── 辅助宏：build 方法和 upcast ──

/// 如果方法是 `build`，生成 `fn build(params) -> Ret { body }`。
#[macro_export]
#[doc(hidden)]
macro_rules! __component_build_method {
    (build; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn build($($p)*) -> $ret $body
    };
    (visible; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn visible($($p)*) -> $ret $body
    };
    (tab_index; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn tab_index($($p)*) -> $ret $body
    };
    (picture_policy; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn picture_policy($($p)*) -> $ret $body
    };
    (has_dynamic_content; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn has_dynamic_content($($p)*) -> $ret $body
    };
    ($other:ident; $($rest:tt)*) => {};
}

/// 每个 trait 只生成一次 upcast（主方法负责）。
#[macro_export]
#[doc(hidden)]
macro_rules! __component_upcast_method {
    (measure; $T:ty) => { $crate::wc_upcast!($T; WidgetLayout); };
    (flex_grow; $T:ty) => {};
    (flex_shrink; $T:ty) => {};
    (layout_margin; $T:ty) => {};
    (layout_children; $T:ty) => {};
    (render; $T:ty) => { $crate::wc_upcast!($T; WidgetRender); };
    (uses_palette; $T:ty) => {};
    (dirty_rect; $T:ty) => {};
    (children_clip; $T:ty) => {};
    (overlay_entry; $T:ty) => {};
    (draw_margin; $T:ty) => {};
    (on_event; $T:ty) => { $crate::wc_upcast!($T; EventHandler); };
    (scroll_delta; $T:ty) => {};
    (scroll_delta_for_dirty; $T:ty) => {};
    (viewport_scroll_offset; $T:ty) => {};
    (hit_test_frame; $T:ty) => {};
    (on_init; $T:ty) => { $crate::wc_upcast!($T; WidgetLifecycle); };
    (on_attach; $T:ty) => {};
    (on_mount; $T:ty) => {};
    (on_active; $T:ty) => {};
    (on_inactive; $T:ty) => {};
    (on_theme_changed; $T:ty) => {};
    (on_unmount; $T:ty) => {};
    (on_detach; $T:ty) => {};
    (on_destroy; $T:ty) => {};
    (update_animation; $T:ty) => { $crate::wc_upcast!($T; WidgetAnimation); };
    (dirty_bounds; $T:ty) => {};
    ($other:ident; $T:ty) => {};
}

#[macro_export]
macro_rules! component {
    (
        name: $component_name:ident,
        $(#[$m:meta])*
        $vis:vis struct $name:ident { $($field:tt)* }
        $($rest:tt)*
    ) => {
        $crate::component! {
            $(#[$m])*
            $vis struct $name { $($field)* }
            $($rest)*
        }
    };
    (
        $(#[$m:meta])* $vis:vis struct $name:ident { $($field:tt)* }
        $($rest:tt)*
    ) => {
        $crate::component! {
            $(#[$m])* $vis $name { $($field)* }
            $($rest)*
        }
    };
    (
        $(#[$m:meta])*
        $vis:vis $name:ident {
            $($field:tt)*
        }
        $(
            @new -> Self $new_body:block
        )?
        $(
            $method:ident => ( $($params:tt)* ) $(-> $ret:ty)? $body:block
        )*
    ) => {
        $crate::__component_struct! {
            [$(#[$m])* $vis struct $name]
            []
            $($field)*
        }

        $(
            impl $name {
                pub fn new() -> Self $new_body
            }
        )?

        $crate::__component_snapshot_impl! {
            $name { $($field)* }
        }

        impl $crate::ui::traits::WidgetComponent for $name {
            fn as_any(&self) -> &dyn std::any::Any { self }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
            fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> { self }
            $crate::__component_component_snapshot_method!($name);
            fn capabilities(&self) -> $crate::ui::traits::WidgetCapabilities {
                let mut c = $crate::ui::traits::WidgetCapabilities::new();
                $(
                    match stringify!($method) {
                        "measure" | "flex_grow" | "flex_shrink" | "layout_margin" | "layout_children" | "build" =>
                            c.insert($crate::ui::traits::WidgetCapabilities::LAYOUT),
                        "render" | "uses_palette" | "dirty_rect" | "children_clip" | "overlay_entry" | "draw_margin" =>
                            c.insert($crate::ui::traits::WidgetCapabilities::RENDER),
                        "on_event" | "scroll_delta" | "scroll_delta_for_dirty" | "viewport_scroll_offset" | "active_timer" | "wants_capture_phase" | "wants_continuous_pointer_move" | "hit_test_frame" =>
                            c.insert($crate::ui::traits::WidgetCapabilities::EVENT),
                        "on_init" | "on_attach" | "on_mount" | "on_active" | "on_inactive" | "on_theme_changed" | "on_unmount" | "on_detach" | "on_destroy" =>
                            c.insert($crate::ui::traits::WidgetCapabilities::LIFECYCLE),
                        "update_animation" | "dirty_bounds" =>
                            c.insert($crate::ui::traits::WidgetCapabilities::ANIMATION),
                        _ => {}
                    }
                )*
                c
            }
            $(
                $crate::__component_build_method!($method; ($($params)*) $(-> $ret)? $body);
            )*

            $crate::wc_upcast!($name; WidgetLayout);
            $crate::wc_upcast!($name; WidgetRender);
            $crate::wc_upcast!($name; EventHandler);
            $crate::wc_upcast!($name; WidgetLifecycle);
            $crate::wc_upcast!($name; WidgetAnimation);
        }

        $crate::__component_grouped_impl! {
            WidgetLayout,
            $name,
            [measure flex_grow flex_shrink layout_margin layout_children],
            [$(
                ($method, ($($params)*) $(-> $ret)? $body)
            )*]
        }
        $crate::__component_grouped_impl! {
            WidgetRender,
            $name,
            [render uses_palette dirty_rect children_clip overlay_entry draw_margin],
            [$(
                ($method, ($($params)*) $(-> $ret)? $body)
            )*]
        }
        $crate::__component_grouped_impl! {
            EventHandler,
            $name,
            [on_event scroll_delta scroll_delta_for_dirty viewport_scroll_offset active_timer wants_capture_phase wants_continuous_pointer_move hit_test_frame],
            [$(
                ($method, ($($params)*) $(-> $ret)? $body)
            )*]
        }
        $crate::__component_grouped_impl! {
            WidgetLifecycle,
            $name,
            [on_init on_attach on_mount on_active on_inactive on_theme_changed on_unmount on_detach on_destroy],
            [$(
                ($method, ($($params)*) $(-> $ret)? $body)
            )*]
        }
        $crate::__component_grouped_impl! {
            WidgetAnimation,
            $name,
            [update_animation dirty_bounds],
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
macro_rules! __component_struct {
    ([$($head:tt)*] [$($out:tt)*]) => {
        $($head)* { $($out)* }
    };
    ([$($head:tt)*] [$($out:tt)*] , $($tail:tt)*) => {
        $crate::__component_struct! {
            [$($head)*]
            [$($out)*]
            $($tail)*
        }
    };
    (
        [$($head:tt)*]
        [$($out:tt)*]
        #[snapshot(skip)]
        $(#[$field_attr:meta])*
        $field_vis:vis $field_name:ident : $field_ty:ty,
        $($tail:tt)*
    ) => {
        $crate::__component_struct! {
            [$($head)*]
            [$($out)* $(#[$field_attr])* $field_vis $field_name: $field_ty,]
            $($tail)*
        }
    };
    (
        [$($head:tt)*]
        [$($out:tt)*]
        #[snapshot(skip)]
        $(#[$field_attr:meta])*
        $field_vis:vis $field_name:ident : $field_ty:ty
    ) => {
        $($head)* {
            $($out)*
            $(#[$field_attr])*
            $field_vis $field_name: $field_ty,
        }
    };
    (
        [$($head:tt)*]
        [$($out:tt)*]
        $(#[$field_attr:meta])*
        $field_vis:vis $field_name:ident : $field_ty:ty,
        $($tail:tt)*
    ) => {
        $crate::__component_struct! {
            [$($head)*]
            [$($out)* $(#[$field_attr])* $field_vis $field_name: $field_ty,]
            $($tail)*
        }
    };
    (
        [$($head:tt)*]
        [$($out:tt)*]
        $(#[$field_attr:meta])*
        $field_vis:vis $field_name:ident : $field_ty:ty
    ) => {
        $($head)* {
            $($out)*
            $(#[$field_attr])*
            $field_vis $field_name: $field_ty,
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __component_component_snapshot_method {
    (Label) => {};
    (Input) => {};
    (Container) => {};
    (Grid) => {};
    ($name:ident) => {
        fn snapshot_fields(&self) -> $crate::ui::SnapshotFields {
            let typed = $crate::ui::component_snapshot::snapshot_fields_from_any(self.as_any());
            match typed {
                $crate::ui::SnapshotFields::Unknown => {
                    $crate::ui::SnapshotSource::snapshot_fields(self)
                }
                fields => fields,
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __component_snapshot_impl {
    (Label { $($field:tt)* }) => {};
    (Input { $($field:tt)* }) => {};
    (Container { $($field:tt)* }) => {};
    (Grid { $($field:tt)* }) => {};
    ($name:ident { $($field:tt)* }) => {
        impl $crate::ui::SnapshotSource for $name {
            #[allow(clippy::vec_init_then_push)]
            fn snapshot_fields(&self) -> $crate::ui::SnapshotFields {
                #[allow(unused_mut, clippy::vec_init_then_push)]
                let mut fields = Vec::new();
                $crate::__component_snapshot_collect_fields!(fields, self, $($field)*);
                $crate::ui::SnapshotFields::Custom {
                    widget: stringify!($name),
                    fields,
                }
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __component_snapshot_collect_fields {
    ($fields:ident, $this:ident,) => {};
    ($fields:ident, $this:ident) => {};
    ($fields:ident, $this:ident, #[snapshot(skip)] $(#[$field_attr:meta])* pub $field_name:ident : $field_ty:ty, $($tail:tt)*) => {
        $crate::__component_snapshot_collect_fields!($fields, $this, $($tail)*);
    };
    ($fields:ident, $this:ident, #[snapshot(skip)] $(#[$field_attr:meta])* pub $field_name:ident : $field_ty:ty) => {};
    ($fields:ident, $this:ident, $(#[$field_attr:meta])* pub $field_name:ident : $field_ty:ty, $($tail:tt)*) => {
        $fields.push($crate::ui::SnapshotField::debug(
            stringify!($field_name),
            &$this.$field_name,
        ));
        $crate::__component_snapshot_collect_fields!($fields, $this, $($tail)*);
    };
    ($fields:ident, $this:ident, $(#[$field_attr:meta])* pub $field_name:ident : $field_ty:ty) => {
        $fields.push($crate::ui::SnapshotField::debug(
            stringify!($field_name),
            &$this.$field_name,
        ));
    };
    ($fields:ident, $this:ident, $(#[$field_attr:meta])* pub($($scope:tt)*) $field_name:ident : $field_ty:ty, $($tail:tt)*) => {
        $crate::__component_snapshot_collect_fields!($fields, $this, $($tail)*);
    };
    ($fields:ident, $this:ident, $(#[$field_attr:meta])* pub($($scope:tt)*) $field_name:ident : $field_ty:ty) => {};
    ($fields:ident, $this:ident, $(#[$field_attr:meta])* $field_name:ident : $field_ty:ty, $($tail:tt)*) => {
        $crate::__component_snapshot_collect_fields!($fields, $this, $($tail)*);
    };
    ($fields:ident, $this:ident, $(#[$field_attr:meta])* $field_name:ident : $field_ty:ty) => {};
}

#[macro_export]
#[doc(hidden)]
macro_rules! __component_method_builder {
    // ── WidgetLayout ──
    (measure; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn measure($($p)*) -> $ret $body
    };
    (flex_grow; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn flex_grow($($p)*) -> $ret $body
    };
    (flex_shrink; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn flex_shrink($($p)*) -> $ret $body
    };
    (layout_margin; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn layout_margin($($p)*) -> $ret $body
    };
    (layout_children; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn layout_children($($p)*) -> $ret $body
    };
    // ── WidgetRender ──
    (render; WidgetRender; ($($p:tt)*) $body:block) => {
        fn render($($p)*) $body
    };
    (uses_palette; WidgetRender; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn uses_palette($($p)*) -> $ret $body
    };
    (dirty_rect; WidgetRender; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn dirty_rect($($p)*) -> $ret $body
    };
    (children_clip; WidgetRender; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn children_clip($($p)*) -> $ret $body
    };
    (overlay_entry; WidgetRender; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn overlay_entry($($p)*) -> $ret $body
    };
    (draw_margin; WidgetRender; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn draw_margin($($p)*) -> $ret $body
    };
    // ── EventHandler ──
    (on_event; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn on_event($($p)*) -> $ret $body
    };
    (semantic_event; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn semantic_event($($p)*) -> $ret $body
    };
    (scroll_delta; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn scroll_delta($($p)*) -> $ret $body
    };
    (scroll_delta_for_dirty; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn scroll_delta_for_dirty($($p)*) -> $ret $body
    };
    (viewport_scroll_offset; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn viewport_scroll_offset($($p)*) -> $ret $body
    };
    (active_timer; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn active_timer($($p)*) -> $ret $body
    };
    (wants_capture_phase; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn wants_capture_phase($($p)*) -> $ret $body
    };
    (wants_continuous_pointer_move; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn wants_continuous_pointer_move($($p)*) -> $ret $body
    };
    (hit_test_frame; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn hit_test_frame($($p)*) -> $ret $body
    };
    // ── WidgetLifecycle ──
    (on_init; WidgetLifecycle; ($($p:tt)*) $body:block) => {
        fn on_init($($p)*) $body
    };
    (on_attach; WidgetLifecycle; ($($p:tt)*) $body:block) => {
        fn on_attach($($p)*) $body
    };
    (on_mount; WidgetLifecycle; ($($p:tt)*) $body:block) => {
        fn on_mount($($p)*) $body
    };
    (on_active; WidgetLifecycle; ($($p:tt)*) $body:block) => {
        fn on_active($($p)*) $body
    };
    (on_inactive; WidgetLifecycle; ($($p:tt)*) $body:block) => {
        fn on_inactive($($p)*) $body
    };
    (on_theme_changed; WidgetLifecycle; ($($p:tt)*) $body:block) => {
        fn on_theme_changed($($p)*) $body
    };
    (on_unmount; WidgetLifecycle; ($($p:tt)*) $body:block) => {
        fn on_unmount($($p)*) $body
    };
    (on_detach; WidgetLifecycle; ($($p:tt)*) $body:block) => {
        fn on_detach($($p)*) $body
    };
    (on_destroy; WidgetLifecycle; ($($p:tt)*) $body:block) => {
        fn on_destroy($($p)*) $body
    };
    // ── WidgetAnimation ──
    (update_animation; WidgetAnimation; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn update_animation($($p)*) -> $ret $body
    };
    (dirty_bounds; WidgetAnimation; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn dirty_bounds($($p)*) -> $ret $body
    };
    // ── 非此 trait 的方法：跳过 ──
    ($method:ident; $trait:ident; $($rest:tt)*) => {};
}

/// 如果方法属于指定 trait，生成 `fn ...` 定义。
#[macro_export]
#[doc(hidden)]
macro_rules! __match_trait_method {
    // ── WidgetLayout ──
    (WidgetLayout, measure, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn measure($($p)*) -> $ret $body
    };
    (WidgetLayout, flex_grow, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn flex_grow($($p)*) -> $ret $body
    };
    (WidgetLayout, flex_shrink, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn flex_shrink($($p)*) -> $ret $body
    };
    (WidgetLayout, layout_margin, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn layout_margin($($p)*) -> $ret $body
    };
    (WidgetLayout, layout_children, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn layout_children($($p)*) -> $ret $body
    };
    // ── WidgetRender ──
    (WidgetRender, render, ($($p:tt)*) $body:block) => {
        fn render($($p)*) $body
    };
    (WidgetRender, uses_palette, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn uses_palette($($p)*) -> $ret $body
    };
    (WidgetRender, dirty_rect, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn dirty_rect($($p)*) -> $ret $body
    };
    (WidgetRender, children_clip, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn children_clip($($p)*) -> $ret $body
    };
    (WidgetRender, overlay_entry, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn overlay_entry($($p)*) -> $ret $body
    };
    (WidgetRender, draw_margin, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn draw_margin($($p)*) -> $ret $body
    };
    // ── EventHandler ──
    (EventHandler, on_event, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn on_event($($p)*) -> $ret $body
    };
    (EventHandler, semantic_event, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn semantic_event($($p)*) -> $ret $body
    };
    (EventHandler, scroll_delta, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn scroll_delta($($p)*) -> $ret $body
    };
    (EventHandler, scroll_delta_for_dirty, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn scroll_delta_for_dirty($($p)*) -> $ret $body
    };
    (EventHandler, viewport_scroll_offset, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn viewport_scroll_offset($($p)*) -> $ret $body
    };
    (EventHandler, active_timer, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn active_timer($($p)*) -> $ret $body
    };
    (EventHandler, wants_capture_phase, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn wants_capture_phase($($p)*) -> $ret $body
    };
    (EventHandler, wants_continuous_pointer_move, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn wants_continuous_pointer_move($($p)*) -> $ret $body
    };
    (EventHandler, hit_test_frame, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn hit_test_frame($($p)*) -> $ret $body
    };
    // ── WidgetLifecycle ──
    (WidgetLifecycle, on_init, ($($p:tt)*) $body:block) => {
        fn on_init($($p)*) $body
    };
    (WidgetLifecycle, on_attach, ($($p:tt)*) $body:block) => {
        fn on_attach($($p)*) $body
    };
    (WidgetLifecycle, on_mount, ($($p:tt)*) $body:block) => {
        fn on_mount($($p)*) $body
    };
    (WidgetLifecycle, on_active, ($($p:tt)*) $body:block) => {
        fn on_active($($p)*) $body
    };
    (WidgetLifecycle, on_inactive, ($($p:tt)*) $body:block) => {
        fn on_inactive($($p)*) $body
    };
    (WidgetLifecycle, on_theme_changed, ($($p:tt)*) $body:block) => {
        fn on_theme_changed($($p)*) $body
    };
    (WidgetLifecycle, on_unmount, ($($p:tt)*) $body:block) => {
        fn on_unmount($($p)*) $body
    };
    (WidgetLifecycle, on_detach, ($($p:tt)*) $body:block) => {
        fn on_detach($($p)*) $body
    };
    (WidgetLifecycle, on_destroy, ($($p:tt)*) $body:block) => {
        fn on_destroy($($p)*) $body
    };
    (WidgetAnimation, update_animation, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn update_animation($($p)*) -> $ret $body
    };
    (WidgetAnimation, dirty_bounds, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn dirty_bounds($($p)*) -> $ret $body
    };
    // ── 不属于此 trait：跳过 ──
    ($trait:ident, $method:ident, $($rest:tt)*) => {};
}

/// 生成一个 trait 的 impl 块，包含该 trait 所有匹配的方法。
///
/// 格式: `(TraitName, Type, [allowed_method_ids], [(method, (params), -> Ret?, body)])`
#[macro_export]
#[doc(hidden)]
macro_rules! __component_grouped_impl {
    // ── WidgetLayout ──
    (WidgetLayout, $T:ty, [$($allowed:ident)*], [$(($method:ident, ($($p:tt)*) $(-> $ret:ty)? $body:block))*]) => {
        impl $crate::ui::traits::WidgetLayout for $T {
            $(
                $crate::__match_trait_method!(WidgetLayout, $method, ($($p)*) $(-> $ret)? $body);
            )*
        }
    };
    // ── WidgetRender ──
    (WidgetRender, $T:ty, [$($allowed:ident)*], [$(($method:ident, ($($p:tt)*) $(-> $ret:ty)? $body:block))*]) => {
        impl $crate::ui::traits::WidgetRender for $T {
            $(
                $crate::__match_trait_method!(WidgetRender, $method, ($($p)*) $(-> $ret)? $body);
            )*
        }
    };
    // ── EventHandler ──
    (EventHandler, $T:ty, [$($allowed:ident)*], [$(($method:ident, ($($p:tt)*) $(-> $ret:ty)? $body:block))*]) => {
        impl $crate::ui::traits::EventHandler for $T {
            $(
                $crate::__match_trait_method!(EventHandler, $method, ($($p)*) $(-> $ret)? $body);
            )*
        }
    };
    // ── WidgetLifecycle ──
    (WidgetLifecycle, $T:ty, [$($allowed:ident)*], [$(($method:ident, ($($p:tt)*) $(-> $ret:ty)? $body:block))*]) => {
        impl $crate::ui::traits::WidgetLifecycle for $T {
            $(
                $crate::__match_trait_method!(WidgetLifecycle, $method, ($($p)*) $(-> $ret)? $body);
            )*
        }
    };
    (WidgetAnimation, $T:ty, [$($allowed:ident)*], [$(($method:ident, ($($p:tt)*) $(-> $ret:ty)? $body:block))*]) => {
        impl $crate::ui::traits::WidgetAnimation for $T {
            $(
                $crate::__match_trait_method!(WidgetAnimation, $method, ($($p)*) $(-> $ret)? $body);
            )*
        }
    };
}
