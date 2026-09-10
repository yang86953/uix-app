// 引入过程宏标识符、跨度与令牌流。
use proc_macro2::{Ident, Span, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入表达式生成、诊断与伪类绑定 AST。
use super::{Diagnostic, PseudoStyleBinding, PseudoStyleCondition, generate_expression};
// 引入共享 Style 差异字段应用入口。
use super::style_codegen::apply_style_properties;

// 把自动状态事实与差异字段叠加到既有 View。
pub(super) fn apply_pseudo_style(
    // 接收已经应用基础 class、内联样式和自定义动态样式的 View。
    view: TokenStream,
    // 接收展开期完成验证的伪类元数据。
    binding: &PseudoStyleBinding,
) -> Result<TokenStream, Diagnostic> {
    // 创建逐层叠加时复用的卫生 View 名称。
    let styled = Ident::new("__uix_pseudo_style_view", Span::mixed_site());
    // 保存有序包装表达式。
    let mut output = view;
    // hover 最先叠加，checked 与 disabled 可在其上覆盖。
    if !binding.hover.is_empty() {
        // hover 必须已经取得组件作用域名称。
        let widget_scope = Ident::new(
            binding
                .hover_scope_name
                .as_deref()
                .expect("hover 伪类必须拥有组件作用域"),
            Span::call_site(),
        );
        // 创建节点私有作用域名称。
        let node_scope = Ident::new("__uix_pseudo_style_scope", Span::mixed_site());
        // 创建 hover State 名称。
        let state = Ident::new("__uix_pseudo_hover_state", Span::mixed_site());
        // 创建事件闭包独占 State 克隆。
        let event_state = Ident::new("__uix_pseudo_hover_event_state", Span::mixed_site());
        // 创建当前 hover 值名称。
        let current = Ident::new("__uix_pseudo_hover_current", Span::mixed_site());
        // 创建 For 实例身份名称。
        let identity = Ident::new("__uix_pseudo_style_identity", Span::mixed_site());
        // 生成 hover 差异字段应用。
        let hovered = apply_style_properties(quote! { #styled }, &binding.hover)?;
        // 嵌套 For 使用实际实例路径，静态节点使用固定身份段。
        let identity_value = binding
            .instance_path_name
            .as_ref()
            .map(|name| {
                // 恢复循环路径局部变量。
                let path = Ident::new(name, Span::call_site());
                // 克隆路径字符串作为稳定身份。
                quote! { (#path).clone() }
            })
            .unwrap_or_else(|| quote! { ::std::string::String::from("static") });
        // 取出稳定声明标识。
        let declaration_id = binding.declaration_id;
        // 生成 hover 状态、监听器与叠加分支。
        output = quote! {{
            // 为当前实际节点计算稳定身份。
            let #identity = #identity_value;
            // 从最近组件作用域派生 hover 私有子作用域。
            let #node_scope = ::uix_app::ui::__private::uix_widget_child_scope(
                &#widget_scope,
                #declaration_id,
                #identity,
            );
            // 复用窗口私有组件状态存储保存 hover 事实。
            let #state = ::uix_app::ui::__private::uix_widget_state(
                &#node_scope,
                0,
                || false,
            );
            // 读取 hover 事实并登记结构性 reconcile 依赖。
            let #current = #state.get();
            // 克隆 State 句柄供指针监听器移动。
            let #event_state = #state.clone();
            // 自动监听进入与离开且不截断组件自身事件。
            let #styled = (#output).on_pointer(move |__uix_pseudo_pointer_event| {
                // 指针进入时选择 hover 差异层。
                if matches!(
                    __uix_pseudo_pointer_event,
                    &::uix_app::prelude::SystemEvent::PointerEnter
                ) {
                    // 写入既有组件私有状态并请求声明式重建。
                    #event_state.set(true);
                // 指针离开时恢复基础样式。
                } else if matches!(
                    __uix_pseudo_pointer_event,
                    &::uix_app::prelude::SystemEvent::PointerLeave
                ) {
                    // 清除 hover 事实。
                    #event_state.set(false);
                }
                // 继续交付组件自身处理器并向父节点冒泡。
                ::uix_app::prelude::EventResult::NotHandled
            });
            // 状态变体只覆盖自己声明的字段。
            (if #current { #hovered } else { #styled })
                // 把 hover 私有状态生命周期绑定到实际节点。
                .uix_widget_scope(#node_scope, 0)
        }};
    }
    // checked 在 hover 之上叠加。
    if let Some((condition, properties)) = &binding.checked {
        // 生成现有 checked 事实读取。
        let condition = condition_tokens(condition)?;
        // 生成 checked 差异字段应用。
        let active = apply_style_properties(quote! { #styled }, properties)?;
        // 只在 checked 为真时覆盖声明字段。
        output = quote! {{
            // 先只求值一次较低优先级 View。
            let #styled = #output;
            // 使用元素现有 checked 事实选择叠加层。
            if #condition { #active } else { #styled }
        }};
    }
    // disabled 最后叠加，确保禁用外观优先于交互与选中态。
    if let Some((condition, properties)) = &binding.disabled {
        // 生成现有 disabled 事实读取。
        let condition = condition_tokens(condition)?;
        // 生成 disabled 差异字段应用。
        let active = apply_style_properties(quote! { #styled }, properties)?;
        // 只在 disabled 为真时覆盖声明字段。
        output = quote! {{
            // 先只求值一次较低优先级 View。
            let #styled = #output;
            // 使用元素现有 disabled 事实选择最高优先级叠加层。
            if #condition { #active } else { #styled }
        }};
    }
    // 返回完成状态叠加的 View。
    Ok(output)
}

// 生成一个已经降低的伪类布尔事实读取。
fn condition_tokens(condition: &PseudoStyleCondition) -> Result<TokenStream, Diagnostic> {
    // 按事实形状生成只求值一次的布尔表达式。
    match condition {
        // 静态布尔直接生成字面量。
        PseudoStyleCondition::Literal(value) => Ok(quote! { #value }),
        // 普通值表达式由 Rust 类型检查保证 bool。
        PseudoStyleCondition::Value(expression) => {
            // 生成已经完成组件绑定改写的表达式。
            generate_expression(&expression.expression, None)
        }
        // State<bool> 通过现有 get() 读取并登记依赖。
        PseudoStyleCondition::State(expression) => {
            // 生成已经保持状态句柄的表达式。
            let state = generate_expression(&expression.expression, None)?;
            // 返回当前布尔值。
            Ok(quote! { (#state).get() })
        }
    }
}
