//! Macros — 组件式 widget 定义。仅支持新语法。

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
        fn as_render(&self) -> Option<&dyn $crate::api::traits::WidgetRender> { Some(self) }
        fn as_render_mut(&mut self) -> Option<&mut dyn $crate::api::traits::WidgetRender> { Some(self) }
    };
    ($T:ty; WidgetEventHandler) => {
        fn as_event(&self) -> Option<&dyn $crate::api::traits::WidgetEventHandler> { Some(self) }
        fn as_event_mut(&mut self) -> Option<&mut dyn $crate::api::traits::WidgetEventHandler> { Some(self) }
    };
    ($T:ty; WidgetLifecycle) => {
        fn as_lifecycle(&self) -> Option<&dyn $crate::api::traits::WidgetLifecycle> { Some(self) }
        fn as_lifecycle_mut(&mut self) -> Option<&mut dyn $crate::api::traits::WidgetLifecycle> { Some(self) }
    };
    ($T:ty; WidgetLayout) => {
        fn as_layout(&self) -> Option<&dyn $crate::api::traits::WidgetLayout> { Some(self) }
    };
}

/// 定义 widget 组件。
///
/// ```ignore
/// define_widget! {
///     pub Button { text: String }
///     @new -> Self { Self { text: "".into() } }
///     impl WidgetRender {
///         render => (&self, frame, ctx, tree) { ... }
///     }
/// }
/// ```
#[macro_export]
macro_rules! define_widget {
    // 带 struct 关键字 → 转发
    (
        $(#[$m:meta])* $vis:vis struct $name:ident { $($field:tt)* }
        $($rest:tt)*
    ) => {
        $crate::define_widget! {
            $(#[$m])* $vis $name { $($field)* }
            $($rest)*
        }
    };

    // 主模式：显式 impl 块
    (
        $(#[$m:meta])*
        $vis:vis $name:ident { $($field:tt)* }
        $(@ new -> Self $new_body:block)?
        $( impl $trait:ident {
            $( $method:ident => ( $($params:tt)* ) $(-> $ret:ty)? $body:block )*
        } )*
    ) => {
        $(#[$m])* $vis struct $name { $($field)* }
        $( impl $name { pub fn new() -> Self $new_body } )?

        impl $crate::api::traits::WidgetComponent for $name {
            fn as_any(&self) -> &dyn std::any::Any { self }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
            fn capabilities(&self) -> $crate::api::traits::WidgetCapabilities {
                let mut c = $crate::api::traits::WidgetCapabilities::new();
                $( match stringify!($trait) {
                    "WidgetLayout" => c.insert($crate::api::traits::WidgetCapabilities::LAYOUT),
                    "WidgetRender" => c.insert($crate::api::traits::WidgetCapabilities::RENDER),
                    "WidgetEventHandler" => c.insert($crate::api::traits::WidgetCapabilities::EVENT),
                    "WidgetLifecycle" => c.insert($crate::api::traits::WidgetCapabilities::LIFECYCLE),
                    _ => {}
                } )*
                c
            }
            $( $crate::wc_upcast!($name; $trait); )*
        }
        $( impl $crate::api::traits::$trait for $name {
            $( fn $method( $($params)* ) $(-> $ret)? $body )*
        } )*
    };
}
