// 引入过程宏标识符、数值字面量、跨度与令牌流。
use proc_macro2::{Ident, Literal, Span, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入动画 AST 与诊断。
use super::{
    AnimationBinding, AnimationDirection, AnimationEasing, AnimationFillMode,
    AnimationPropertyBinding, AnimationPropertyKind, Diagnostic,
};
// 引入动画值专用解析入口。
use super::style_value_codegen::{animation_color_value, animation_f32_value};

// 把静态 animation 绑定应用到最终 ViewNode。
pub(super) fn apply_animation(
    // 接收已经应用普通样式的实际 ViewNode。
    view: TokenStream,
    // 接收展开期完成验证的动画元数据。
    binding: &AnimationBinding,
) -> Result<TokenStream, Diagnostic> {
    // 重建最近组件或文档根作用域标识符。
    let widget_scope = Ident::new(&binding.widget_scope_name, Span::call_site());
    // 创建节点私有动画作用域名称。
    let node_scope = Ident::new("__uix_animation_scope", Span::mixed_site());
    // 创建节点稳定身份名称。
    let identity = Ident::new("__uix_animation_identity", Span::mixed_site());
    // 创建逐字段应用时复用的 View 名称。
    let animated_view = Ident::new("__uix_animation_view", Span::mixed_site());
    // 嵌套 For 使用实际实例路径，静态节点使用固定身份段。
    let identity_value = binding
        // 借用可选 For 路径名称。
        .instance_path_name
        // 存在时恢复局部标识符。
        .as_ref()
        // 生成路径克隆。
        .map(|name| {
            // 使用调用点卫生解析循环体局部变量。
            let path = Ident::new(name, Span::call_site());
            // 克隆稳定路径字符串。
            quote! { (#path).clone() }
        })
        // 静态节点使用固定身份段。
        .unwrap_or_else(|| quote! { ::std::string::String::from("static") });
    // 生成完整播放配置令牌。
    let playback = playback_tokens(binding);
    // 生成统一片段缓动令牌。
    let easing = easing_tokens(binding.playback.easing);
    // 保存逐字段状态建立与 View 应用语句。
    let mut property_steps = Vec::with_capacity(binding.properties.len());
    // 按稳定字段顺序生成独立 typed Animated。
    for (field_id, property) in binding.properties.iter().enumerate() {
        // 创建字段 State 局部变量。
        let state = Ident::new(
            // 使用字段序号形成确定名称。
            &format!("__uix_animation_state_{field_id}"),
            // 使用混合卫生隔离调用方名称。
            Span::mixed_site(),
        );
        // 创建字段 Animated 句柄局部变量。
        let source = Ident::new(
            // 使用字段序号形成确定名称。
            &format!("__uix_animation_source_{field_id}"),
            // 使用混合卫生隔离调用方名称。
            Span::mixed_site(),
        );
        // 生成基础值令牌。
        let baseline = property_value(property, &property.baseline)?;
        // 保存字段关键帧令牌。
        let mut frames = Vec::with_capacity(property.frames.len());
        // 按解析期已排序偏移生成每一帧。
        for frame in &property.frames {
            // 把百万分比转换为确定性 f64 字面量。
            let offset = Literal::f64_unsuffixed(frame.offset_millionths as f64 / 1_000_000.0);
            // 生成当前字段值。
            let value = property_value(property, &frame.property)?;
            // 生成带统一 timing-function 的 typed 关键帧。
            frames.push(quote! {
                // 当前简写的 timing-function 应用于每个相邻片段。
                ::uix_app::prelude::Keyframe::new(#offset, #value).easing(#easing)
            });
        }
        // 选择当前字段对应的 View Animated 绑定方法。
        let method = Ident::new(property_method(property.kind), Span::call_site());
        // 字段编号在单个节点子作用域内稳定且互不冲突。
        let field_id = field_id as u64;
        // 生成状态复用、首次播放初始化与 View 值绑定。
        property_steps.push(quote! {
            // 在窗口私有组件状态存储中复用当前字段的 Animated 句柄。
            let #state = ::uix_app::ui::__private::uix_widget_state(
                // 使用实际节点动画子作用域。
                &#node_scope,
                // 使用字段稳定编号。
                #field_id,
                // 初始化闭包只在节点首次挂载时执行。
                || {
                    // 使用最终基础样式值作为 fill-mode 恢复值。
                    let #source = ::uix_app::prelude::Animated::new(#baseline);
                    // 编译器已验证非空有限帧，运行时错误代表内部契约破坏。
                    if let ::std::result::Result::Err(__uix_animation_error) =
                        #source.animate_keyframes_with([#(#frames),*], #playback)
                    {
                        // 立即暴露不可能的生成器与运行时契约错配。
                        panic!("UIX animation 关键帧初始化失败: {}", __uix_animation_error);
                    }
                    // 把共享句柄保存到组件私有 State。
                    #source
                },
            );
            // 读取持久化句柄并登记结构性状态依赖。
            let #source = #state.get();
            // `.value()` 在绑定方法内把同一 source 交给窗口帧调度器。
            let #animated_view = #animated_view.#method(&#source);
        });
    }
    // 取出稳定节点声明标识。
    let declaration_id = binding.declaration_id;
    // 返回节点作用域、字段初始化与最终生命周期标记的单一表达式。
    Ok(quote! {{
        // 组合当前节点在 For 或静态树中的稳定身份。
        let #identity = #identity_value;
        // 从最近组件作用域派生实际节点动画状态作用域。
        let #node_scope = ::uix_app::ui::__private::uix_widget_child_scope(
            // 使用最近组件或文档根作用域。
            &#widget_scope,
            // 使用静态节点声明标识。
            #declaration_id,
            // 使用实际实例身份。
            #identity,
        );
        // 只求值一次较低层 View。
        let #animated_view = #view;
        // 按字段顺序绑定所有持久化动画值。
        #(#property_steps)*
        // 让实际节点承载动画私有状态生命周期。
        #animated_view.uix_widget_scope(#node_scope, 0)
    }})
}

// 生成一个动画字段的具体 Rust 值。
fn property_value(
    // 接收字段类型。
    binding: &AnimationPropertyBinding,
    // 接收基础或关键帧样式属性。
    property: &super::StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 数值与颜色使用各自专用解析器。
    match binding.kind {
        // 四类 f32 字段复用范围规则。
        AnimationPropertyKind::Width
        | AnimationPropertyKind::Height
        | AnimationPropertyKind::BorderRadius
        | AnimationPropertyKind::Opacity => animation_f32_value(binding.kind, property),
        // 两类颜色字段生成具体 Color。
        AnimationPropertyKind::Color | AnimationPropertyKind::BackgroundColor => {
            animation_color_value(property)
        }
    }
}

// 返回闭合动画字段对应的 View 绑定方法名。
const fn property_method(kind: AnimationPropertyKind) -> &'static str {
    // 按字段类型选择公开 ViewNode 方法。
    match kind {
        // 宽度绑定。
        AnimationPropertyKind::Width => "width_animated",
        // 高度绑定。
        AnimationPropertyKind::Height => "height_animated",
        // 圆角绑定。
        AnimationPropertyKind::BorderRadius => "radius_animated",
        // 透明度绑定。
        AnimationPropertyKind::Opacity => "opacity_animated",
        // 前景颜色绑定。
        AnimationPropertyKind::Color => "color_animated",
        // 背景颜色绑定。
        AnimationPropertyKind::BackgroundColor => "background_color_animated",
    }
}

// 生成完整 KeyframePlayback 配置。
fn playback_tokens(binding: &AnimationBinding) -> TokenStream {
    // 把微秒整数转换为秒级 f64 字面量。
    let duration = Literal::f64_unsuffixed(binding.playback.duration_micros as f64 / 1_000_000.0);
    // 把延迟微秒整数转换为秒级 f64 字面量。
    let delay = Literal::f64_unsuffixed(binding.playback.delay_micros as f64 / 1_000_000.0);
    // 生成有限或无限轮次。
    let iterations = match binding.playback.iterations {
        // 有限轮次生成 Some。
        Some(count) => quote! { ::std::option::Option::Some(#count) },
        // 无限轮次生成 None。
        None => quote! { ::std::option::Option::None },
    };
    // 生成方向枚举。
    let direction = direction_tokens(binding.playback.direction);
    // 生成填充枚举。
    let fill_mode = fill_mode_tokens(binding.playback.fill_mode);
    // 返回公开播放配置字面量。
    quote! {
        ::uix_app::prelude::KeyframePlayback {
            // 保存单轮时长。
            duration: #duration,
            // 保存启动延迟。
            delay: #delay,
            // 保存有限或无限轮次。
            iterations: #iterations,
            // 保存播放方向。
            direction: #direction,
            // 保存填充模式。
            fill_mode: #fill_mode,
        }
    }
}

// 生成运行时缓动枚举令牌。
pub(super) fn easing_tokens(value: AnimationEasing) -> TokenStream {
    // 按闭合集合映射现有 Easing API。
    match value {
        // 线性插值。
        AnimationEasing::Linear => quote! { ::uix_app::prelude::Easing::linear },
        // CSS 默认 ease 使用标准三次贝塞尔。
        AnimationEasing::Ease => quote! { ::uix_app::prelude::Easing::css_ease() },
        // 缓入使用标准三次贝塞尔。
        AnimationEasing::EaseIn => quote! { ::uix_app::prelude::Easing::css_ease_in() },
        // 缓出使用标准三次贝塞尔。
        AnimationEasing::EaseOut => quote! { ::uix_app::prelude::Easing::css_ease_out() },
        // 缓入缓出使用标准三次贝塞尔。
        AnimationEasing::EaseInOut => quote! { ::uix_app::prelude::Easing::css_ease_in_out() },
    }
}

// 生成运行时方向枚举令牌。
fn direction_tokens(value: AnimationDirection) -> TokenStream {
    // 按闭合集合映射公开方向枚举。
    match value {
        // 正向播放。
        AnimationDirection::Normal => quote! { ::uix_app::prelude::KeyframeDirection::Normal },
        // 倒向播放。
        AnimationDirection::Reverse => quote! { ::uix_app::prelude::KeyframeDirection::Reverse },
        // 正向交替。
        AnimationDirection::Alternate => {
            quote! { ::uix_app::prelude::KeyframeDirection::Alternate }
        }
        // 倒向交替。
        AnimationDirection::AlternateReverse => {
            quote! { ::uix_app::prelude::KeyframeDirection::AlternateReverse }
        }
    }
}

// 生成运行时填充枚举令牌。
fn fill_mode_tokens(value: AnimationFillMode) -> TokenStream {
    // 按闭合集合映射公开填充枚举。
    match value {
        // 无填充。
        AnimationFillMode::None => quote! { ::uix_app::prelude::KeyframeFillMode::None },
        // 完成后填充。
        AnimationFillMode::Forwards => quote! { ::uix_app::prelude::KeyframeFillMode::Forwards },
        // 延迟期填充。
        AnimationFillMode::Backwards => quote! { ::uix_app::prelude::KeyframeFillMode::Backwards },
        // 双向填充。
        AnimationFillMode::Both => quote! { ::uix_app::prelude::KeyframeFillMode::Both },
    }
}
