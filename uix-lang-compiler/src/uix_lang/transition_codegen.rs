// 引入过程宏标识符、数值字面量、跨度与令牌流。
use proc_macro2::{Ident, Literal, Span, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入共享缓动映射。
use super::animation_codegen::easing_tokens;
// 引入 transition AST 与闭合字段枚举。
use super::{AnimationPropertyKind, TransitionBinding};

// 把 transition 绑定应用到全部状态样式完成后的最终 ViewNode。
pub(super) fn apply_transition(
    // 接收已经完成动态与伪类样式叠加的目标 View。
    view: TokenStream,
    // 借用展开期验证完成的 transition 元数据。
    binding: &TransitionBinding,
) -> TokenStream {
    // 重建最近组件或文档根作用域标识符。
    let widget_scope = Ident::new(&binding.widget_scope_name, Span::call_site());
    // 创建最终目标 View 局部变量。
    let target_view = Ident::new("__uix_transition_target_view", Span::mixed_site());
    // 创建实例路径局部变量。
    let instance_path = Ident::new("__uix_transition_instance_path", Span::mixed_site());
    // 创建包含最终 key 的节点身份局部变量。
    let identity = Ident::new("__uix_transition_identity", Span::mixed_site());
    // 创建 transition 私有子作用域局部变量。
    let node_scope = Ident::new("__uix_transition_scope", Span::mixed_site());
    // 创建持久化 State 局部变量。
    let state = Ident::new("__uix_transition_state", Span::mixed_site());
    // 创建持久化 transition 句柄局部变量。
    let transition = Ident::new("__uix_transition_runtime", Span::mixed_site());
    // 嵌套 For 使用实际路径，静态节点使用固定段。
    let instance_path_value = binding
        // 借用可选 For 路径名称。
        .instance_path_name
        // 恢复局部标识符。
        .as_ref()
        // 生成路径克隆。
        .map(|name| {
            // 使用调用点卫生解析循环体局部变量。
            let path = Ident::new(name, Span::call_site());
            // 返回稳定实际路径。
            quote! { (#path).clone() }
        })
        // 静态节点使用固定身份段。
        .unwrap_or_else(|| quote! { ::std::string::String::from("static") });
    // 生成闭合运行时字段枚举。
    let properties = binding
        // 遍历确定顺序字段。
        .properties
        // 借用迭代器。
        .iter()
        // 映射运行时枚举路径。
        .map(|property| transition_property_tokens(*property))
        // 收集供数组展开。
        .collect::<Vec<_>>();
    // 把微秒时长转换为秒级 f64 字面量。
    let duration = Literal::f64_unsuffixed(binding.duration_micros as f64 / 1_000_000.0);
    // 把微秒延迟转换为秒级 f64 字面量。
    let delay = Literal::f64_unsuffixed(binding.delay_micros as f64 / 1_000_000.0);
    // 生成闭合缓动曲线。
    let easing = easing_tokens(binding.easing);
    // 取出稳定节点声明标识。
    let declaration_id = binding.declaration_id;
    // 返回目标比较、状态复用与当前帧值应用的单一表达式。
    quote! {{
        // 先求值全部基础、动态与伪类样式，得到本轮最终目标。
        let #target_view = #view;
        // 只求值一次静态位置或实际 For 路径。
        let #instance_path = #instance_path_value;
        // 组合最终 View key，确保换 key 不复用旧动画状态。
        let #identity = ::uix_app::ui::__private::uix_transition_identity(
            // 借用已经应用 key 的最终目标 View。
            &#target_view,
            // 借用实例路径文本。
            &#instance_path,
        );
        // 从最近组件作用域派生实际节点 transition 子作用域。
        let #node_scope = ::uix_app::ui::__private::uix_widget_child_scope(
            // 借用最近组件或文档根作用域。
            &#widget_scope,
            // 使用稳定声明编号。
            #declaration_id,
            // 使用包含实际 key 的身份。
            #identity,
        );
        // 在窗口私有组件状态存储中复用 transition 运行时。
        let #state = ::uix_app::ui::__private::uix_widget_state(
            // 借用实际节点子作用域。
            &#node_scope,
            // 子作用域内 transition 只占用固定字段零。
            0,
            // 初始化闭包只在首次挂载时执行。
            || {
                // 从首次最终目标静止创建全部 typed Animated 源。
                match ::uix_app::ui::__private::uix_transition_state(
                    // 借用首次最终目标。
                    &#target_view,
                    // 传入编译期闭合字段集合。
                    &[#(#properties),*],
                ) {
                    // 合法编译输出返回持久化状态。
                    ::std::result::Result::Ok(value) => value,
                    // 契约错配必须立即暴露而非静默跳过字段。
                    ::std::result::Result::Err(error) => {
                        // 报告内部生成器与运行时边界错配。
                        panic!("UIX transition 初始化失败: {}", error)
                    }
                }
            },
        );
        // 读取持久化句柄并登记组件结构性状态依赖。
        let #transition = #state.get();
        // 比较本轮最终目标并原位重定向变化字段。
        let #target_view = match ::uix_app::ui::__private::uix_apply_transition(
            // 消费本轮目标 View。
            #target_view,
            // 借用持久化 transition 状态。
            &#transition,
            // 传入已验证播放配置。
            ::uix_app::ui::__private::UixTransitionSpec {
                // 保存单次时长。
                duration: #duration,
                // 保存启动延迟。
                delay: #delay,
                // 保存缓动曲线。
                easing: #easing,
            },
        ) {
            // 合法目标返回写入当前帧值的 View。
            ::std::result::Result::Ok(value) => value,
            // 契约错配必须立即暴露而非展示错误目标。
            ::std::result::Result::Err(error) => {
                // 报告内部生成器与运行时边界错配。
                panic!("UIX transition 目标应用失败: {}", error)
            }
        };
        // 让实际节点承载 transition 私有状态生命周期。
        #target_view.uix_widget_scope(#node_scope, 0)
    }}
}

// 映射闭合字段到隐藏运行时枚举。
fn transition_property_tokens(property: AnimationPropertyKind) -> TokenStream {
    // 按字段返回公开宏桥接路径。
    match property {
        // 显式宽度。
        AnimationPropertyKind::Width => {
            quote! { ::uix_app::ui::__private::UixTransitionProperty::Width }
        }
        // 显式高度。
        AnimationPropertyKind::Height => {
            quote! { ::uix_app::ui::__private::UixTransitionProperty::Height }
        }
        // 统一圆角。
        AnimationPropertyKind::BorderRadius => {
            quote! { ::uix_app::ui::__private::UixTransitionProperty::BorderRadius }
        }
        // 节点透明度。
        AnimationPropertyKind::Opacity => {
            quote! { ::uix_app::ui::__private::UixTransitionProperty::Opacity }
        }
        // 前景颜色。
        AnimationPropertyKind::Color => {
            quote! { ::uix_app::ui::__private::UixTransitionProperty::Color }
        }
        // 普通背景颜色。
        AnimationPropertyKind::BackgroundColor => {
            quote! { ::uix_app::ui::__private::UixTransitionProperty::BackgroundColor }
        }
    }
}
