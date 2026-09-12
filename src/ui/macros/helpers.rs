#[macro_export]
/// 创建捕获响应式状态、计算值或窗口句柄的语义事件处理器。
macro_rules! semantic_handler {
    (
        // 语义类型参数采用 Rust 2024 表达式片段语义。
        $kind:expr,
        $(state [$($state:ident),* $(,)?],)?
        $(computed [$($computed:ident),* $(,)?],)?
        $(window [$($window:ident),* $(,)?],)?
        |$event:ident| $body:block
    ) => {{
        $(let __uix_state_captures = ($($state.clone(),)*);)?
        $(let __uix_computed_captures = ($($computed.clone(),)*);)?
        $(let __uix_window_captures = ($($window.clone(),)*);)?
        let __uix_registration = $crate::ui::HandlerRegistration::new(
            $kind,
            Box::new(move |$event| {
                $(#[allow(unused_variables)] let ($($state,)*) = __uix_state_captures.clone();)?
                $(#[allow(unused_variables)] let ($($computed,)*) = __uix_computed_captures.clone();)?
                $(#[allow(unused_variables)] let ($($window,)*) = __uix_window_captures.clone();)?
                $body
            }),
        );
        #[allow(unused_mut)]
        let mut __uix_registration = __uix_registration;
        $($(
            __uix_registration = __uix_registration.with_state_capture(&$state);
        )*)?
        $($(
            __uix_registration = __uix_registration.with_computed_capture(&$computed);
        )*)?
        $($(
            __uix_registration = __uix_registration.with_window_capture($window);
        )*)?
        __uix_registration
    }};
}

/// 将异质 View 子节点收成 `Vec<ViewNode>`，供 `column` / `row` / `grid` 使用（[#178]）。
///
///
/// 两三个子节点时优先元组：`column((a, b))`。
#[macro_export]
macro_rules! views {
    // 子视图列表采用 Rust 2024 表达式片段语义。
    ($($child:expr),* $(,)?) => {{
        {
            use $crate::ui::View;
            vec![$(View::build($child),)*]
        }
    }};
}

/// 定义一组共享动画时钟的 typed keyframe 属性。
///
/// 宏生成一个普通结构体、同可见性的 `new(...)` 构造器，以及逐字段
/// [`Animatable`](crate::ui::Animatable) 实现。将生成类型作为
/// `Animated<T>` / `KeyframeAnimation<T>` 的值，即可让全部属性由一个
/// source 和一条时间线推进。
///
#[macro_export]
macro_rules! keyframe {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $($field_vis:vis $field:ident : $field_ty:ty),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy)]
        $vis struct $name {
            $($field_vis $field: $field_ty),+
        }

        impl $name {
            $vis const fn new($($field: $field_ty),+) -> Self {
                Self { $($field),+ }
            }
        }

        impl $crate::ui::Animatable for $name {
            fn lerp(from: Self, to: Self, t: f64) -> Self {
                Self {
                    $($field: <$field_ty as $crate::ui::Animatable>::lerp(
                        from.$field,
                        to.$field,
                        t,
                    )),+
                }
            }

            fn delta(from: Self, to: Self) -> f64 {
                let mut delta = 0.0_f64;
                $(
                    delta = delta.max(
                        <$field_ty as $crate::ui::Animatable>::delta(
                            from.$field,
                            to.$field,
                        )
                        .abs(),
                    );
                )+
                delta
            }
        }
    };
}

/// 一次捕获多个 `Clone` 值，生成 `'static` 闭包（`App::root` / `on_start` 等）。
///
/// 外层 clone 进闭包；每次调用再 clone 供 body 使用（root 重建需要）。
/// 不消除 Rust `'static` 固有 clone，只去掉手写 `*_for_root` 样板（[#180]）。
///
#[macro_export]
macro_rules! with_cloned {
    // 带参数闭包体采用 Rust 2024 表达式片段语义。
    ($($name:ident),+ $(,)? ; |$arg:ident| $body:expr) => {{
        $(let $name = $name.clone();)+
        move |$arg| {
            $(let $name = $name.clone();)+
            $body
        }
    }};
    // 无参数闭包体采用 Rust 2024 表达式片段语义。
    ($($name:ident),+ $(,)? ; $body:expr) => {{
        $(let $name = $name.clone();)+
        move || {
            $(let $name = $name.clone();)+
            $body
        }
    }};
}
