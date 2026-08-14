// 引入过程宏标识符、跨度与令牌流。
use proc_macro2::{Ident, Span, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入动态样式包裹入口。
use super::dynamic_style_codegen::apply_dynamic_style;
// 引入状态伪类叠加入口。
use super::pseudo_style_codegen::apply_pseudo_style;
// 引入组件状态装饰与诊断。
use super::{ComponentScopeMarker, Diagnostic};

// 把展开阶段保存的组件状态装饰应用到最终 ViewNode。
pub(super) fn apply_component_scopes(
    // 接收已经构造完成的实际 ViewNode。
    mut view: TokenStream,
    // 接收由外层到内层累积的组件状态装饰。
    markers: &[ComponentScopeMarker],
) -> Result<TokenStream, Diagnostic> {
    // 按装饰保存顺序应用，确保事件 setter 位于动态样式状态作用域内。
    for marker in markers {
        // 按装饰类别选择普通生命周期标记或动态样式包裹。
        match marker {
            // 普通组件根只附加非视觉生命周期标记。
            ComponentScopeMarker::Scope {
                scope_name,
                root_ordinal,
            } => {
                // 从生成阶段保存的卫生名称重建作用域标识符。
                let scope = Ident::new(scope_name, Span::call_site());
                // 读取多根组件中的稳定根序号。
                let root_ordinal = *root_ordinal;
                // 交给运行时登记该根对组件私有状态作用域的活跃引用。
                view = quote! {
                    // 让运行时把当前 View 根与组件私有状态实例建立生命周期关联。
                    (#view).uix_component_scope((#scope).clone(), #root_ordinal)
                };
            }
            // 动态样式必须包裹已经注册事件的当前实际 View。
            ComponentScopeMarker::DynamicStyle(binding) => {
                // 应用组件私有状态、闭合 setter 与完整样式分支。
                view = apply_dynamic_style(view, binding)?;
            }
            // 状态伪类在基础与自定义动态样式之上只叠加声明字段。
            ComponentScopeMarker::PseudoStyle(binding) => {
                // 应用自动 hover 与既有 disabled/checked 事实选择。
                view = apply_pseudo_style(view, binding)?;
            }
        }
    }
    // 返回包含全部组件状态装饰的 ViewNode。
    Ok(view)
}
