//! Macros — 组件式 widget 定义。

/// 为组件生成 `Widget` 胶水代码（能力位 + 上转型）。
///
/// 组件本身是数据 struct；按需 `impl WidgetLayout / WidgetRender / …`，
/// 未覆盖的方法使用 trait 默认实现。
///
#[macro_export]
macro_rules! impl_widget {
    (
        $T:ty;
        $($cap:ident),+ $(,)?
        // 可选 tab 索引采用 Rust 2024 表达式片段语义。
        $(; tab_index => $tab:expr)?
        // 手写 WidgetRender 只有显式声明无浮层时才可关闭保守重建。
        $(; may_produce_overlay => $may_produce_overlay:expr)?
        // 手写 EventHandler 只有显式声明不会请求布局时才可关闭保守检查。
        $(; may_request_event_layout => $may_request_event_layout:expr)?
        // 手写 EventHandler 只有显式声明无需扩展收尾时才可进入最短路径。
        $(; requires_extended_event_finish => $requires_extended_event_finish:expr)?
        // 已有直接 SnapshotSource 实现的组件可显式接入类型化快照。
        $(; snapshot_source => $snapshot_source:ident)?
        $(; reconcile_sync => $reconcile_sync:ident)?
        $(; reconcile_config => $reconcile_config:expr)?
        $(; reconcile_layout => $reconcile_layout:expr)?
        $(; reconcile_runtime => $reconcile_runtime:expr)?
        $(; declaration_style => $declaration_style:expr)?
        $(; snapshot_free_reconcile => $snapshot_free_reconcile:expr)?
        $(; interaction_disabled => $interaction_disabled:expr)?
    ) => {
        impl $crate::ui::__private::traits::Widget for $T {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
            fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
                self
            }
            $crate::impl_widget!(@snapshot_method $T $(, $snapshot_source)?);
            $(fn reconciles_without_snapshot(&self) -> bool { $snapshot_free_reconcile })?
            $(fn interaction_disabled(&self) -> Option<bool> { ($interaction_disabled)(self) })?
            $( $crate::__widget_reconcile_method!($reconcile_sync); )?
            $(fn declaration_config_changed(&self, next: &dyn $crate::ui::Widget) -> Option<bool> {
                ($reconcile_config)(self, next)
            })?
            $(fn declaration_layout_changed(&self, next: &dyn $crate::ui::Widget) -> Option<bool> {
                ($reconcile_layout)(self, next)
            })?
            $(fn declaration_runtime_changed(&self, next: &dyn $crate::ui::Widget) -> bool {
                ($reconcile_runtime)(self, next)
            })?
            $(fn apply_declaration_style(&mut self, style: &$crate::ui::Style, declared: &$crate::ui::StyleDiff, flex_grow: Option<f32>, flex_shrink: Option<f32>) {
                ($declaration_style)(self, style, declared, flex_grow, flex_shrink)
            })?
            fn capabilities(&self) -> $crate::ui::__private::traits::WidgetCapabilities {
                let mut caps = $crate::ui::__private::traits::WidgetCapabilities::new();
                $(
                    impl_widget!(@insert_cap caps $cap);
                )+
                caps
            }
            $(
                fn tab_index(&self) -> i32 {
                    $tab
                }
            )?
            $(
                fn may_produce_overlay(&self) -> bool {
                    $may_produce_overlay
                }
            )?
            $(
                fn may_request_event_layout(&self) -> bool {
                    $may_request_event_layout
                }
            )?
            $(
                fn requires_extended_event_finish(&self) -> bool {
                    $requires_extended_event_finish
                }
            )?
            $(
                impl_widget!(@upcast $cap);
            )+
        }
    };
    (@snapshot_method $T:ty, $snapshot_source:ident) => {
        fn snapshot_fields(&self) -> $crate::ui::WidgetSnapshotFields {
            // 标记只负责选择直接端口；具体快照仍由组件自己的 SnapshotSource 拥有。
            let _ = stringify!($snapshot_source);
            <$T as $crate::ui::SnapshotSource>::snapshot_fields(self).into()
        }
    };
    (@snapshot_method $T:ty) => {
        fn snapshot_fields(&self) -> $crate::ui::WidgetSnapshotFields {
            // 普通手写组件未登记类型化快照，保持 Unknown 语义并跳过全表扫描。
            $crate::ui::WidgetSnapshotFields::UNKNOWN
        }
    };
    (@insert_cap $caps:ident Layout) => {
        $caps.insert($crate::ui::__private::traits::WidgetCapabilities::LAYOUT);
    };
    (@insert_cap $caps:ident Render) => {
        $caps.insert($crate::ui::__private::traits::WidgetCapabilities::RENDER);
    };
    (@insert_cap $caps:ident Event) => {
        $caps.insert($crate::ui::__private::traits::WidgetCapabilities::EVENT);
    };
    (@insert_cap $caps:ident Lifecycle) => {
        $caps.insert($crate::ui::__private::traits::WidgetCapabilities::LIFECYCLE);
    };
    (@insert_cap $caps:ident Animation) => {
        $caps.insert($crate::ui::__private::traits::WidgetCapabilities::ANIMATION);
    };
    (@insert_cap $caps:ident TextInput) => {
        $caps.insert($crate::ui::__private::traits::WidgetCapabilities::TEXT_INPUT);
    };
    (@upcast Layout) => {
        fn as_layout(&self) -> Option<&dyn $crate::ui::__private::traits::WidgetLayout> {
            Some(self)
        }
    };
    (@upcast Render) => {
        fn as_render(&self) -> Option<&dyn $crate::ui::__private::traits::WidgetRender> {
            Some(self)
        }
        fn as_render_mut(&mut self) -> Option<&mut dyn $crate::ui::__private::traits::WidgetRender> {
            Some(self)
        }
    };
    (@upcast Event) => {
        fn as_event(&self) -> Option<&dyn $crate::ui::__private::traits::EventHandler> {
            Some(self)
        }
        fn as_event_mut(&mut self) -> Option<&mut dyn $crate::ui::__private::traits::EventHandler> {
            Some(self)
        }
    };
    (@upcast Lifecycle) => {
        fn as_lifecycle(&self) -> Option<&dyn $crate::ui::__private::traits::WidgetLifecycle> {
            Some(self)
        }
        fn as_lifecycle_mut(&mut self) -> Option<&mut dyn $crate::ui::__private::traits::WidgetLifecycle> {
            Some(self)
        }
    };
    (@upcast Animation) => {
        fn as_animation(&self) -> Option<&dyn $crate::ui::__private::traits::WidgetAnimation> {
            Some(self)
        }
        fn as_animation_mut(&mut self) -> Option<&mut dyn $crate::ui::__private::traits::WidgetAnimation> {
            Some(self)
        }
    };
    (@upcast TextInput) => {
        fn as_text_input(&self) -> Option<&dyn $crate::ui::__private::traits::WidgetTextInput> {
            Some(self)
        }
        fn as_text_input_mut(&mut self) -> Option<&mut dyn $crate::ui::__private::traits::WidgetTextInput> {
            Some(self)
        }
    };
}

#[macro_export]
/// 将组件及其可选子列表构造成声明式组件树节点。
macro_rules! tree {
    // 父组件与子组件列表均采用 Rust 2024 表达式片段语义。
    ($parent:expr => [$($child:expr),+ $(,)?]) => {
        $crate::ui::__private::WidgetNode::new(
            Box::new($parent),
            vec![$($crate::ui::IntoWidgetNode::into_node($child)),+],
        )
    };
    // 单组件入口采用 Rust 2024 表达式片段语义。
    ($widget:expr) => {
        $crate::ui::__private::WidgetNode::leaf(Box::new($widget))
    };
}

/// 辅助：能力上转型生成。
#[macro_export]
#[doc(hidden)]
macro_rules! wc_upcast {
    ($T:ty; WidgetRender) => {
        fn as_render(&self) -> Option<&dyn $crate::ui::__private::traits::WidgetRender> {
            Some(self)
        }
        fn as_render_mut(
            &mut self,
        ) -> Option<&mut dyn $crate::ui::__private::traits::WidgetRender> {
            Some(self)
        }
    };
    ($T:ty; EventHandler) => {
        fn as_event(&self) -> Option<&dyn $crate::ui::__private::traits::EventHandler> {
            Some(self)
        }
        fn as_event_mut(&mut self) -> Option<&mut dyn $crate::ui::__private::traits::EventHandler> {
            Some(self)
        }
    };
    ($T:ty; WidgetLifecycle) => {
        fn as_lifecycle(&self) -> Option<&dyn $crate::ui::__private::traits::WidgetLifecycle> {
            Some(self)
        }
        fn as_lifecycle_mut(
            &mut self,
        ) -> Option<&mut dyn $crate::ui::__private::traits::WidgetLifecycle> {
            Some(self)
        }
    };
    ($T:ty; WidgetAnimation) => {
        fn as_animation(&self) -> Option<&dyn $crate::ui::__private::traits::WidgetAnimation> {
            Some(self)
        }
        fn as_animation_mut(
            &mut self,
        ) -> Option<&mut dyn $crate::ui::__private::traits::WidgetAnimation> {
            Some(self)
        }
    };
    ($T:ty; WidgetLayout) => {
        fn as_layout(&self) -> Option<&dyn $crate::ui::__private::traits::WidgetLayout> {
            Some(self)
        }
    };
    ($T:ty; WidgetTextInput) => {
        fn as_text_input(&self) -> Option<&dyn $crate::ui::__private::traits::WidgetTextInput> {
            Some(self)
        }
        fn as_text_input_mut(&mut self) -> Option<&mut dyn $crate::ui::__private::traits::WidgetTextInput> {
            Some(self)
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __widget_text_input_upcast_method {
    (text_input_cursor_rect; $T:ty) => {
        $crate::wc_upcast!($T; WidgetTextInput);
    };
    ($other:ident; $T:ty) => {};
}

// ── 辅助宏：build 方法和 upcast ──

/// 组件声明 `build_view_children` 时，生成 ViewChildrenProvider 实现
/// （SMC-04：声明期 View 子节点端口归 System 私有边界）。
#[macro_export]
#[doc(hidden)]
macro_rules! __widget_view_children_impl {
    ($T:ty; build_view_children; ($($p:tt)*) -> $ret:ty $body:block) => {
        impl $crate::ui::__private::traits::ViewChildrenProvider for $T {
            fn build_view_children($($p)*) -> $ret $body
        }
    };
    ($T:ty; $other:ident; $($rest:tt)*) => {};
}

/// 如果方法是 `build`，生成 `fn build(params) -> Ret { body }`。
#[macro_export]
#[doc(hidden)]
macro_rules! __widget_build_method {
    (dynamic_children; ($coordinator:expr) $body:block) => {
        fn dynamic_children_coordinator(&self) -> Option<&'static dyn $crate::ui::DynamicChildrenCoordinator> { Some($coordinator) }
    };
    (text_selection; ($($ignored:tt)*) $body:block) => {
        fn as_text_selection(&self) -> Option<&dyn $crate::ui::WidgetTextSelection> { Some(self) }
        fn as_text_selection_mut(&mut self) -> Option<&mut dyn $crate::ui::WidgetTextSelection> { Some(self) }
    };
    (viewport_overflow_axes; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn viewport_overflow_axes($($p)*) -> $ret $body
    };
    (layout_size_locks; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn layout_size_locks($($p)*) -> $ret $body
    };
    (overlay_is_present; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn overlay_is_present($($p)*) -> $ret $body
    };
    (overlay_destroy_on_close; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn overlay_destroy_on_close($($p)*) -> $ret $body
    };
    (consume_context_close; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn consume_context_close($($p)*) -> $ret $body
    };
    (dismiss_overlay; ($($p:tt)*) $body:block) => {
        fn dismiss_overlay($($p)*) $body
    };
    (pointer_focus_descendant; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn pointer_focus_descendant($($p)*) -> $ret $body
    };
    (semantic_text_value; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn semantic_text_value($($p)*) -> $ret $body
    };
    (window_drag_region; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn window_drag_region($($p)*) -> $ret $body
    };
    (invalidate_action_siblings; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn invalidate_action_siblings($($p)*) -> $ret $body
    };
    (preferred_focus_child; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn preferred_focus_child($($p)*) -> $ret $body
    };
    (declaration_style; ($($p:tt)*) $body:block) => {
        fn apply_declaration_style($($p)*) $body
    };
    (reconcile_config; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn declaration_config_changed($($p)*) -> $ret $body
    };
    (reconcile_layout; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn declaration_layout_changed($($p)*) -> $ret $body
    };
    (reconcile_runtime; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn declaration_runtime_changed($($p)*) -> $ret $body
    };
    (reconcile_sync; ($sync:ident) $body:block) => {
        $crate::__widget_reconcile_method!($sync);
    };
    (__semantic_actions_decl; ($($actions:tt)*) $body:block) => {
        // E-05：`semantic_actions => [...]` 槽位生成的声明方法；$actions 为
        // `&[...]` 表达式（由前置分支包装），直接作为返回值。
        fn declared_semantic_actions(&self) -> &'static [$crate::ui::SemanticAction] {
            $($actions)*
        }
    };
    (build; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn build($($p)*) -> $ret $body
    };
    // 核心子节点通知不属于可选能力 trait，直接生成 Widget 方法。
    (on_children_changed; ($($p:tt)*) $body:block) => {
        // 保留声明中的参数与方法体，让组件自行同步派生运行态。
        fn on_children_changed($($p)*) $body
    };
    (on_child_visibility_changed; ($($p:tt)*) $body:block) => {
        // 可见子树变化会使自然尺寸缓存失效，由组件清除自身派生状态。
        fn on_child_visibility_changed($($p)*) $body
    };
    (build_view_children; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn as_view_children(
            &self,
        ) -> Option<&dyn $crate::ui::__private::traits::ViewChildrenProvider> {
            Some(self)
        }
    };
    (visible; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn visible($($p)*) -> $ret $body
    };
    (tab_index; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn tab_index($($p)*) -> $ret $body
    };
    // 真实终端类组件声明消费原始 Tab（不走树层焦点导航），直接生成 Widget 方法。
    (consumes_tab_key; ($($p:tt)*) -> $ret:ty $body:block) => {
        fn consumes_tab_key($($p)*) -> $ret $body
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
macro_rules! __widget_upcast_method {
    (measure; $T:ty) => { $crate::wc_upcast!($T; WidgetLayout); };
    (flex_grow; $T:ty) => {};
    (flex_shrink; $T:ty) => {};
    (align_self; $T:ty) => {};
    (grid_cell; $T:ty) => {};
    (grid_column_span; $T:ty) => {};
    (grid_row_span; $T:ty) => {};
    (layout_margin; $T:ty) => {};
    (child_overflow_expands_parent; $T:ty) => {};
    (child_visible; $T:ty) => {};
    (measure_children; $T:ty) => {};
    (layout_children; $T:ty) => {};
    (render; $T:ty) => { $crate::wc_upcast!($T; WidgetRender); };
    (uses_palette; $T:ty) => {};
    (dirty_rect; $T:ty) => {};
    (children_clip; $T:ty) => {};
    (paint_after_children; $T:ty) => {};
    (overlay_entry; $T:ty) => {};
    (draw_margin; $T:ty) => {};
    (on_event; $T:ty) => { $crate::wc_upcast!($T; EventHandler); };
    (take_layout_request; $T:ty) => {};
    (scroll_delta; $T:ty) => {};
    (scroll_delta_for_dirty; $T:ty) => {};
    (scroll_composite_viewport; $T:ty) => {};
    (viewport_scroll_offset; $T:ty) => {};
    (scroll_descendant_by; $T:ty) => {};
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

mod helpers;
mod widget;

/// Generate an opt-in, type-checked reconciliation port for a component.
#[doc(hidden)]
#[macro_export]
macro_rules! __widget_reconcile_method {
    ($sync:ident) => {
        fn reconcile_from(
            &mut self,
            next: Box<dyn $crate::ui::Widget>,
        ) -> Result<bool, Box<dyn $crate::ui::Widget>> {
            if !next.as_any().is::<Self>() {
                return Err(next);
            }
            let next = next.into_any().downcast::<Self>()
                .expect("widget type checked before consuming declaration");
            self.$sync(*next);
            Ok(true)
        }
    };
}
