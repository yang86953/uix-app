#[macro_export]
macro_rules! semantic_handler {
    (
        $kind:expr_2021,
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
/// ```ignore
/// column(views![
///     label("Hello"),
///     button("+1").primary().on_click(&count, |c| c.set(c.get() + 1)),
/// ])
/// ```
///
/// 两三个子节点时优先元组：`column((a, b))`。
#[macro_export]
macro_rules! views {
    ($($child:expr_2021),* $(,)?) => {{
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
/// ```ignore
/// keyframe! {
///     #[derive(Debug, PartialEq)]
///     pub struct EnterFrame {
///         pub opacity: f32,
///         pub offset_y: f32,
///     }
/// }
/// ```
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
/// ```ignore
/// .root(with_cloned!(active, count, theme; {
///     shell(active, &count, &theme)
/// }))
/// .on_start(with_cloned!(theme, ticks; |handle| {
///     theme.set_handle(handle.clone());
///     // …
/// }))
/// ```
#[macro_export]
macro_rules! with_cloned {
    ($($name:ident),+ $(,)? ; |$arg:ident| $body:expr_2021) => {{
        $(let $name = $name.clone();)+
        move |$arg| {
            $(let $name = $name.clone();)+
            $body
        }
    }};
    ($($name:ident),+ $(,)? ; $body:expr_2021) => {{
        $(let $name = $name.clone();)+
        move || {
            $(let $name = $name.clone();)+
            $body
        }
    }};
}
