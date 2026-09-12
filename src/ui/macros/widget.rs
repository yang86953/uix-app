/// 声明组件状态，并按所列窄能力方法生成对应的组件 trait 实现与快照入口。
#[macro_export]
macro_rules! widget {
    (
        name: $widget_name:ident,
        $(#[$m:meta])*
        $vis:vis struct $name:ident { $($field:tt)* }
        $($rest:tt)*
    ) => {
        $crate::widget! {
            $(#[$m])*
            $vis struct $name { $($field)* }
            $($rest)*
        }
    };
    (
        $(#[$m:meta])* $vis:vis struct $name:ident { $($field:tt)* }
        $($rest:tt)*
    ) => {
        $crate::widget! {
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
        semantic_actions => [ $($semantic_actions:tt)* ]
        $(
            $method:ident => ( $($params:tt)* ) $(-> $ret:ty)? $body:block
        )*
    ) => {
        // E-05：把 `semantic_actions => [...]` 槽位转换为合成方法槽位后递归展开；
        // 槽位必须位于 `@new` 之后、其他方法之前。
        $crate::widget! {
            $(#[$m])* $vis $name { $($field)* }
            $(
                @new -> Self $new_body
            )?
            __semantic_actions_decl => ( &[$($semantic_actions)*] ) {}
            $(
                $method => ( $($params)* ) $(-> $ret)? $body
            )*
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
        $crate::__widget_struct! {
            [$(#[$m])* $vis struct $name]
            []
            $($field)*
        }

        $(
            impl $name {
                #[doc = concat!("创建 `", stringify!($name), "` 的默认配置实例。")]
                pub fn new() -> Self $new_body
            }
        )?

        $crate::__widget_snapshot_impl! {
            $name { $($field)* }
            ; [$(($method; ($($params)*) $(-> $ret)? $body))*]
        }

        impl $crate::ui::__private::traits::Widget for $name {
            fn as_any(&self) -> &dyn std::any::Any { self }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
            fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> { self }
            $crate::__widget_widget_snapshot_method!($name; $(($method; ($($params)*) $(-> $ret)? $body))*);
            fn capabilities(&self) -> $crate::ui::__private::traits::WidgetCapabilities {
                let mut c = $crate::ui::__private::traits::WidgetCapabilities::new();
                $(
                    match stringify!($method) {
                        "flex_basis" | "minimum_size" | "size_constraints" | "flex_layout_axes" | "measure" | "measure_natural" | "measure_from_children" | "flex_grow" | "flex_shrink" | "align_self" | "grid_cell" | "grid_column_span" | "grid_row_span" | "layout_margin" | "child_overflow_expands_parent" | "child_visible" | "measure_children" | "layout_children" | "build" | "build_view_children" =>
                            c.insert($crate::ui::__private::traits::WidgetCapabilities::LAYOUT),
                        "render" | "uses_palette" | "dirty_rect" | "children_clip" | "children_transform" | "children_opacity" | "paint_after_children" | "overlay_entry" | "overlay_entry_for_surface" | "draw_margin" =>
                            c.insert($crate::ui::__private::traits::WidgetCapabilities::RENDER),
                        "on_event" | "on_focus_within" | "take_layout_request" | "scroll_delta" | "scroll_delta_for_dirty" | "scroll_composite_viewport" | "viewport_scroll_offset" | "scroll_descendant_by" | "active_timer" | "wants_capture_phase" | "wants_continuous_pointer_move" | "hit_test_frame" | "hit_test_children" =>
                            c.insert($crate::ui::__private::traits::WidgetCapabilities::EVENT),
                        "on_init" | "on_attach" | "on_mount" | "on_active" | "on_inactive" | "on_theme_changed" | "on_unmount" | "on_detach" | "on_destroy" =>
                            c.insert($crate::ui::__private::traits::WidgetCapabilities::LIFECYCLE),
                        "update_animation" | "dirty_bounds" =>
                            c.insert($crate::ui::__private::traits::WidgetCapabilities::ANIMATION),
                        "text_input_cursor_rect" =>
                            c.insert($crate::ui::__private::traits::WidgetCapabilities::TEXT_INPUT),
                        _ => {}
                    }
                )*
                c
            }
            fn may_produce_overlay(&self) -> bool {
                // widget! 已知完整方法集合，可精确排除没有浮层槽位的内建组件。
                false $(|| matches!(stringify!($method), "overlay_entry" | "overlay_entry_for_surface"))*
            }
            fn may_request_event_layout(&self) -> bool {
                // widget! 已知完整事件槽位，可排除不会请求布局的组件。
                false $(|| matches!(stringify!($method), "take_layout_request"))*
            }
            fn requires_extended_event_finish(&self) -> bool {
                // 动态 owner 由语义或滚动槽位覆盖；普通事件组件无需完整收尾。
                false $(|| matches!(
                    stringify!($method),
                    "take_window_action"
                        | "semantic_event"
                        | "scroll_delta"
                        | "scroll_delta_for_dirty"
                        | "scroll_composite_viewport"
                        | "viewport_scroll_offset"
                        | "scroll_descendant_by"
                ))*
            }
            $(
                $crate::__widget_build_method!($method; ($($params)*) $(-> $ret)? $body);
            )*
            $(
                $crate::__widget_text_input_upcast_method!($method; $name);
            )*

            $crate::wc_upcast!($name; WidgetLayout);
            $crate::wc_upcast!($name; WidgetRender);
            $crate::wc_upcast!($name; EventHandler);
            $crate::wc_upcast!($name; WidgetLifecycle);
            $crate::wc_upcast!($name; WidgetAnimation);
        }

        $crate::__widget_grouped_impl! {
            WidgetLayout,
            $name,
            [flex_basis minimum_size size_constraints flex_layout_axes measure measure_natural measure_from_children flex_grow flex_shrink align_self grid_cell grid_column_span grid_row_span layout_margin child_overflow_expands_parent child_visible measure_children layout_children],
            [$(
                ($method, ($($params)*) $(-> $ret)? $body)
            )*]
        }
        $crate::__widget_grouped_impl! {
            WidgetRender,
            $name,
            [render uses_palette dirty_rect children_clip children_transform children_opacity paint_after_children overlay_entry overlay_entry_for_surface draw_margin],
            [$(
                ($method, ($($params)*) $(-> $ret)? $body)
            )*]
        }
        $crate::__widget_grouped_impl! {
            EventHandler,
            $name,
            [on_event on_focus_within semantic_event take_layout_request scroll_delta scroll_delta_for_dirty scroll_composite_viewport viewport_scroll_offset scroll_descendant_by active_timer wants_capture_phase wants_continuous_pointer_move hit_test_frame hit_test_children],
            [$(
                ($method, ($($params)*) $(-> $ret)? $body)
            )*]
        }
        $crate::__widget_grouped_impl! {
            WidgetLifecycle,
            $name,
            [on_init on_attach on_mount on_active on_inactive on_theme_changed on_unmount on_detach on_destroy],
            [$(
                ($method, ($($params)*) $(-> $ret)? $body)
            )*]
        }
        $crate::__widget_grouped_impl! {
            WidgetAnimation,
            $name,
            [update_animation dirty_bounds],
            [$(
                ($method, ($($params)*) $(-> $ret)? $body)
            )*]
        }
        $crate::__widget_grouped_impl! {
            WidgetTextInput,
            $name,
            [accepts_text_input text_input_cursor_rect text_edit_snapshot restore_text_edit_selection],
            [$(
                ($method, ($($params)*) $(-> $ret)? $body)
            )*]
        }
        $(
            $crate::__widget_view_children_impl!($name; $method; ($($params)*) $(-> $ret)? $body);
        )*
    };
}

// ════════════════════════════════════════════════════════════════════════════
// 辅助宏：将方法名 + 参数 + 返回类型 + body 转换为 method 定义（仅当属于指定 trait）
// ════════════════════════════════════════════════════════════════════════════

/// 如果方法属于指定 trait，生成 `fn method(params) -> Ret? { body }`；否则生成空。
#[macro_export]
#[doc(hidden)]
macro_rules! __widget_struct {
    ([$($head:tt)*] [$($out:tt)*]) => {
        $($head)* { $($out)* }
    };
    ([$($head:tt)*] [$($out:tt)*] , $($tail:tt)*) => {
        $crate::__widget_struct! {
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
        $crate::__widget_struct! {
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
        $crate::__widget_struct! {
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
macro_rules! __widget_widget_snapshot_method {
    ($name:ident; (snapshot; ($($p:tt)*) -> $ret:ty $body:block) $($rest:tt)*) => {
        fn snapshot_fields($($p)*) -> $crate::ui::WidgetSnapshotFields {
            let fields: $ret = $body;
            fields.into()
        }
    };
    ($name:ident; ($($other:tt)*) $($rest:tt)*) => {
        $crate::__widget_widget_snapshot_method!($name; $($rest)*);
    };
    ($name:ident;) => {
        fn snapshot_fields(&self) -> $crate::ui::WidgetSnapshotFields {
            $crate::ui::SnapshotSource::snapshot_fields(self).into()
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __widget_snapshot_impl {
    ($name:ident { $($field:tt)* }; [(snapshot; $($spec:tt)*) $($rest:tt)*]) => {};
    ($name:ident { $($field:tt)* }; [($($other:tt)*) $($rest:tt)*]) => {
        $crate::__widget_snapshot_impl!($name { $($field)* }; [$($rest)*]);
    };
    ($name:ident { $($field:tt)* }; []) => {
        $crate::__widget_snapshot_impl!($name { $($field)* });
    };
    ($name:ident { $($field:tt)* }) => {
        impl $crate::ui::SnapshotSource for $name {
            #[allow(clippy::vec_init_then_push)]
            fn snapshot_fields(&self) -> $crate::ui::WidgetSnapshotFields {
                #[allow(unused_mut, clippy::vec_init_then_push)]
                let mut fields = Vec::new();
                $crate::__widget_snapshot_collect_fields!(fields, self, $($field)*);
                $crate::ui::WidgetSnapshotFields::custom(stringify!($name), fields)
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __widget_snapshot_collect_fields {
    ($fields:ident, $this:ident,) => {};
    ($fields:ident, $this:ident) => {};
    ($fields:ident, $this:ident, #[snapshot(skip)] $(#[$field_attr:meta])* pub $field_name:ident : $field_ty:ty, $($tail:tt)*) => {
        $crate::__widget_snapshot_collect_fields!($fields, $this, $($tail)*);
    };
    ($fields:ident, $this:ident, #[snapshot(skip)] $(#[$field_attr:meta])* pub $field_name:ident : $field_ty:ty) => {};
    ($fields:ident, $this:ident, $(#[$field_attr:meta])* pub $field_name:ident : $field_ty:ty, $($tail:tt)*) => {
        $fields.push($crate::ui::SnapshotField::debug(
            stringify!($field_name),
            &$this.$field_name,
        ));
        $crate::__widget_snapshot_collect_fields!($fields, $this, $($tail)*);
    };
    ($fields:ident, $this:ident, $(#[$field_attr:meta])* pub $field_name:ident : $field_ty:ty) => {
        $fields.push($crate::ui::SnapshotField::debug(
            stringify!($field_name),
            &$this.$field_name,
        ));
    };
    ($fields:ident, $this:ident, $(#[$field_attr:meta])* pub($($scope:tt)*) $field_name:ident : $field_ty:ty, $($tail:tt)*) => {
        $crate::__widget_snapshot_collect_fields!($fields, $this, $($tail)*);
    };
    ($fields:ident, $this:ident, $(#[$field_attr:meta])* pub($($scope:tt)*) $field_name:ident : $field_ty:ty) => {};
    ($fields:ident, $this:ident, $(#[$field_attr:meta])* $field_name:ident : $field_ty:ty, $($tail:tt)*) => {
        $crate::__widget_snapshot_collect_fields!($fields, $this, $($tail)*);
    };
    ($fields:ident, $this:ident, $(#[$field_attr:meta])* $field_name:ident : $field_ty:ty) => {};
}

#[macro_export]
#[doc(hidden)]
macro_rules! __widget_method_builder {
    // ── WidgetLayout ──
    (measure; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn measure($($p)*) -> $ret $body
    };
    // 生成组件独立声明的自然内容测量实现。
    (flex_basis; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn flex_basis($($p)*) -> $ret $body
    };
    (minimum_size; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn minimum_size($($p)*) -> $ret $body
    };
    (size_constraints; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn size_constraints($($p)*) -> $ret $body
    };
    (flex_layout_axes; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn flex_layout_axes($($p)*) -> $ret $body
    };
    (measure_natural; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        // 保留组件声明提供的自然测量窄契约签名。
        fn measure_natural($($p)*) -> $ret $body
    };
    // 生成透明包装组件的同轮直接子节点测量实现。
    (measure_from_children; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        // 保留组件声明提供的完整窄契约签名。
        fn measure_from_children($($p)*) -> $ret $body
    };
    (flex_grow; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn flex_grow($($p)*) -> $ret $body
    };
    (flex_shrink; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn flex_shrink($($p)*) -> $ret $body
    };
    (align_self; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn align_self($($p)*) -> $ret $body
    };
    (grid_cell; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn grid_cell($($p)*) -> $ret $body
    };
    (grid_column_span; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn grid_column_span($($p)*) -> $ret $body
    };
    (grid_row_span; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn grid_row_span($($p)*) -> $ret $body
    };
    (layout_margin; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn layout_margin($($p)*) -> $ret $body
    };
    (child_overflow_expands_parent; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn child_overflow_expands_parent($($p)*) -> $ret $body
    };
    (child_visible; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn child_visible($($p)*) -> $ret $body
    };
    (measure_children; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn measure_children($($p)*) -> $ret $body
    };
    (measure_children_into; WidgetLayout; ($($p:tt)*) $body:block) => {
        fn measure_children_into($($p)*) $body
    };
    (layout_children; WidgetLayout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn layout_children($($p)*) -> $ret $body
    };
    (layout_children_into; WidgetLayout; ($($p:tt)*) $body:block) => {
        fn layout_children_into($($p)*) $body
    };
    // ── WidgetRender ──
    (render; WidgetRender; (&$this:ident, $frame:ident : $frame_ty:ty, $ctx:ident : &mut $ctx_ty:ty) $body:block) => {
        fn render(
            &$this,
            $frame: $frame_ty,
            $ctx: &mut $ctx_ty,
            _tree: &$crate::ui::__private::WidgetTree,
        ) $body
    };
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
    (children_transform; WidgetRender; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn children_transform($($p)*) -> $ret $body
    };
    (children_opacity; WidgetRender; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn children_opacity($($p)*) -> $ret $body
    };
    (paint_after_children; WidgetRender; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn paint_after_children($($p)*) -> $ret $body
    };
    (overlay_entry; WidgetRender; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn overlay_entry($($p)*) -> $ret $body
    };
    // 将显式表面浮层方法归入渲染能力实现。
    (overlay_entry_for_surface; WidgetRender; ($($p:tt)*) -> $ret:ty $body:block) => {
        // 原样生成组件声明的方法签名与方法体。
        fn overlay_entry_for_surface($($p)*) -> $ret $body
    };
    (draw_margin; WidgetRender; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn draw_margin($($p)*) -> $ret $body
    };
    // ── EventHandler ──
    (on_event; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn on_event($($p)*) -> $ret $body
    };
    (on_focus_within; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn on_focus_within($($p)*) -> $ret $body
    };
    (semantic_event; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn semantic_event($($p)*) -> $ret $body
    };
    (take_layout_request; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn take_layout_request($($p)*) -> $ret $body
    };
    (scroll_delta; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn scroll_delta($($p)*) -> $ret $body
    };
    (scroll_delta_for_dirty; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn scroll_delta_for_dirty($($p)*) -> $ret $body
    };
    (scroll_composite_viewport; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn scroll_composite_viewport($($p)*) -> $ret $body
    };
    (viewport_scroll_offset; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn viewport_scroll_offset($($p)*) -> $ret $body
    };
    (scroll_descendant_by; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn scroll_descendant_by($($p)*) -> $ret $body
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
    (hit_test_children; EventHandler; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn hit_test_children($($p)*) -> $ret $body
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
    // 将自然内容测量声明归入 WidgetLayout 实现。
    (WidgetLayout, flex_basis, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn flex_basis($($p)*) -> $ret $body
    };
    (WidgetLayout, minimum_size, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn minimum_size($($p)*) -> $ret $body
    };
    (WidgetLayout, size_constraints, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn size_constraints($($p)*) -> $ret $body
    };
    (WidgetLayout, flex_layout_axes, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn flex_layout_axes($($p)*) -> $ret $body
    };
    (WidgetLayout, measure_natural, ($($p:tt)*) -> $ret:ty $body:block) => {
        // 原样生成组件提供的自然尺寸实现。
        fn measure_natural($($p)*) -> $ret $body
    };
    // 将透明包装测量声明归入 WidgetLayout 实现。
    (WidgetLayout, measure_from_children, ($($p:tt)*) -> $ret:ty $body:block) => {
        // 原样生成组件提供的同轮子节点测量实现。
        fn measure_from_children($($p)*) -> $ret $body
    };
    (WidgetLayout, flex_grow, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn flex_grow($($p)*) -> $ret $body
    };
    (WidgetLayout, flex_shrink, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn flex_shrink($($p)*) -> $ret $body
    };
    (WidgetLayout, align_self, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn align_self($($p)*) -> $ret $body
    };
    (WidgetLayout, grid_cell, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn grid_cell($($p)*) -> $ret $body
    };
    (WidgetLayout, grid_column_span, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn grid_column_span($($p)*) -> $ret $body
    };
    (WidgetLayout, grid_row_span, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn grid_row_span($($p)*) -> $ret $body
    };
    (WidgetLayout, layout_margin, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn layout_margin($($p)*) -> $ret $body
    };
    (WidgetLayout, child_overflow_expands_parent, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn child_overflow_expands_parent($($p)*) -> $ret $body
    };
    (WidgetLayout, child_visible, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn child_visible($($p)*) -> $ret $body
    };
    (WidgetLayout, measure_children, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn measure_children($($p)*) -> $ret $body
    };
    (WidgetLayout, measure_children_into, ($($p:tt)*) $body:block) => {
        fn measure_children_into($($p)*) $body
    };
    (WidgetLayout, layout_children, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn layout_children($($p)*) -> $ret $body
    };
    (WidgetLayout, layout_children_into, ($($p:tt)*) $body:block) => {
        fn layout_children_into($($p)*) $body
    };
    // ── WidgetRender ──
    (WidgetRender, render, (&$this:ident, $frame:ident : $frame_ty:ty, $ctx:ident : &mut $ctx_ty:ty) $body:block) => {
        fn render(
            &$this,
            $frame: $frame_ty,
            $ctx: &mut $ctx_ty,
            _tree: &$crate::ui::__private::WidgetTree,
        ) $body
    };
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
    (WidgetRender, children_transform, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn children_transform($($p)*) -> $ret $body
    };
    (WidgetRender, children_opacity, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn children_opacity($($p)*) -> $ret $body
    };
    (WidgetRender, paint_after_children, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn paint_after_children($($p)*) -> $ret $body
    };
    (WidgetRender, overlay_entry, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn overlay_entry($($p)*) -> $ret $body
    };
    // 将显式表面浮层方法匹配到渲染能力 trait。
    (WidgetRender, overlay_entry_for_surface, ($($p:tt)*) -> $ret:ty $body:block) => {
        // 原样生成组件声明的方法签名与方法体。
        fn overlay_entry_for_surface($($p)*) -> $ret $body
    };
    (WidgetRender, draw_margin, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn draw_margin($($p)*) -> $ret $body
    };
    // ── EventHandler ──
    (EventHandler, on_event, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn on_event($($p)*) -> $ret $body
    };
    (EventHandler, on_focus_within, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn on_focus_within($($p)*) -> $ret $body
    };
    (EventHandler, semantic_event, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn semantic_event($($p)*) -> $ret $body
    };
    (EventHandler, take_layout_request, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn take_layout_request($($p)*) -> $ret $body
    };
    (EventHandler, scroll_delta, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn scroll_delta($($p)*) -> $ret $body
    };
    (EventHandler, scroll_delta_for_dirty, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn scroll_delta_for_dirty($($p)*) -> $ret $body
    };
    (EventHandler, scroll_composite_viewport, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn scroll_composite_viewport($($p)*) -> $ret $body
    };
    (EventHandler, viewport_scroll_offset, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn viewport_scroll_offset($($p)*) -> $ret $body
    };
    (EventHandler, scroll_descendant_by, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn scroll_descendant_by($($p)*) -> $ret $body
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
    (EventHandler, hit_test_children, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn hit_test_children($($p)*) -> $ret $body
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
    (WidgetTextInput, accepts_text_input, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn accepts_text_input($($p)*) -> $ret $body
    };
    (WidgetTextInput, text_input_cursor_rect, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn text_input_cursor_rect($($p)*) -> $ret $body
    };
    (WidgetTextInput, text_edit_snapshot, ($($p:tt)*) -> $ret:ty $body:block) => {
        fn text_edit_snapshot($($p)*) -> $ret $body
    };
    (WidgetTextInput, restore_text_edit_selection, ($($p:tt)*) $body:block) => {
        fn restore_text_edit_selection($($p)*) $body
    };
    // ── 不属于此 trait：跳过 ──
    ($trait:ident, $method:ident, $($rest:tt)*) => {};
}

/// 生成一个 trait 的 impl 块，包含该 trait 所有匹配的方法。
///
/// 格式: `(TraitName, Type, [allowed_method_ids], [(method, (params), -> Ret?, body)])`
#[macro_export]
#[doc(hidden)]
macro_rules! __widget_grouped_impl {
    // ── WidgetLayout ──
    (WidgetLayout, $T:ty, [$($allowed:ident)*], [$(($method:ident, ($($p:tt)*) $(-> $ret:ty)? $body:block))*]) => {
        impl $crate::ui::__private::traits::WidgetLayout for $T {
            $(
                $crate::__match_trait_method!(WidgetLayout, $method, ($($p)*) $(-> $ret)? $body);
            )*
        }
    };
    // ── WidgetRender ──
    (WidgetRender, $T:ty, [$($allowed:ident)*], [$(($method:ident, ($($p:tt)*) $(-> $ret:ty)? $body:block))*]) => {
        impl $crate::ui::__private::traits::WidgetRender for $T {
            $(
                $crate::__match_trait_method!(WidgetRender, $method, ($($p)*) $(-> $ret)? $body);
            )*
        }
    };
    // ── EventHandler ──
    (EventHandler, $T:ty, [$($allowed:ident)*], [$(($method:ident, ($($p:tt)*) $(-> $ret:ty)? $body:block))*]) => {
        impl $crate::ui::__private::traits::EventHandler for $T {
            $(
                $crate::__match_trait_method!(EventHandler, $method, ($($p)*) $(-> $ret)? $body);
            )*
        }
    };
    // ── WidgetLifecycle ──
    (WidgetLifecycle, $T:ty, [$($allowed:ident)*], [$(($method:ident, ($($p:tt)*) $(-> $ret:ty)? $body:block))*]) => {
        impl $crate::ui::__private::traits::WidgetLifecycle for $T {
            $(
                $crate::__match_trait_method!(WidgetLifecycle, $method, ($($p)*) $(-> $ret)? $body);
            )*
        }
    };
    (WidgetAnimation, $T:ty, [$($allowed:ident)*], [$(($method:ident, ($($p:tt)*) $(-> $ret:ty)? $body:block))*]) => {
        impl $crate::ui::__private::traits::WidgetAnimation for $T {
            $(
                $crate::__match_trait_method!(WidgetAnimation, $method, ($($p)*) $(-> $ret)? $body);
            )*
        }
    };
    (WidgetTextInput, $T:ty, [$($allowed:ident)*], [$(($method:ident, ($($p:tt)*) $(-> $ret:ty)? $body:block))*]) => {
        impl $crate::ui::__private::traits::WidgetTextInput for $T {
            $(
                $crate::__match_trait_method!(WidgetTextInput, $method, ($($p)*) $(-> $ret)? $body);
            )*
        }
    };
}
