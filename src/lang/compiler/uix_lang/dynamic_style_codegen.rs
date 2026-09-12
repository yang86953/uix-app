// 引入过程宏标识符、跨度与令牌流。
use proc_macro2::{Ident, Span, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入动态样式 AST、诊断与表达式生成入口。
use super::{Diagnostic, DynamicStyleBinding, DynamicStyleKey, generate_expression};
// 引入共享 Style 字段映射入口。
use super::style_codegen::apply_style_properties;

// 用组件私有 State 和闭合枚举包裹声明事件的当前 View。
pub(super) fn apply_dynamic_style(
    // 接收已经应用普通属性与事件的 View 表达式。
    view: TokenStream,
    // 接收展开期完成验证的动态样式元数据。
    binding: &DynamicStyleBinding,
) -> Result<TokenStream, Diagnostic> {
    // 重建最近组件作用域标识符。
    let widget_scope = Ident::new(&binding.widget_scope_name, Span::call_site());
    // 重建闭合样式枚举名称。
    let enum_ident = Ident::new(&binding.enum_name, Span::call_site());
    // 创建内部节点作用域局部变量。
    let node_scope = Ident::new("__uix_dynamic_style_scope", Span::mixed_site());
    // 创建动态样式 State 局部变量。
    let state = Ident::new("__uix_dynamic_style_state", Span::mixed_site());
    // 创建当前枚举值局部变量。
    let current = Ident::new("__uix_dynamic_style_current", Span::mixed_site());
    // 创建只求值一次的节点身份字符串。
    let identity = Ident::new("__uix_dynamic_style_identity", Span::mixed_site());
    // 创建只求值一次的显式 key 字符串。
    let key_value = Ident::new("__uix_dynamic_style_key", Span::mixed_site());
    // 创建事件已注册的 View 局部变量。
    let dynamic_view = Ident::new("__uix_dynamic_style_view", Span::mixed_site());
    // 重建全部枚举变体名称。
    let variants = binding
        // 遍历闭合分支。
        .variants
        // 借用迭代器。
        .iter()
        // 把安全内部名称恢复为标识符。
        .map(|variant| Ident::new(&variant.variant_name, Span::call_site()))
        // 收集确定顺序。
        .collect::<Vec<_>>();
    // 为每个分支生成零参数状态写入器。
    let mut setters = Vec::new();
    // 按闭合分支顺序生成全部逐调用点 setter。
    for (variant, variant_ident) in binding.variants.iter().zip(variants.iter()) {
        // 为同一目标的每个事件调用点生成独占闭包。
        for setter_name in &variant.setter_names {
            // 重建事件表达式引用的 setter 名称。
            let setter = Ident::new(setter_name, Span::call_site());
            // 为每个闭包创建独立 State 克隆名称。
            let setter_state = Ident::new(
                &format!("__uix_dynamic_style_state_{setter_name}"),
                Span::mixed_site(),
            );
            // 保存只写入闭合枚举值的无参数闭包。
            setters.push(quote! {
                // 克隆 State 句柄以独立移动到当前事件闭包。
                let #setter_state = #state.clone();
                // 当前 setter 只能选择编译期已登记的枚举分支。
                let #setter = move || #setter_state.set(#enum_ident::#variant_ident);
            });
        }
    }
    // 为每个枚举分支生成完整 Style 字段应用。
    let mut style_arms = Vec::new();
    // 按分支顺序生成匹配臂。
    for (variant, variant_ident) in binding.variants.iter().zip(variants.iter()) {
        // 复用静态样式字段映射生成当前分支。
        let styled = apply_style_properties(quote! { #dynamic_view }, &variant.properties)?;
        // 保存只会执行一个分支的移动匹配臂。
        style_arms.push(quote! { #enum_ident::#variant_ident => #styled });
    }
    // 生成显式 key 的求值与 ViewNode 应用语句。
    let (key_setup, keyed_view) = match &binding.key {
        // 字面量 key 直接转换为拥有所有权的字符串。
        Some(DynamicStyleKey::Literal(value)) => (
            quote! { let #key_value = ::std::string::String::from(#value); },
            quote! { (#view).key(#key_value.clone()) },
        ),
        // 表达式 key 只求值一次后统一格式化。
        Some(DynamicStyleKey::Expression(node)) => {
            // 生成已经完成组件绑定改写的表达式。
            let value = generate_expression(&node.expression, None)?;
            // 返回求值与实际 View key 应用。
            (
                quote! { let #key_value = ::std::format!("{}", #value); },
                quote! { (#view).key(#key_value.clone()) },
            )
        }
        // 无显式 key 时保持原 View。
        None => (quote! {}, quote! { #view }),
    };
    // 重建可选 For 实例路径标识符。
    let instance_path = binding
        // 借用可选名称。
        .instance_path_name
        // 把内部名称恢复为局部变量。
        .as_ref()
        // 使用调用点卫生解析到循环体声明。
        .map(|name| Ident::new(name, Span::call_site()));
    // 按 For 路径与显式 key 组合实际节点身份。
    let identity_value = match (instance_path, binding.key.is_some()) {
        // 嵌套循环路径与节点 key 共同参与身份。
        (Some(path), true) => quote! { ::std::format!("{}|{}", #path, #key_value) },
        // 无显式 key 时循环实例路径就是稳定身份。
        (Some(path), false) => quote! { (#path).clone() },
        // 静态位置的显式 key 直接作为身份。
        (None, true) => quote! { #key_value.clone() },
        // 静态无 key 节点由声明标识与固定段共同确定。
        (None, false) => quote! { ::std::string::String::from("static") },
    };
    // 取出运行时状态身份常量。
    let declaration_id = binding.declaration_id;
    // 取出子作用域字段编号。
    let field_id = binding.field_id;
    // 初始值永远选择原始 class 分支。
    let original = variants.first().expect("动态样式至少包含原始分支");
    // 返回状态创建、事件闭包与样式匹配的单一表达式。
    Ok(quote! {{
        // 闭合枚举使运行时无法写入未声明类名。
        #[derive(Clone)]
        enum #enum_ident {
            // 仅生成展开期确认可达的分支。
            #(#variants),*
        }
        // 显式 key 在身份与 ViewNode 之间只求值一次。
        #key_setup
        // 组合循环实例路径、节点 key 与静态声明身份。
        let #identity = #identity_value;
        // 从最近组件作用域派生当前实际节点的状态作用域。
        let #node_scope = ::uix_app::ui::__private::uix_widget_child_scope(
            &#widget_scope,
            #declaration_id,
            #identity,
        );
        // 在窗口私有组件状态存储中取得动态样式枚举。
        let #state = ::uix_app::ui::__private::uix_widget_state(
            &#node_scope,
            #field_id,
            || #enum_ident::#original,
        );
        // 读取值以登记结构性 reconcile 依赖。
        let #current = #state.get();
        // 为事件表达式声明全部闭合 setter。
        #(#setters)*
        // 事件闭包在 setter 的词法作用域内构建。
        let #dynamic_view = #keyed_view;
        // 当前枚举分支应用完整 Style 字段并承载生命周期标记。
        (match #current {
            #(#style_arms),*
        }).uix_widget_scope(#node_scope, 0)
    }})
}
